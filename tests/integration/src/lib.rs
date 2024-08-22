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
    pub icp_index: Principal,
    pub cycles_minting_canister: Principal,
    pub funding_canister: Principal,
    pub funded_canister: Principal,
}
