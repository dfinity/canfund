use candid::{self, CandidType, Deserialize, Nat, Principal};
use canfund::api::ledger::CyclesLedgerCanister;
use canfund::operations::obtain::WithdrawFromLedger;
use canfund::{
    manager::{
        options::{
            CyclesThreshold, EstimatedRuntime, FundManagerOptions, FundStrategy,
            ObtainCyclesOptions,
        },
        RegisterOpts,
    },
    operations::fetch::FetchCyclesBalanceFromCanisterStatus,
    FundManager,
};
use ic_cdk::api::call::call_with_payment128;
use ic_cdk::{id, query};
use ic_cdk_macros::{init, post_upgrade, update};
use icrc_ledger_types::icrc1::account::Account;
use icrc_ledger_types::icrc1::transfer::Memo;
use std::{cell::RefCell, sync::Arc};

thread_local! {
    /// Monitor the cycles of canisters and top up if necessary.
    pub static FUND_MANAGER: RefCell<FundManager> = RefCell::new(FundManager::new());
}

#[derive(CandidType, Deserialize)]
pub struct FundingConfig {
    pub funded_canister_ids: Vec<Principal>,
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DepositArg {
    pub cycles: u128,
    pub to: Account,
    pub memo: Option<Memo>,
}

#[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct DepositResult {
    pub block_index: Nat,
    pub balance: Nat,
}

#[init]
fn initialize(config: FundingConfig) {
    start_canister_cycles_monitoring(config);
}

#[post_upgrade]
fn post_upgrade(config: FundingConfig) {
    start_canister_cycles_monitoring(config);
}

pub fn start_canister_cycles_monitoring(config: FundingConfig) {
    FUND_MANAGER.with(|fund_manager| {
        let mut fund_manager = fund_manager.borrow_mut();

        let mut fund_manager_options = FundManagerOptions::new()
            .with_interval_secs(12 * 60 * 60) // twice a day
            .with_strategy(FundStrategy::BelowEstimatedRuntime(
                EstimatedRuntime::new()
                    .with_min_runtime_secs(2 * 24 * 60 * 60) // 2 day
                    .with_fund_runtime_secs(5 * 24 * 60 * 60) // 3 days
                    .with_max_runtime_cycles_fund(1_000_000_000_000)
                    .with_fallback_min_cycles(400_000_000_000)
                    .with_fallback_fund_cycles(250_000_000_000),
            ));

        fund_manager_options =
            fund_manager_options.with_obtain_cycles_options(get_obtain_cycles_config());

        fund_manager.with_options(fund_manager_options);

        for canister_id in config.funded_canister_ids {
            fund_manager.register(
                canister_id,
                RegisterOpts::new()
                    .with_cycles_fetcher(Arc::new(FetchCyclesBalanceFromCanisterStatus::new()))
                    .with_obtain_cycles_options(get_obtain_cycles_config().unwrap()),
            );
        }

        // The funding canister itself can also be monitored.
        fund_manager.register(
            id(),
            RegisterOpts::new()
                .with_cycles_fetcher(Arc::new(FetchCyclesBalanceFromCanisterStatus::new()))
                .with_strategy(FundStrategy::BelowThreshold(
                    CyclesThreshold::new()
                        .with_min_cycles(500_000_000_000)
                        .with_fund_cycles(750_000_000_000),
                )),
        );

        fund_manager.start();
    });
}

pub const MAINNET_CYCLES_LEDGER_CANISTER_ID: Principal =
    Principal::from_slice(&[0x00, 0x00, 0x00, 0x00, 0x02, 0x10, 0x00, 0x02, 0x01, 0x01]);
// Default subaccount for minting cycles is derived from the canister's account.
pub fn get_obtain_cycles_config() -> Option<ObtainCyclesOptions> {
    Some(ObtainCyclesOptions {
        obtain_cycles: Arc::new(WithdrawFromLedger {
            ledger: Arc::new(CyclesLedgerCanister::new(MAINNET_CYCLES_LEDGER_CANISTER_ID)),
            from_subaccount: None,
        }),
    })
}

#[derive(CandidType, Deserialize)]
pub struct GetDepositedCyclesRetItem {
    pub deposited_cycles: u128,
    pub canister_id: Principal,
}

#[query(name = "get_deposited_cycles")]
fn get_deposited_cycles() -> Vec<GetDepositedCyclesRetItem> {
    FUND_MANAGER.with(|fund_manager| {
        let fund_manager = fund_manager.borrow();

        fund_manager
            .get_canisters()
            .iter()
            .map(|(canister_id, record)| {
                let deposited_cycles = record
                    .get_deposited_cycles()
                    .as_ref()
                    .map_or(0, |c| c.amount);
                GetDepositedCyclesRetItem {
                    deposited_cycles,
                    canister_id: *canister_id,
                }
            })
            .collect()
    })
}

#[update]
async fn deposit(arg: DepositArg) -> DepositResult {
    #[derive(CandidType, Deserialize, Clone, Debug, PartialEq, Eq)]
    pub struct CallDepositArg {
        pub to: Account,
        pub memo: Option<Memo>,
    }

    let call_arg = CallDepositArg {
        to: arg.to,
        memo: arg.memo,
    };

    let cycles = arg.cycles;
    let (result,): (DepositResult,) = call_with_payment128(
        MAINNET_CYCLES_LEDGER_CANISTER_ID,
        "deposit",
        (call_arg,),
        cycles,
    )
    .await
    .expect("deposit call failed");
    result
}
