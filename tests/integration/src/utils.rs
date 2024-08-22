use candid::{CandidType, Deserialize, Principal};
use pocket_ic::PocketIc;
use std::time::Duration;

// Simple counter canister used to burn cycles in tests
pub const COUNTER_WAT: &str = r#"
    (module
        (import "ic0" "debug_print"
            (func $debug_print (param i32 i32)))
        (import "ic0" "msg_cycles_available"
            (func $ic0_msg_cycles_available (result i64)))
        (import "ic0" "msg_cycles_accept"
            (func $ic0_msg_cycles_accept (param $pages i64) (result i64)))
        (import "ic0" "msg_arg_data_copy"
            (func $msg_arg_data_copy (param i32 i32 i32)))
        (import "ic0" "msg_reply" (func $msg_reply))
        (import "ic0" "msg_reply_data_append"
            (func $msg_reply_data_append (param i32 i32)))
        (import "ic0" "stable_grow"
            (func $ic0_stable_grow (param $pages i32) (result i32)))
        (import "ic0" "stable_read"
            (func $ic0_stable_read (param $dst i32) (param $offset i32) (param $size i32)))
        (import "ic0" "stable_write"
            (func $ic0_stable_write (param $offset i32) (param $src i32) (param $size i32)))
        (func $init
            (drop (call $ic0_stable_grow (i32.const 1))))
        (func $set
            (call $msg_arg_data_copy (i32.const 0) (i32.const 0) (i32.const 4))
            (call $ic0_stable_write (i32.const 0) (i32.const 0) (i32.const 4))
            (drop (call $ic0_msg_cycles_accept (call $ic0_msg_cycles_available)))
            (call $msg_reply_data_append
                (i32.const 100) ;; the value at heap[100] encoding '(variant {Ok = "good"})' in candid
                (i32.const 19)) ;; length
            (call $msg_reply))
        (func $bad
            (call $doinc)
            (drop (call $ic0_msg_cycles_accept (call $ic0_msg_cycles_available)))
            (call $msg_reply_data_append
                (i32.const 200) ;; the value at heap[200] encoding '(variant {Err = "bad"})' in candid
                (i32.const 19)) ;; length
            (call $msg_reply))
        (func $inc
            (call $doinc)
            (drop (call $ic0_msg_cycles_accept (call $ic0_msg_cycles_available)))
            (call $msg_reply_data_append
                (i32.const 300) ;; the value at heap[300] encoding '(variant {Ok = "valid"})' in candid
                (i32.const 20)) ;; length
            (call $msg_reply))
        (func $doinc
            (call $ic0_stable_read (i32.const 0) (i32.const 0) (i32.const 4))
            (i32.store
                (i32.const 0)
                (i32.add (i32.load (i32.const 0)) (i32.const 2)))
            (call $ic0_stable_write (i32.const 0) (i32.const 0) (i32.const 4)))
        (func $read
            (call $ic0_stable_read (i32.const 0) (i32.const 0) (i32.const 4))
            (call $msg_reply_data_append
                (i32.const 0) ;; the counter from heap[0]
                (i32.const 4)) ;; length
            (call $msg_reply))
        (memory $memory 1)
        (data (i32.const 100) "\44\49\44\4c\01\6b\01\bc\8a\01\71\01\00\00\04\67\6f\6f\64")
        (data (i32.const 200) "\44\49\44\4c\01\6b\01\c5\fe\d2\01\71\01\00\00\03\62\61\64")
        (data (i32.const 300) "\44\49\44\4c\01\6b\01\bc\8a\01\71\01\00\00\05\76\61\6c\69\64")
        (export "canister_init" (func $init))
        (export "canister_post_upgrade" (func $doinc))
        (export "canister_query read" (func $read))
        (export "canister_update set" (func $set))
        (export "canister_update bad" (func $bad))
        (export "canister_update inc" (func $inc))
    )"#;

pub fn controller_test_id() -> Principal {
    let mut bytes = 0_u64.to_le_bytes().to_vec();
    bytes.push(0xfd); // internal marker for controller test id
    bytes.push(0x01); // marker for opaque ids
    Principal::from_slice(&bytes)
}

pub fn minter_test_id() -> Principal {
    let mut bytes = 0_u64.to_le_bytes().to_vec();
    bytes.push(0xfc); // internal marker for minter test id
    bytes.push(0x01); // marker for opaque ids
    Principal::from_slice(&bytes)
}

#[derive(CandidType, serde::Serialize, Deserialize, Clone, Debug)]
pub struct SystemInfoDTO {
    pub name: String,
    pub version: String,
    pub upgrader_id: Principal,
    pub cycles: u64,
}

#[derive(CandidType, serde::Serialize, Deserialize, Clone, Debug)]
pub struct SystemInfoResponse {
    pub system: SystemInfoDTO,
}

pub fn advance_time_to_burn_cycles(
    env: &PocketIc,
    sender: Principal,
    canister_id: Principal,
    target_cycles: u128,
) {
    if env.cycle_balance(canister_id) < target_cycles {
        return;
    }

    // Stops to prevent side effects like timers or heartbeats
    env.stop_canister(canister_id, Some(sender)).unwrap();
    let canister_cycles = env.cycle_balance(canister_id);
    let jump_secs = 10;
    let cycles_to_burn = canister_cycles.saturating_sub(target_cycles);

    // advance time one step to get an estimate of the burned cycles per advance
    env.advance_time(Duration::from_secs(jump_secs));
    env.tick();

    let burned_cycles = canister_cycles.saturating_sub(env.cycle_balance(canister_id));
    if burned_cycles == 0 {
        panic!("Canister did not burn any cycles, this should not happen.");
    }

    // advance time to burn the remaining cycles
    let advance_times_to_burn_cycles = (cycles_to_burn + burned_cycles - 1) / burned_cycles;
    let burn_duration = Duration::from_secs(jump_secs * advance_times_to_burn_cycles as u64);
    env.advance_time(burn_duration);
    env.tick();

    if target_cycles > 0 {
        // restart the canister if it has some cycles remaining
        env.start_canister(canister_id, Some(sender)).unwrap();
    }

    // need at least 2 ticks
    env.tick();
    env.tick();

    // adds cycles to be as close as possible to the target
    let canister_cycles = env.cycle_balance(canister_id);
    let add_cycles = target_cycles.saturating_sub(canister_cycles);
    if add_cycles > 0 {
        env.add_cycles(canister_id, add_cycles);
    }
}
