## 0.4.0 (2024-11-19)

### BREAKING CHANGE

- There is no observed use-case to disable minting for topping up the funding canister
- The funding cunister must be registered explicitly with this update

### Feat

- allow minting strategy per canister

### Refactor

- remove self top-up configuration for funding canister
- **manager**: remove implicit funding canister registration

## 0.3.0 (2024-11-01)

### Feat

- support custom proxy method
- allow proxy/blackhole for canister status
- **fetch**: consider freezing threshold in mngmt canister balance fetch
- add balance check history and improve average consumption calculation

### Refactor

- remove outdated FetchOwnCyclesBalance cycle fetching approach

## 0.2.0 (2024-09-23)


### 🚀 Features

- per-canister funding override functionality


### ❤️  Thank You

- Jan Hrubes

## 0.1.0 (2024-09-10)


### 🚀 Features

- add funding callback and deposited cycles store


### ❤️  Thank You

- Jan Hrubes

## 0.0.2-alpha.2 (2024-05-12)


### 🚀 Features

- create canfund lib to monitor and fund canisters


### ❤️  Thank You

- Kepler Vital
