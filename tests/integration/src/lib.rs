#![cfg(test)]

use candid::Principal;
use pocket_ic::PocketIc;

mod cycles_monitor_tests;
mod interfaces;
mod setup;
mod utils;

pub struct TestEnv {
    pub env: PocketIc,
    pub canister_ids: CanisterIds,
    pub controller: Principal,
    pub minter: Principal,
}

#[derive(Debug)]
pub struct CanisterIds {
    pub icp_ledger: Principal,
    pub cycles_minting_canister: Principal,
    pub advanced_funding_canister: Principal, // Performs self-funding && advanced funding operations
    pub funding_canister: Principal, // Performs simple threshold-based funding opration
    pub funded_canister: Principal,
}
