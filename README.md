[![Internet Computer portal](https://img.shields.io/badge/InternetComputer-grey?logo=internet%20computer&style=for-the-badge)](https://internetcomputer.org)
[![DFinity Forum](https://img.shields.io/badge/help-post%20on%20forum.dfinity.org-blue?style=for-the-badge)](https://forum.dfinity.org/)
[![GitHub license](https://img.shields.io/badge/license-Apache%202.0-blue.svg?logo=apache&style=for-the-badge)](LICENSE)


# canfund

Welcome to **`canfund`**! This library provides automated cycles management for canisters on the Internet Computer (IC). `canfund` helps ensure your canisters have sufficient cycles by automating the process of balance checking, funding, and cycle-minting based on configurable rules.

## Features

- **Automated Cycles Management**: Automatically manages cycles for specified canisters, including the canister running `canfund`, according to predefined rules.
- **Periodic Balance Monitoring**: `canfund` periodically checks cycle balances using ICP timers, ensuring your canisters stay adequately funded.
- **Optional Cycle Minting**: If funding canister lacks sufficient cycles, `canfund` can mint new cycles from ICP.
- **Configurable Funding Strategies**: Offers three customizable strategies for managing cycles, allowing you to tailor it to specific needs.
- **Configurable Balance Fetching**: Supports multiple methods for fetching the cycle balance of canisters during registration, accommodating different levels of access and canister configurations.
- **Funding Callback**: Canfund supports providing a callback function that is triggered after each funding round, which can be used for monitoring or logging purposes.

## Table of Contents

- [Installation](#installation)
- [Usage](#usage)
  - [Configuration](#configuration)
  - [Canister Registration](#canister-registration)
  - [Funding Strategies](#funding-strategies)
  - [Minting Cycles](#obtaining-cycles)
  - [Funding Callback](#funding-callback)
- [Examples](#examples)
- [License](#license)

## Installation

To integrate `canfund` into your Internet Computer project, add the library as a dependency:

```bash
cargo install `canfund`
```

Or alternatively manually define the dependency in your `Cargo.toml` file:

```json
{
  "dependencies": {
    "canfund": {
      "git": "https://github.com/dfinity/canfund",
      "version": "0.1.0"
    }
  }
}
```

Then, use `canfund` in your canister code:

```rust,ignore
use canfund;
```

## Usage

### Configuration

To use `canfund`, configure it with rules for managing cycles for your canisters. The configuration includes:

- **Target Canisters**: Specify the canisters that should be managed.
- **Funding Rules**: Set the thresholds and strategies that trigger additional cycle funding.
- **Cycle Minting**: Enable or disable the minting of cycles from the ICP balance when necessary.


### Canister Registration

Each canister that you want to fund using `canfund` must be registered. During registration, you must specify the method by which the canister's cycle balance will be fetched. `canfund` supports two different balance-fetching methods:

1. **FetchCyclesBalanceFromCanisterStatus**: Retrieves the cycle balance from the response of an update call to a canister, typically using the `canister_status` method on the management canister. It can also work with responses from other canisters, such as a blackhole, or from self-exposed methods on any canister, as long as the response follows the [CanisterStatusResponse specification](https://docs.rs/ic-cdk/0.15.0/ic_cdk/api/management_canister/main/struct.CanisterStatusResponse.html). 

   This method is only applicable if the funding canister has the required permissions to invoke the target canister’s method, such as being a controller when interacting with the management canister.

   ```rust,ignore
   // By default, using the management canister (funding canister is a controller)
   let fetcher = FetchCyclesBalanceFromCanisterStatus::new();
   
   // Using the blackhole canister (Blackhole is a controller)
   let fetcher = FetchCyclesBalanceFromCanisterStatus::new()
           .with_proxy(Principal::from_text("e3mmv-5qaaa-aaaah-aadma-cai").unwrap()));
   );
   
   // Using a custom method of a canister (method is public)
   let fetcher = FetchCyclesBalanceFromCanisterStatus::new()
           .with_proxy(Principal::from_text("ml52i-qqaaa-aaaar-qaaba-cai").unwrap())
           .with_method("get_canister_status".to_string());
   );
   ```
   **Note:** This method is currently the only one that accounts for the freezing threshold of the funded canister. As a result, the runtime and threshold-based funding strategies (explained below) are calculated based on when the canister becomes frozen. This behavior does not apply to the Blackhole proxy, as it currently does not return the required _idle_cycles_burned_per_day_ field in its response.


3. **FetchCyclesBalanceFromPrometheusMetrics**: Fetches the cycle balance by leveraging Prometheus metrics exposed by the canister through an HTTP endpoint.

   ```rust,ignore
   let fetcher = FetchCyclesBalanceFromPrometheusMetrics::new(
       "/metrics".to_string(), // path
       "canister_cycles_balance".to_string(), // metric name
   );
   ```

To register a canister with selected `fetcher`:

```rust,ignore
fund_manager.register(
    Principal::from_text("funded_canister_id").unwrap(),
    RegisterOpts::new().with_cycles_fetcher(
        Arc::new(fetcher)
    ),
);
```

### Funding Strategies

`canfund` provides three distinct strategies for funding your canisters:

1. **BelowThreshold (Default)**: Funds the canister when its cycle balance falls below a predefined threshold.

   ```rust,ignore
   let strategy = FundStrategy::BelowThreshold(
       CyclesThreshold::new()
           .with_min_cycles(125_000_000_000)
           .with_fund_cycles(250_000_000_000)
   );
   ```

2. **BelowEstimatedRuntime**: Funds the canister based on an estimated runtime in seconds. This strategy calculates the required cycles to keep the canister running for the specified duration.

   ```rust,ignore
   let strategy = FundStrategy::BelowEstimatedRuntime(
       EstimatedRuntime::new()
           .with_min_runtime_secs(2 * 24 * 60 * 60) // 2 day
           .with_fund_runtime_secs(3 * 24 * 60 * 60) // 3 days
           .with_max_runtime_cycles_fund(1_000_000_000_000)
           .with_fallback_min_cycles(125_000_000_000)
           .with_fallback_fund_cycles(250_000_000_000),
   );
   ```

3. **Always**: Funds the canister at a fixed interval with a specified amount of cycles, regardless of the current balance.

   ```rust,ignore
   let strategy = FundStrategy::Always(1_000);
   ```

### Obtaining Cycles

`canfund` can be configured to obtain cycles if your canister requires more cycles than it currently holds. This is achieved by interacting with the ICP Ledger and the Cycles Minting Canister (CMC) or by withdrawing cycles from the cycles ledger. Only one strategy can be set at a time.

#### Minting Cycles
To enable minting cycles, you must provide the necessary configuration to allow `canfund` to mint cycles. This configuration can also be set for each registered canister to override the global configuration.

```rust,ignore
let obtain_cycles_config = ObtainCyclesOptions {
    obtain_cycles: Arc::new(MintCycles {
        ledger: Arc::new(IcLedgerCanister::new(MAINNET_LEDGER_CANISTER_ID)),
        cmc: Arc::new(IcCyclesMintingCanister::new(
            MAINNET_CYCLES_MINTING_CANISTER_ID,
        )),
        from_subaccount: Subaccount::from(DEFAULT_SUBACCOUNT),
    }),
};

funding_options.with_obtain_cycles_options(Some(obtain_cycles_config));
```

With this configuration, `canfund` will periodically check the ICP balance and mint new cycles as needed to ensure that your canisters remain adequately funded.

#### Withdrawing Cycles

Alternatively, `canfund` can be configured to withdraw cycles from the [cycles ledger](https://dashboard.internetcomputer.org/canister/um5iw-rqaaa-aaaaq-qaaba-cai). This is achieved by interacting with the Cycles Ledger using the `WithdrawFromCyclesLedger` struct.

To enable this feature, you must provide the necessary configuration to allow `canfund` to withdraw cycles. This configuration can also be set for each registered canister to override the global configuration.

```rust,ignore
let obtain_cycles_config = ObtainCyclesOptions {
    obtain_cycles: Arc::new(WithdrawFromCyclesLedger {
        ledger: Arc::new(CyclesLedgerCanister::new(MAINNET_CYCLES_LEDGER_CANISTER_ID)),
        from_subaccount: None,
    }),
};

funding_options.with_obtain_cycles_options(Some(obtain_cycles_config));
```

With this configuration, canfund will periodically check the cycles balance and withdraw cycles as needed to ensure that your canisters remain adequately funded.

### Funding Callback

`canfund` also supports registering a callback function that will be triggered after a funding round is completed. This feature is useful for monitoring and logging purposes, allowing you to capture and read data such as the remaining cycle balances and total cycles deposited per canister.

Example of registering a callback:

```rust,ignore
let funding_config = FundManagerOptions::new()
    .with_funding_callback(Rc::new(|canister_records| {
        // custom monitoring || logging logic
    })
); 
```

Funding callback can also be used to handle funding failures. For example, if the funding canister does not have enough cycles to fund the target canister, the callback can be used to notify the user or trigger a different action.

Funding failures can be accessed through the `canister_records` parameter, which contains an optional `funding_failure` enum field. This field will be `Some` if the latest funding operation failed, and `None` otherwise.



### Initialization

Initialize `canfund` with your configuration:

```rust,ignore
let funding_config = FundManagerOptions::new()
        .with_interval_secs(12 * 60 * 60) // check twice a day
        .with_strategy(strategy); 

fund_manager.with_options(funding_config);
```

## Examples

Here's a basic example of using `canfund` for automated cycles management:

```rust,ignore
use canfund::{manager::{options::{CyclesThreshold, FundManagerOptions, FundStrategy}, RegisterOpts}, operations::fetch::FetchCyclesBalanceFromCanisterStatus, FundManager};

#[ic_cdk_macros::init]
fn initialize() {
    let mut fund_manager = FundManager::new();

    let funding_config = FundManagerOptions::new()
        .with_interval_secs(12 * 60 * 60)
        .with_strategy(FundStrategy::BelowThreshold(
            CyclesThreshold::new()
                .with_min_cycles(125_000_000_000)
                .with_fund_cycles(250_000_000_000),
    ));        

    fund_manager.with_options(funding_config);
    
    fund_manager.register(
        Principal::from_text("funded_canister_id").unwrap(),
        RegisterOpts::new().with_cycles_fetcher(
            Arc::new(FetchCyclesBalanceFromCanisterStatus::new())
        ),
    );

    // Note: the funding canister is NOT automatically registered
    fund_manager.register(
        id(),
        RegisterOpts::new(),
    );

    fund_manager.start();
}
```

Full examples can be found in the examples folder for [simple](examples/simple_funding/src/lib.rs) and [advanced](examples/advanced_funding/src/lib.rs) funding configurations.

## License

This project is licensed under the Apache 2.0 License - see the [LICENSE](LICENSE) file for details.

---

This README provides an overview of `canfund`, along with installation instructions and examples to help you get started. Contributions and feedback are welcome!