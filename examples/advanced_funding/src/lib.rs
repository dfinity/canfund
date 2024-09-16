use std::{cell::RefCell, rc::Rc, sync::Arc};

use candid::{self, CandidType, Deserialize, Principal};
use canfund::{
    api::{cmc::IcCyclesMintingCanister, ledger::IcLedgerCanister},
    manager::{
        options::{EstimatedRuntime, FundManagerOptions, FundStrategy, ObtainCyclesOptions},
        RegisterOpts,
    },
    operations::{fetch::FetchCyclesBalanceFromCanisterStatus, obtain::MintCycles},
    FundManager,
};
use ic_cdk::{id, query};
use ic_cdk_macros::{init, post_upgrade};
use ic_ledger_types::{
    Subaccount, DEFAULT_SUBACCOUNT, MAINNET_CYCLES_MINTING_CANISTER_ID, MAINNET_LEDGER_CANISTER_ID,
};

thread_local! {
    /// Monitor the cycles of canisters and top up if necessary.
    pub static FUND_MANAGER: RefCell<FundManager> = RefCell::new(FundManager::new());
}

#[derive(CandidType, Deserialize)]
pub struct FundingConfig {
    pub funded_canister_ids: Vec<Principal>,
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
                    .with_fallback_min_cycles(125_000_000_000)
                    .with_fallback_fund_cycles(250_000_000_000),
            ))
            .with_funding_callback(Rc::new(|records| {
                // Loop over the hashmap of canister records and print the cycles balance and total of deposited cycles
                for (canister_id, record) in records.iter() {
                    let cycles = record.get_cycles().as_ref().map(|c| c.amount).unwrap_or(0);
                    let deposited_cycles = record
                        .get_deposited_cycles()
                        .as_ref()
                        .map(|c| c.amount)
                        .unwrap_or(0);
                    ic_cdk::print(format!(
                        "Canister {} had {} cycles and got {} deposited cycles",
                        canister_id, cycles, deposited_cycles
                    ));
                }
            }));

        fund_manager_options =
            fund_manager_options.with_obtain_cycles_options(get_obtain_cycles_config());

        fund_manager.with_options(fund_manager_options);

        for canister_id in config.funded_canister_ids {
            fund_manager.register(
                canister_id,
                RegisterOpts::new()
                    .with_cycles_fetcher(Arc::new(FetchCyclesBalanceFromCanisterStatus)),
            );
        }

        // By default, the fund manager registers its canister with the FetchOwnCyclesBalance operation
        // We are able to override it with FetchCyclesBalanceFromCanisterStatus because even canisters that
        // are not their own controller can call the canister_status endpoint
        fund_manager.unregister(id());
        fund_manager.register(
            id(),
            RegisterOpts::new().with_cycles_fetcher(Arc::new(FetchCyclesBalanceFromCanisterStatus)),
        );

        fund_manager.start();
    });
}

// Default subaccount for minting cycles is derived from the canister's account.
pub fn get_obtain_cycles_config() -> Option<ObtainCyclesOptions> {
    Some(ObtainCyclesOptions {
        obtain_cycles: Arc::new(MintCycles {
            ledger: Arc::new(IcLedgerCanister::new(MAINNET_LEDGER_CANISTER_ID)),
            cmc: Arc::new(IcCyclesMintingCanister::new(
                MAINNET_CYCLES_MINTING_CANISTER_ID,
            )),
            from_subaccount: Subaccount::from(DEFAULT_SUBACCOUNT),
        }),
        top_up_self: true,
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
                    .map(|c| c.amount)
                    .unwrap_or(0);
                GetDepositedCyclesRetItem {
                    deposited_cycles,
                    canister_id: *canister_id,
                }
            })
            .collect()
    })
}
