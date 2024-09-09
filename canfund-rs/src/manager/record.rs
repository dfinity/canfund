use std::sync::Arc;

use crate::operations::fetch::FetchCyclesBalance;

#[derive(Clone)]
pub struct CanisterRecord {
    /// The canister cycles balance record for the last check.
    cycles: Option<CyclesBalance>,
    /// The canister cycles balance record when it was last funded.
    previous_cycles: Option<CyclesBalance>,
    /// The number of cycles deposited in the last check.
    deposited_cycles: Option<CyclesBalance>,
    /// The method to fetch the canister cycles balance.
    cycles_fetcher: Arc<dyn FetchCyclesBalance>,
}

impl CanisterRecord {
    pub fn new(cycles_fetcher: Arc<dyn FetchCyclesBalance>) -> Self {
        Self {
            cycles: None,
            previous_cycles: None,
            deposited_cycles: None,
            cycles_fetcher,
        }
    }

    pub fn set_cycles(&mut self, cycles: CyclesBalance) {
        if let Some(previous_cycles) = self.cycles.as_ref() {
            self.previous_cycles = Some(previous_cycles.clone());
        }

        self.cycles = Some(cycles);
    }

    pub fn get_cycles(&self) -> &Option<CyclesBalance> {
        &self.cycles
    }

    pub fn get_previous_cycles(&self) -> &Option<CyclesBalance> {
        &self.previous_cycles
    }

    pub fn set_deposited_cycles(&mut self, deposited_cycles: CyclesBalance) {
        self.deposited_cycles = Some(deposited_cycles);
    }

    pub fn get_deposited_cycles(&self) -> &Option<CyclesBalance> {
        &self.deposited_cycles
    }

    pub fn get_cycles_fetcher(&self) -> Arc<dyn FetchCyclesBalance> {
        self.cycles_fetcher.clone()
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
