use crate::manager::options::ObtainCyclesOptions;
use crate::operations::fetch::FetchCyclesBalance;
use std::sync::Arc;

use super::{history::ConsumptionHistory, options::FundStrategy};

#[derive(Clone)]
pub struct CanisterRecord {
    /// The canister cycles balance record for the last check.
    cycles: Option<CyclesBalance>,
    /// The canister cycles balance record when it was last funded.
    previous_cycles: Option<CyclesBalance>,
    /// The cycles consumption history of the canister.
    consumption_history: ConsumptionHistory,
    /// The cumulative total of cycles deposited to the canister.
    deposited_cycles: Option<CyclesBalance>,
    /// The last deposited cycles to the canister.
    last_deposited_cycles: Option<CyclesBalance>,
    /// The method to fetch the canister cycles balance.
    cycles_fetcher: Arc<dyn FetchCyclesBalance>,
    /// Optional fund strategy for the canister which overrides the global strategy.
    strategy: Option<FundStrategy>,
    /// Optional minting strategy for the canister which overrides the global strategy.
    obtain_cycles_options: Option<ObtainCyclesOptions>,
    /// Tracks the state of funding failures for the canister.
    funding_failure: Option<FundingFailure>,
}

impl CanisterRecord {
    pub fn new(
        cycles_fetcher: Arc<dyn FetchCyclesBalance>,
        strategy: Option<FundStrategy>,
        obtain_cycles_options: Option<ObtainCyclesOptions>,
        history_window_size: usize,
    ) -> Self {
        Self {
            cycles: None,
            consumption_history: ConsumptionHistory::new(history_window_size),
            previous_cycles: None,
            deposited_cycles: None,
            last_deposited_cycles: None,
            cycles_fetcher,
            strategy,
            obtain_cycles_options,
            funding_failure: None,
        }
    }

    pub fn set_cycles(&mut self, cycles: CyclesBalance) {
        if let Some(previous_cycles) = self.cycles.as_ref() {
            self.previous_cycles = Some(previous_cycles.clone());
            // Timestamp difference is in nanoseconds, so we need to multiply by 1_000_000_000 to get cycles per second.
            self.consumption_history.add_sample(
                (previous_cycles.amount.saturating_sub(cycles.amount) * 1_000_000_000
                    / cycles.timestamp.saturating_sub(previous_cycles.timestamp) as u128) as u64,
            );
        }

        self.cycles = Some(cycles);
    }

    pub fn get_cycles(&self) -> &Option<CyclesBalance> {
        &self.cycles
    }

    pub fn get_previous_cycles(&self) -> &Option<CyclesBalance> {
        &self.previous_cycles
    }

    pub fn add_deposited_cycles(&mut self, deposited_cycles: CyclesBalance) {
        if let Some(total_deposited_cycles) = self.deposited_cycles.as_mut() {
            total_deposited_cycles.amount = total_deposited_cycles
                .amount
                .saturating_add(deposited_cycles.amount);
            total_deposited_cycles.timestamp = deposited_cycles.timestamp;
        } else {
            self.deposited_cycles = Some(deposited_cycles.clone());
        }

        // Adds the last deposited cycles to the record, so it can be used to facilitate consumer's decision making
        // when the CanisterRecord is retrieved.
        self.last_deposited_cycles = Some(deposited_cycles.clone());

        // Update the cycles balance to reflect the deposited amount.
        // This allows for the history to be accuratly calculated.
        self.cycles = self.cycles.as_ref().map(|cycles| {
            CyclesBalance::new(cycles.amount + deposited_cycles.amount, cycles.timestamp)
        });
    }

    pub fn get_deposited_cycles(&self) -> &Option<CyclesBalance> {
        &self.deposited_cycles
    }

    pub fn get_last_deposited_cycles(&self) -> &Option<CyclesBalance> {
        &self.last_deposited_cycles
    }

    pub fn get_cycles_fetcher(&self) -> Arc<dyn FetchCyclesBalance> {
        self.cycles_fetcher.clone()
    }

    pub fn get_strategy(&self) -> &Option<FundStrategy> {
        &self.strategy
    }

    pub fn get_obtain_cycles_options(&self) -> &Option<ObtainCyclesOptions> {
        &self.obtain_cycles_options
    }

    /// Returns the average consumption of the canister in cycles per second.
    pub fn get_average_consumption(&self) -> u64 {
        self.consumption_history.average()
    }

    pub fn get_funding_failure(&self) -> Option<&FundingFailure> {
        self.funding_failure.as_ref()
    }

    pub fn set_funding_failure(&mut self, error_code: FundingErrorCode, timestamp: u64) {
        self.funding_failure = Some(FundingFailure {
            error_code,
            timestamp,
        });
    }

    pub fn reset_funding_failure(&mut self) {
        self.funding_failure = None;
    }
}

/// The canister cycles balance record.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CyclesBalance {
    /// The cycles balance of the canister.
    pub amount: u128,
    /// The timestamp when the cycles were last updated.
    pub timestamp: u64,
}

impl CyclesBalance {
    /// Constructs a new CyclesBalance with the specified amount and timestamp.
    pub fn new(amount: u128, timestamp: u64) -> Self {
        Self { amount, timestamp }
    }
}

/// The funding failure record.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FundingFailure {
    /// The code of the reason for the funding failure.
    pub error_code: FundingErrorCode,
    /// The timestamp of the failure.
    pub timestamp: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FundingErrorCode {
    #[default]
    InsufficientCycles, // Funding canister has insufficient cycles
    DepositFailed,      // The deposit of cycles failed
    ObtainCyclesFailed, // Obtaining cycles failed
    BalanceCheckFailed, // Fetching cycles balance failed
    Other(String),      // Other errors with a custom message
}

impl FundingErrorCode {
    /// Returns a human-readable error message for the error code.
    pub fn message(&self) -> String {
        match self {
            FundingErrorCode::InsufficientCycles => {
                "Insufficient cycles in the funding canister.".to_string()
            }
            FundingErrorCode::DepositFailed => "The deposit of cycles failed.".to_string(),
            FundingErrorCode::ObtainCyclesFailed => {
                "Obtaining cycles for the canister failed.".to_string()
            }
            FundingErrorCode::BalanceCheckFailed => "Fetching cycles balance failed.".to_string(),
            FundingErrorCode::Other(msg) => msg.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::operations::fetch::FetchCyclesBalanceFromCanisterStatus;

    #[test]
    fn test_canister_record() {
        let cycles_fetcher = Arc::new(FetchCyclesBalanceFromCanisterStatus::new());
        let mut canister_record = CanisterRecord::new(cycles_fetcher, None, None, 0);

        let cycles = CyclesBalance::new(100, 100);
        canister_record.set_cycles(cycles.clone());
        assert_eq!(canister_record.get_cycles(), &Some(cycles));
        assert_eq!(canister_record.get_previous_cycles(), &None);

        let previous_cycles = canister_record.get_cycles().as_ref().unwrap().clone();
        canister_record.set_cycles(CyclesBalance::new(200, 200));
        assert_eq!(
            canister_record.get_previous_cycles(),
            &Some(previous_cycles)
        );

        let deposited_cycles = CyclesBalance::new(50, 1234567890);
        canister_record.add_deposited_cycles(deposited_cycles.clone());
        assert_eq!(
            canister_record.get_deposited_cycles(),
            &Some(CyclesBalance::new(50, deposited_cycles.timestamp))
        );

        canister_record.add_deposited_cycles(deposited_cycles.clone());
        assert_eq!(
            canister_record.get_deposited_cycles(),
            &Some(CyclesBalance::new(100, deposited_cycles.timestamp))
        );

        assert_eq!(
            canister_record.get_last_deposited_cycles(),
            &Some(CyclesBalance::new(50, deposited_cycles.timestamp))
        );

        assert_eq!(canister_record.get_average_consumption(), 0);
    }

    #[test]
    fn test_canister_consumption() {
        let cycles_fetcher = Arc::new(FetchCyclesBalanceFromCanisterStatus::new());
        let mut canister_record = CanisterRecord::new(cycles_fetcher, None, None, 5);

        canister_record.set_cycles(CyclesBalance::new(300_000, 1_000_000_000));
        canister_record.set_cycles(CyclesBalance::new(200_000, 2_000_000_000));
        canister_record.set_cycles(CyclesBalance::new(250_000, 3_000_000_000)); // reservations returned

        canister_record.add_deposited_cycles(CyclesBalance::new(500_000, 3_000_000_100));

        assert_eq!(canister_record.get_average_consumption(), 50_000);

        canister_record.set_cycles(CyclesBalance::new(600_000, 4_000_000_000));
        canister_record.set_cycles(CyclesBalance::new(350_000, 5_000_000_000));
        canister_record.set_cycles(CyclesBalance::new(250_000, 6_000_000_000));
        canister_record.set_cycles(CyclesBalance::new(200_000, 7_000_000_000));

        assert_eq!(canister_record.get_average_consumption(), 110_000);
    }

    #[test]
    fn test_set_funding_failure() {
        let mut record = CanisterRecord::new(
            Arc::new(FetchCyclesBalanceFromCanisterStatus::new()),
            None,
            None,
            0,
        );

        // Initially, there should be no funding failure
        assert!(record.get_funding_failure().is_none());

        // Set a funding failure
        let error_code = FundingErrorCode::DepositFailed;
        let timestamp = 123456789;
        record.set_funding_failure(error_code.clone(), timestamp);

        // Check if the funding failure is set correctly
        let failure = record.get_funding_failure().unwrap();
        assert_eq!(failure.error_code, error_code);
        assert_eq!(failure.timestamp, timestamp);
    }
}
