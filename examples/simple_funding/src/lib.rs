use std::{cell::RefCell, sync::Arc};

use candid::{self, CandidType, Deserialize, Principal};
use canfund::{manager::{options::{CyclesThreshold, FundManagerOptions, FundStrategy}, RegisterOpts}, operations::fetch::FetchCyclesBalanceFromCanisterStatus, FundManager};


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
    if config.funded_canister_ids.is_empty() {
        return;
    }

    FUND_MANAGER.with(|fund_manager| {
        let mut fund_manager = fund_manager.borrow_mut();

        let fund_manager_options = FundManagerOptions::new()
            .with_interval_secs(12 * 60 * 60) // twice a day
            .with_strategy(FundStrategy::BelowThreshold(
                CyclesThreshold::new()
                    .with_min_cycles(125_000_000_000)
                    .with_fund_cycles(250_000_000_000),
        ));        

        fund_manager.with_options(fund_manager_options);

        for canister_id in config.funded_canister_ids {
            fund_manager.register(
                canister_id,
                RegisterOpts::new().with_cycles_fetcher(
                    Arc::new(FetchCyclesBalanceFromCanisterStatus)
                ),
            );
        }

        fund_manager.start();
    });
}
