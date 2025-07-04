//! The fund manager that monitors and funds canister cycles based on the configuration.

use self::{
    lock::ProcessExecutionLock,
    options::{FundManagerOptions, FundStrategy},
    record::{CanisterRecord, CyclesBalance},
};
use crate::manager::options::ObtainCyclesOptions;
use crate::manager::record::FundingErrorCode;
use crate::operations::fetch::{FetchCyclesBalance, FetchCyclesBalanceFromCanisterStatus};
use ic_cdk::api::{canister_self, debug_print};
use ic_cdk::management_canister::DepositCyclesArgs;
use ic_cdk::{
    api::time,
    futures::spawn,
    management_canister::{deposit_cycles, CanisterId},
};
use ic_cdk_timers::TimerId;
use std::{
    cell::RefCell,
    cmp,
    collections::{hash_map::Entry, HashMap},
    rc::Rc,
    sync::Arc,
    time::Duration,
};

pub mod history;
pub mod lock;
pub mod options;
pub mod record;

/// The core features of the fund manager.
pub struct FundManagerCore {
    /// The canisters that are being monitored by the fund manager.
    lock: ProcessExecutionLock,
    canisters: HashMap<CanisterId, CanisterRecord>,
    options: FundManagerOptions,
}

/// RegisterOpts holds the options for registering a canister to be monitored by the fund manager.
/// By default, it uses the `FetchCyclesBalanceFromCanisterStatus` to fetch the cycles balance.
/// The fund strategy is set to `None` by default, meaning that the global strategy will be applied.
/// The obtain cycles strategy is set to `None` by default, meaning that the global strategy will be applied.
pub struct RegisterOpts {
    pub cycles_fetcher: Arc<dyn FetchCyclesBalance>,
    pub strategy: Option<FundStrategy>,
    pub obtain_cycles_options: Option<ObtainCyclesOptions>,
}

impl RegisterOpts {
    /// Creates a new register options with the default cycles fetcher.
    pub fn new() -> Self {
        Self {
            cycles_fetcher: Arc::new(FetchCyclesBalanceFromCanisterStatus::new()),
            strategy: None,
            obtain_cycles_options: None,
        }
    }

    /// Sets the cycles fetcher for the register options.
    pub fn with_cycles_fetcher(mut self, cycles_fetcher: Arc<dyn FetchCyclesBalance>) -> Self {
        self.cycles_fetcher = cycles_fetcher;
        self
    }

    /// Sets the funding strategy for the register options.
    pub fn with_strategy(mut self, strategy: FundStrategy) -> Self {
        self.strategy = Some(strategy);
        self
    }

    /// Sets the obtain cycles config for the register options.
    pub fn with_obtain_cycles_options(
        mut self,
        obtain_cycles_options: ObtainCyclesOptions,
    ) -> Self {
        self.obtain_cycles_options = Some(obtain_cycles_options);
        self
    }
}

impl Default for RegisterOpts {
    fn default() -> Self {
        Self::new()
    }
}

/// The fund manager that monitors and funds canisters with cycles based on the configuration.
pub struct FundManager {
    inner: Rc<RefCell<FundManagerCore>>,
    tracker: Option<TimerId>,
}

impl Default for FundManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FundManager {
    /// Creates a new fund manager with the specified options.
    pub fn new() -> Self {
        FundManager {
            inner: FundManagerCore::new(),
            tracker: None,
        }
    }

    /// Configures the fund manager with the specified options.
    pub fn with_options(&mut self, options: FundManagerOptions) -> &mut Self {
        self.inner.borrow_mut().options = options;

        self
    }

    /// Registers a canister to be monitored by the fund manager.
    pub fn register(&mut self, canister_id: CanisterId, opts: RegisterOpts) -> &mut Self {
        self.inner.borrow_mut().register(canister_id, opts);

        self
    }

    /// Unregisters a canister from being monitored by the fund manager.
    pub fn unregister(&mut self, canister_id: CanisterId) -> &mut Self {
        self.inner.borrow_mut().unregister(canister_id);

        self
    }

    /// Returns the canisters that are being monitored by the fund manager.
    pub fn get_canisters(&self) -> HashMap<CanisterId, CanisterRecord> {
        self.inner.borrow().canisters.clone()
    }

    /// Returns the canister record for the specified canister id.
    pub fn get_canister(&self, canister_id: CanisterId) -> Option<CanisterRecord> {
        self.inner.borrow().canisters.get(&canister_id).cloned()
    }

    /// Returns the options for the fund manager.
    pub fn get_options(&self) -> FundManagerOptions {
        self.inner.borrow().options.clone()
    }

    /// Returns whether the fund manager has started tracking the canisters.
    pub fn is_running(&self) -> bool {
        self.tracker.is_some()
    }

    /// Starts the fund manager to monitor and fund the canisters based on the configuration.
    pub fn start(&mut self) {
        let (is_running, interval_secs) = {
            let inner = self.inner.borrow();
            (self.is_running(), inner.options.interval_secs())
        };

        if is_running {
            return;
        }

        self.tracker = Some(FundManager::create_tracker(
            Rc::clone(&self.inner),
            Duration::from_secs(interval_secs),
        ));
    }

    /// Stops the fund manager from monitoring and funding the canisters, if it is running.
    pub fn stop(&mut self) {
        if let Some(tracker) = self.tracker.take() {
            ic_cdk_timers::clear_timer(tracker);
        }
    }

    /// Creates a timer to track the canisters and fund them based on the configuration.
    fn create_tracker(manager: Rc<RefCell<FundManagerCore>>, interval: Duration) -> TimerId {
        let start_immediately = {
            if interval.is_zero() {
                false
            } else {
                !manager.borrow().options.delayed_start()
            }
        };

        if start_immediately {
            let manager = Rc::clone(&manager);
            ic_cdk_timers::set_timer(Duration::from_secs(0), move || {
                spawn(async move {
                    Self::execute_scheduled_monitoring(manager).await;
                });
            });
        }

        // Schedule the timer to run the monitoring at the specified interval.
        ic_cdk_timers::set_timer_interval(interval, move || {
            let manager = Rc::clone(&manager);
            spawn(async move {
                Self::execute_scheduled_monitoring(manager).await;
            });
        })
    }

    /// Executes the scheduled monitoring of the canisters and fund them if needed.
    #[allow(clippy::too_many_lines)]
    async fn execute_scheduled_monitoring(manager: Rc<RefCell<FundManagerCore>>) {
        // Lock the process execution to prevent concurrent executions, it is dropped automatically
        // when it goes out of scope.
        let _lock = {
            manager.borrow_mut().lock.lock(
                "execute_scheduled_monitoring"
                    .to_string()
                    .as_bytes()
                    .to_vec(),
            )
        };

        if _lock.is_none() {
            debug_print("Failed to acquire lock for `execute_scheduled_monitoring`, another process is running");
            return;
        }

        // Reset funding failure for all canister records
        for record in manager.borrow_mut().canisters.values_mut() {
            record.reset_funding_failure();
        }

        let (all_canister_ids, chunk_size) = {
            let manager_ref = manager.borrow();
            let all_canister_ids: Vec<(CanisterId, Arc<dyn FetchCyclesBalance>)> = manager_ref
                .canisters
                .iter()
                .map(|(canister_id, canister_record)| {
                    (*canister_id, canister_record.get_cycles_fetcher())
                })
                .collect();
            let chunk_size = manager_ref.options.chunk_size();
            (all_canister_ids, chunk_size)
        };

        for canister_ids in all_canister_ids.chunks(cmp::max(1, chunk_size as usize)) {
            let canisters_to_fund =
                Self::monitor_specified_canisters(Rc::clone(&manager), canister_ids).await;

            // Funds the canisters with the necessary cycles.
            for (canister_id, needed_cycles) in canisters_to_fund {
                // Before transferring cycles from the funding canister, check if the funding canister actually has enough cycles.
                let funding_canister_needs_cycles = canister_id != canister_self() && {
                    // Get the current balance.
                    let funding_canister_balance = ic_cdk::api::canister_cycle_balance();

                    // Get the record of the funding canister if it exists, to access the previous cycles balance to calculate estimated runtime left.
                    let maybe_funding_canister_record =
                        manager.borrow().canisters.get(&canister_self()).cloned();

                    // see if transferring cycles to the canister will make the funding canister run low of cycles
                    let funding_canister_needed_cycles = calc_needed_cycles(
                        &CyclesBalance::new(
                            funding_canister_balance.saturating_sub(needed_cycles),
                            time(),
                        ),
                        maybe_funding_canister_record
                            .as_ref()
                            .map_or(0, |record| record.get_average_consumption() as u128),
                        &maybe_funding_canister_record
                            .as_ref()
                            .and_then(|record| record.get_strategy().clone())
                            .unwrap_or_else(|| manager.borrow().options.strategy().clone()),
                    );

                    funding_canister_needed_cycles > 0
                };

                // If either the funding canister is low on cycles,
                // or it does not have enough cycles to fund another canister,
                // then need to obtain cycles for the funding canister.
                if canister_id == canister_self() || funding_canister_needs_cycles {
                    let maybe_obtain_cycles = manager
                        .borrow()
                        .canisters
                        .get(&canister_id)
                        .and_then(|record| record.get_obtain_cycles_options().clone())
                        .or_else(|| manager.borrow().options.obtain_cycles_options().clone());

                    if let Some(obtain_cycles_options) = maybe_obtain_cycles {
                        ic_cdk::println!(
                            "Topping up {} with {} cycles",
                            canister_id,
                            needed_cycles
                        );

                        let mut tries_left = 4;
                        while tries_left > 0 {
                            tries_left -= 1;
                            match obtain_cycles_options
                                .obtain_cycles
                                .obtain_cycles(needed_cycles, canister_id)
                                .await
                            {
                                Ok(cycles_obtained) => {
                                    if let Some(record) =
                                        manager.borrow_mut().canisters.get_mut(&canister_id)
                                    {
                                        record.add_deposited_cycles(CyclesBalance::new(
                                            cycles_obtained,
                                            time(),
                                        ));
                                        debug_print(format!(
                                            "Successfully obtained {} cycles for canister {}",
                                            cycles_obtained,
                                            canister_id.to_text()
                                        ));
                                    } else {
                                        debug_print(format!(
                                            "Warning: Obtained {} cycles but canister {} not found in records",
                                            cycles_obtained,
                                            canister_id.to_text()
                                        ));
                                    }
                                    break;
                                }
                                Err(error) => {
                                    debug_print(format!(
                                        "Failed to obtain {} cycles for canister {}, err: {}",
                                        needed_cycles,
                                        canister_id.to_text(),
                                        error.details
                                    ));

                                    if error.can_retry && tries_left > 0 {
                                        debug_print("Retrying to obtain cycles...");
                                        continue;
                                    } else if let Some(record) =
                                        manager.borrow_mut().canisters.get_mut(&canister_id)
                                    {
                                        record.set_funding_failure(
                                            FundingErrorCode::ObtainCyclesFailed,
                                            time(),
                                        );
                                    }
                                    break;
                                }
                            }
                        }
                    } else {
                        if funding_canister_needs_cycles {
                            debug_print(format!("WARNING: Could not top up canister {}. Funding canister is low on cycles.", canister_id.to_text()));
                        }

                        debug_print("WARNING: No top-up method configured for topping up the funding canister. Consider configuring `obtain_cycles_options`.");

                        if let Some(record) = manager.borrow_mut().canisters.get_mut(&canister_id) {
                            record
                                .set_funding_failure(FundingErrorCode::InsufficientCycles, time());
                        }
                    }
                } else {
                    match deposit_cycles(&DepositCyclesArgs { canister_id }, needed_cycles).await {
                        Err(err) => {
                            debug_print(format!(
                                "Failed to fund canister {} with {} cycles, error: {}",
                                canister_id.to_text(),
                                needed_cycles,
                                err,
                            ));

                            if let Some(record) =
                                manager.borrow_mut().canisters.get_mut(&canister_id)
                            {
                                record.set_funding_failure(FundingErrorCode::DepositFailed, time());
                            }
                        }
                        Ok(_) => {
                            debug_print(format!(
                                "Funded canister {} with {} cycles",
                                canister_id.to_text(),
                                needed_cycles
                            ));

                            if let Some(record) =
                                manager.borrow_mut().canisters.get_mut(&canister_id)
                            {
                                record.add_deposited_cycles(CyclesBalance::new(
                                    needed_cycles,
                                    time(),
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Execute funding callback after the canisters have been funded.
        manager.borrow().funding_callback();
    }

    /// Fetches the cycles balance for the provided canisters and calculates the needed cycles to fund them.
    ///
    /// Returns a list of canister ids and the cycles needed to fund them, if any.
    async fn monitor_specified_canisters(
        manager: Rc<RefCell<FundManagerCore>>,
        canisters: &[(CanisterId, Arc<dyn FetchCyclesBalance>)],
    ) -> Vec<(CanisterId, u128)> {
        let mut canisters_to_fund = Vec::new();
        let options = manager.borrow().options().clone();
        let requests = canisters
            .iter()
            .map(|(canister_id, cycles_fetcher)| cycles_fetcher.fetch_cycles_balance(*canister_id));

        let results = futures::future::join_all(requests).await;
        let current_time = time();

        for (i, (canister_id, _)) in canisters.iter().enumerate() {
            match &results[i] {
                Ok(cycles_balance) => {
                    let mut manager_mut = manager.borrow_mut();
                    if let Entry::Occupied(mut entry) = manager_mut.canisters.entry(*canister_id) {
                        let canister_record = entry.get_mut();

                        canister_record
                            .set_cycles(CyclesBalance::new(*cycles_balance, current_time));

                        let needed_cycles = calc_needed_cycles(
                            &canister_record.get_cycles().clone().unwrap_or_default(),
                            canister_record.get_average_consumption() as u128,
                            canister_record
                                .get_strategy()
                                .as_ref()
                                .unwrap_or_else(|| options.strategy()),
                        );

                        if needed_cycles > 0 {
                            canisters_to_fund.push((*canister_id, needed_cycles));
                        }
                    }
                }
                Err(error) => {
                    debug_print(format!(
                        "Failed to fetch cycles balance for canister {}, err: {:?}",
                        canister_id.to_text(),
                        error
                    ));

                    if let Some(record) = manager.borrow_mut().canisters.get_mut(canister_id) {
                        record.set_funding_failure(FundingErrorCode::BalanceCheckFailed, time());
                    }
                }
            }
        }

        canisters_to_fund
    }
}

impl FundManagerCore {
    pub fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(FundManagerCore {
            canisters: HashMap::new(),
            options: FundManagerOptions::default(),
            lock: ProcessExecutionLock::new(),
        }))
    }

    /// Returns the options for the fund manager.
    pub fn options(&self) -> &FundManagerOptions {
        &self.options
    }

    /// Register a canister to be monitored by the fund manager.
    ///
    /// If the canister is already registered, it will be ignored.
    pub fn register(&mut self, canister_id: CanisterId, opts: RegisterOpts) {
        let history_window_size = match &opts.strategy {
            Some(FundStrategy::BelowEstimatedRuntime(estimated_runtime)) => {
                estimated_runtime.min_runtime_secs() / self.options.interval_secs()
            }
            None => match self.options.strategy() {
                FundStrategy::BelowEstimatedRuntime(estimated_runtime) => {
                    estimated_runtime.min_runtime_secs() / self.options.interval_secs()
                }
                _ => 0,
            },
            _ => 0,
        };

        match self.canisters.entry(canister_id) {
            Entry::Vacant(entry) => {
                entry.insert(CanisterRecord::new(
                    opts.cycles_fetcher,
                    opts.strategy,
                    opts.obtain_cycles_options,
                    history_window_size as usize,
                ));
            }
            Entry::Occupied(_) => {
                // The canister is already registered so ignore.
            }
        }
    }

    /// Unregister a canister from being monitored by the fund manager.
    ///
    /// Returns the canister record if it was found.
    pub fn unregister(&mut self, canister_id: CanisterId) -> Option<CanisterRecord> {
        self.canisters.remove(&canister_id)
    }

    /// Executes the funding callback if it is set in the options.
    pub fn funding_callback(&self) {
        if let Some(funding_callback) = self.options.funding_callback() {
            funding_callback(self.canisters.clone());
        }
    }
}

/// Calculates the needed cycles to fund the canister based on the current, previous cycles balance and
/// the used strategy.
fn calc_needed_cycles(
    current: &CyclesBalance,
    estimated_cycles_per_sec: u128,
    strategy: &FundStrategy,
) -> u128 {
    match strategy {
        FundStrategy::Always(cycles) => *cycles,
        FundStrategy::BelowThreshold(threshold) => {
            if current.amount <= threshold.min_cycles() {
                return threshold.fund_cycles();
            }

            0
        }
        FundStrategy::BelowEstimatedRuntime(estimated_runtime) => {
            if estimated_cycles_per_sec == 0 {
                let is_below_threshold = current.amount <= estimated_runtime.fallback_min_cycles();

                // If the current cycles balance is below the threshold, we should fund the canister.
                if is_below_threshold {
                    return estimated_runtime.fallback_fund_cycles();
                }

                return 0;
            }

            // If the current cycles balance is below the min cycles threshold,
            // fund the canister with the fallback cycles amount.
            if current.amount <= estimated_runtime.fallback_min_cycles() {
                return estimated_runtime.fallback_fund_cycles();
            }

            // Fund the canister with the cycles needed to run for the estimated runtime, but cap it to the
            // maximum runtime cycles fund to prevent over-funding.
            let fund_with_cycles = cmp::min(
                estimated_cycles_per_sec
                    .saturating_mul(estimated_runtime.fund_runtime_secs() as u128),
                estimated_runtime.max_runtime_cycles_fund(),
            );

            if current.amount == 0 {
                return fund_with_cycles;
            }

            let estimated_runtime_secs = current.amount / estimated_cycles_per_sec;

            if estimated_runtime_secs <= estimated_runtime.min_runtime_secs() as u128 {
                return fund_with_cycles;
            }

            0
        }
    }
}

impl Drop for FundManager {
    /// Stops the fund manager tracking when the fund manager is dropped.
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use tests::options::{CyclesThreshold, EstimatedRuntime};

    use super::*;

    #[test]
    fn test_calc_needed_cycles() {
        let current = CyclesBalance::new(50, Duration::from_secs(10).as_nanos() as u64);

        let strategy = FundStrategy::Always(1000);
        assert_eq!(calc_needed_cycles(&current, 0, &strategy), 1000);

        let strategy = FundStrategy::BelowThreshold(
            CyclesThreshold::new()
                .with_min_cycles(50)
                .with_fund_cycles(100),
        );
        assert_eq!(calc_needed_cycles(&current, 0, &strategy), 100);

        let strategy = FundStrategy::BelowThreshold(
            CyclesThreshold::new()
                .with_min_cycles(49)
                .with_fund_cycles(100),
        );
        assert_eq!(calc_needed_cycles(&current, 0, &strategy), 0);

        let strategy = FundStrategy::BelowEstimatedRuntime(
            EstimatedRuntime::new()
                .with_min_runtime_secs(10)
                .with_fund_runtime_secs(10)
                .with_fallback_min_cycles(0),
        );
        assert_eq!(calc_needed_cycles(&current, 5, &strategy), 50);

        let strategy = FundStrategy::BelowEstimatedRuntime(
            EstimatedRuntime::new()
                .with_min_runtime_secs(10)
                .with_fund_runtime_secs(10)
                .with_max_runtime_cycles_fund(30)
                .with_fallback_min_cycles(0),
        );
        assert_eq!(calc_needed_cycles(&current, 5, &strategy), 30);
    }

    #[test]
    fn test_calc_needed_cycles_zero_previous_cycles() {
        let current = CyclesBalance::new(50, Duration::from_secs(10).as_nanos() as u64);

        let strategy = FundStrategy::BelowEstimatedRuntime(
            EstimatedRuntime::new()
                .with_min_runtime_secs(10)
                .with_fund_runtime_secs(10)
                .with_fallback_min_cycles(50)
                .with_fallback_fund_cycles(100),
        );
        assert_eq!(calc_needed_cycles(&current, 0, &strategy), 100);

        let strategy = FundStrategy::BelowEstimatedRuntime(
            EstimatedRuntime::new()
                .with_min_runtime_secs(10)
                .with_fund_runtime_secs(10)
                .with_fallback_min_cycles(49)
                .with_fallback_fund_cycles(100),
        );
        assert_eq!(calc_needed_cycles(&current, 0, &strategy), 0);
    }

    #[test]
    fn test_calc_needed_cycles_zero_current_amount() {
        let current = CyclesBalance::new(0, Duration::from_secs(10).as_nanos() as u64);

        let strategy = FundStrategy::BelowEstimatedRuntime(
            EstimatedRuntime::new()
                .with_min_runtime_secs(10)
                .with_fund_runtime_secs(10)
                .with_fallback_min_cycles(50)
                .with_fallback_fund_cycles(100),
        );
        assert_eq!(calc_needed_cycles(&current, 0, &strategy), 100);

        let strategy = FundStrategy::BelowThreshold(
            CyclesThreshold::new()
                .with_min_cycles(0)
                .with_fund_cycles(100),
        );

        assert_eq!(calc_needed_cycles(&current, 0, &strategy), 100);
    }
}
