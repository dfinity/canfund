use crate::interfaces::{
    NnsLedgerCanisterInitPayload, NnsLedgerCanisterPayload,
};
use crate::utils::{controller_test_id, minter_test_id, COUNTER_WAT};
use crate::{CanisterIds, TestEnv};
use candid::{CandidType, Encode, Principal};
use simple_funding::FundingConfig;
use ic_ledger_types::{AccountIdentifier, Tokens, DEFAULT_SUBACCOUNT};
use pocket_ic::{PocketIc, PocketIcBuilder};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

static POCKET_IC_BIN: &str = "./pocket-ic";

#[derive(Serialize, CandidType, Clone, Debug, PartialEq, Eq)]
pub enum ExchangeRateCanister {
    /// Enables the exchange rate canister with the given canister ID.
    Set(Principal),
}

#[derive(Serialize, CandidType, Clone, Debug, PartialEq, Eq)]
pub struct CyclesCanisterInitPayload {
    pub ledger_canister_id: Option<Principal>,
    pub governance_canister_id: Option<Principal>,
    pub minting_account_id: Option<AccountIdentifier>,
    pub exchange_rate_canister: Option<ExchangeRateCanister>,
    pub cycles_ledger_canister_id: Option<Principal>,
    pub last_purged_notification: Option<u64>,
}

pub fn setup_new_env() -> TestEnv {
    let path = match env::var_os("POCKET_IC_BIN") {
        None => {
            env::set_var("POCKET_IC_BIN", POCKET_IC_BIN);
            POCKET_IC_BIN.to_string()
        }
        Some(path) => path
            .clone()
            .into_string()
            .unwrap_or_else(|_| panic!("Invalid string path for {path:?}")),
    };

    if !Path::new(&path).exists() {
        println!("
        Could not find the PocketIC binary to run canister integration tests.

        I looked for it at {:?}. You can specify another path with the environment variable POCKET_IC_BIN (note that I run from {:?}).

        Running the testing script will automatically place the PocketIC binary at the right place to be run without setting the POCKET_IC_BIN environment variable:
            ./scripts/run-integration-tests.sh
        ", &path, &env::current_dir().map(|x| x.display().to_string()).unwrap_or_else(|_| "an unknown directory".to_string()));
    }

    let env = PocketIcBuilder::new()
        .with_nns_subnet()
        .with_application_subnet()
        .with_ii_subnet()
        .build();

    // If we set the time to SystemTime::now, and then progress pocketIC a couple ticks
    // and then enter live mode, we would crash the deterministic state machine, as the
    // live mode would set the time back to the current time.
    // Therefore, if we want to use live mode, we need to start the tests with the time
    // set to the past.
    env.set_time(SystemTime::now() - Duration::from_secs(24 * 60 * 60));
    let controller = controller_test_id();
    let minter = minter_test_id();
    let canister_ids = install_canisters(&env, controller, minter);

    TestEnv {
        env,
        canister_ids,
        controller,
        minter,
    }
}

pub fn create_canister_with_cycles(
    env: &PocketIc,
    controller: Principal,
    cycles: u128,
) -> Principal {
    let canister_id = env.create_canister_with_settings(Some(controller), None);
    env.add_cycles(canister_id, cycles);
    canister_id
}

fn install_canisters(
    env: &PocketIc,
    controller: Principal,
    minter: Principal,
) -> CanisterIds {
    let specified_nns_ledger_canister_id =
        Principal::from_text("ryjl3-tyaaa-aaaaa-aaaba-cai").unwrap();
    let nns_ledger_canister_id = env
        .create_canister_with_id(Some(controller), None, specified_nns_ledger_canister_id)
        .unwrap();
    assert_eq!(nns_ledger_canister_id, specified_nns_ledger_canister_id);

    let specified_nns_index_canister_id =
        Principal::from_text("r7inp-6aaaa-aaaaa-aaabq-cai").unwrap();
    let nns_index_canister_id = env
        .create_canister_with_id(Some(controller), None, specified_nns_index_canister_id)
        .unwrap();
    assert_eq!(nns_index_canister_id, specified_nns_index_canister_id);

    let specified_cmc_canister_id = Principal::from_text("rkp4c-7iaaa-aaaaa-aaaca-cai").unwrap();
    let cmc_canister_id = env
        .create_canister_with_id(Some(controller), None, specified_cmc_canister_id)
        .unwrap();
    assert_eq!(cmc_canister_id, specified_cmc_canister_id);

    let specified_nns_exchange_rate_canister_id =
        Principal::from_text("uf6dk-hyaaa-aaaaq-qaaaq-cai").unwrap();
    let nns_exchange_rate_canister_id = env
        .create_canister_with_id(Some(controller), None, specified_nns_exchange_rate_canister_id)
        .unwrap();
    assert_eq!(nns_exchange_rate_canister_id,specified_nns_exchange_rate_canister_id);

    let nns_governance_canister_id = Principal::from_text("rrkah-fqaaa-aaaaa-aaaaq-cai").unwrap();
    let nns_cycles_ledger_canister_id =
        Principal::from_text("um5iw-rqaaa-aaaaq-qaaba-cai").unwrap();

    let controller_account = AccountIdentifier::new(&controller, &DEFAULT_SUBACCOUNT);
    let minting_account = AccountIdentifier::new(&minter, &DEFAULT_SUBACCOUNT);

    let icp_ledger_canister_wasm = get_canister_wasm("icp_ledger").to_vec();
    let icp_ledger_init_args = NnsLedgerCanisterPayload::Init(NnsLedgerCanisterInitPayload {
        minting_account: minting_account.to_string(),
        initial_values: HashMap::from([(
            controller_account.to_string(),
            Tokens::from_e8s(1_000_000_000_000),
        )]),
        send_whitelist: HashSet::new(),
        transfer_fee: Some(Tokens::from_e8s(10_000)),
        token_symbol: Some("ICP".to_string()),
        token_name: Some("Internet Computer".to_string()),
    });
    env.install_canister(
        nns_ledger_canister_id,
        icp_ledger_canister_wasm,
        Encode!(&icp_ledger_init_args).unwrap(),
        Some(controller),
    );

    let cmc_canister_wasm = get_canister_wasm("cmc").to_vec();
    let cmc_init_args: Option<CyclesCanisterInitPayload> = Some(CyclesCanisterInitPayload {
        ledger_canister_id: Some(nns_ledger_canister_id),
        governance_canister_id: Some(nns_governance_canister_id),
        minting_account_id: Some(minting_account),
        exchange_rate_canister: Some(ExchangeRateCanister::Set(nns_exchange_rate_canister_id)),
        cycles_ledger_canister_id: Some(nns_cycles_ledger_canister_id),
        last_purged_notification: Some(0),
    });
    env.install_canister(
        cmc_canister_id,
        cmc_canister_wasm,
        Encode!(&cmc_init_args).unwrap(),
        Some(controller),
    );

    CanisterIds {
        icp_ledger: nns_ledger_canister_id,
        cycles_minting_canister: cmc_canister_id,
    }
}

pub fn install_simple_funding_canister(env: &PocketIc, controller: Principal, cycles: u128, funded_canister_ids: Vec<Principal>) -> Principal {
    // simple funding canister starts with more cycles so it does not run out of cycles before the funded canister does
    let funding_canister_id = create_canister_with_cycles(
        env,
        controller,
        cycles,
    );

    
    let funding_canister_wasm = get_canister_wasm("simple_funding").to_vec();
    let funding_canister_args = FundingConfig {
        funded_canister_ids: funded_canister_ids,
    };
    env.install_canister(
        funding_canister_id,
        funding_canister_wasm,
        Encode!(&funding_canister_args).unwrap(),
        Some(controller),
    );
    
    funding_canister_id
}

pub fn install_advanced_funding_canister(env: &PocketIc, controller: Principal, cycles: u128, funded_canister_ids: Vec<Principal>) -> Principal {
    // simple funding canister starts with more cycles so it does not run out of cycles before the funded canister does
    let funding_canister_id = create_canister_with_cycles(
        env,
        controller,
        cycles,
    );

    
    let funding_canister_wasm = get_canister_wasm("advanced_funding").to_vec();
    let funding_canister_args = FundingConfig {
        funded_canister_ids: funded_canister_ids,
    };
    env.install_canister(
        funding_canister_id,
        funding_canister_wasm,
        Encode!(&funding_canister_args).unwrap(),
        Some(controller),
    );
    
    funding_canister_id
}

pub fn install_funded_canister(env: &PocketIc, controller: Principal, cycles: u128) -> Principal {
    // simple canister to burn cycles and trigger funding rules
    let funded_canister_id = create_canister_with_cycles(
        env, 
        controller, 
        cycles,
    );
    let module_bytes = wat::parse_str(COUNTER_WAT).unwrap();
    env.install_canister(funded_canister_id, module_bytes.clone(), vec![], Some(controller));
    
    funded_canister_id
}

pub(crate) fn get_canister_wasm(canister_name: &str) -> Vec<u8> {
    read_file_from_local_bin(&format!("{canister_name}.wasm.gz"))
}

fn local_bin() -> PathBuf {
    let mut file_path = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR")
            .expect("Failed to read CARGO_MANIFEST_DIR env variable"),
    );
    file_path.push("wasms");
    file_path
}

fn read_file_from_local_bin(file_name: &str) -> Vec<u8> {
    let mut file_path = local_bin();
    file_path.push(file_name);

    let mut file = File::open(&file_path)
        .unwrap_or_else(|_| panic!("Failed to open file: {}", file_path.to_str().unwrap()));
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).expect("Failed to read file");
    bytes
}
