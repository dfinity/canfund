use std::{cell::RefCell, sync::Arc};

use candid::{self, CandidType, Deserialize, Principal};
use canfund::{manager::{options::{EstimatedRuntime, FundManagerOptions, FundStrategy}, RegisterOpts}, operations::fetch::{FetchCyclesBalance, FetchCyclesBalanceFromCanisterStatus}, FundManager};

thread_local! {
    /// Monitor the cycles of canisters and top up if necessary.
    pub static FUND_MANAGER: RefCell<FundManager> = RefCell::new(FundManager::new());
}

#[derive(CandidType, Deserialize)]
pub struct FundingConfig { pub funded_canister_ids: Vec<Principal> }

#[ic_cdk_macros::init]
async fn initialize(config: FundingConfig) {
    start_canister_cycles_monitoring(config);
}   

pub fn start_canister_cycles_monitoring(config: FundingConfig) {
    FUND_MANAGER.with(|fund_manager| {
        let mut fund_manager = fund_manager.borrow_mut();

        fund_manager.with_options(
            FundManagerOptions::new()
                .with_interval_secs(12 * 60 * 60) // twice a day
                .with_strategy(FundStrategy::BelowEstimatedRuntime(
                    EstimatedRuntime::new()
                        .with_min_runtime_secs(2 * 24 * 60 * 60) // 2 days
                        .with_fund_runtime_secs(5 * 24 * 60 * 60) // 3 days
                        .with_max_runtime_cycles_fund(1_000_000_000_000)
                        .with_fallback_min_cycles(125_000_000_000)
                        .with_fallback_fund_cycles(250_000_000_000),
                )),
        );

        for canister_id in config.funded_canister_ids {
            fund_manager.register(
                canister_id,
                RegisterOpts::new().with_cycles_fetcher(create_station_cycles_fetcher()),
            );
        }

        fund_manager.start();
    });
}

pub fn create_station_cycles_fetcher() -> Arc<dyn FetchCyclesBalance> {
    Arc::new(FetchCyclesBalanceFromCanisterStatus)
}