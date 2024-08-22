use crate::setup::setup_new_env;
use crate::utils::{advance_time_to_burn_cycles, controller_test_id};
use crate::TestEnv;

#[test]
fn successfuly_monitors_funded_canister_and_tops_up() {
    let TestEnv {
        env, canister_ids, ..
    } = setup_new_env();

    let top_up_should_happen_when_cycles_below = 125_000_000_000;
    advance_time_to_burn_cycles(
        &env,
        controller_test_id(),
        canister_ids.funded_canister,
        top_up_should_happen_when_cycles_below + 5_000_000_000,
    );

    let funded_canister_cycles_balance = env.cycle_balance(canister_ids.funded_canister);
    if funded_canister_cycles_balance <= top_up_should_happen_when_cycles_below {
        panic!("Upgrader cycles balance is too low to run the test");
    }

    // wait for the fund manager to complete and release the lock
    for _ in 0..2 {
        env.tick();
    }

    advance_time_to_burn_cycles(
        &env,
        controller_test_id(),
        canister_ids.funded_canister,
        top_up_should_happen_when_cycles_below - 5_000_000_000,
    );

    // wait for the fund manager to complete and top up the cycles
    for _ in 0..2 {
        env.tick();
    }

    assert!(env.cycle_balance(canister_ids.funded_canister) > 250_000_000_000);
}
