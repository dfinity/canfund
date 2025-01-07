use std::time::Duration;

use ic_ledger_types::{AccountIdentifier, DEFAULT_SUBACCOUNT};

use crate::interfaces::{
    get_icp_account_balance, query_deposited_cycles, send_icp_to_account, ICP,
};
use crate::setup::{
    create_canister_with_cycles, install_advanced_funding_canister, install_funded_canister,
    install_simple_funding_canister, setup_new_env,
};
use crate::utils::{advance_time_to_burn_cycles, controller_test_id};
use crate::TestEnv;

#[test]
fn successfully_monitors_funded_canister_and_tops_up() {
    let TestEnv {
        env, controller, ..
    } = setup_new_env();

    let top_up_should_happen_when_cycles_below = 400_000_000_000;
    let funded_canister_id = install_funded_canister(
        &env,
        controller,
        top_up_should_happen_when_cycles_below + 5_000_000_000,
    );

    let funding_canister_id = create_canister_with_cycles(&env, controller, 100_000_000_000_000);

    // add the funding canister as a controller to the funded canister so that it can make canister_status calls
    env.set_controllers(
        funded_canister_id,
        Some(controller),
        vec![controller, funding_canister_id],
    )
    .unwrap();

    // install the funding canister to start monitoring the funded canister
    install_simple_funding_canister(
        &env,
        controller,
        funding_canister_id,
        vec![funded_canister_id],
    );

    let funded_canister_cycles_balance = env.cycle_balance(funded_canister_id);
    if funded_canister_cycles_balance <= top_up_should_happen_when_cycles_below {
        panic!("Funded canister's cycles balance is too low to run the test");
    }

    // wait for the fund manager to complete and release the lock
    env.tick();
    env.tick();

    advance_time_to_burn_cycles(
        &env,
        controller_test_id(),
        funded_canister_id,
        top_up_should_happen_when_cycles_below - 5_000_000_000,
    );

    env.tick();
    env.tick();

    assert!(env.cycle_balance(funded_canister_id) > 500_000_000_000);
}

#[test]
fn successfully_stores_funding_data() {
    let TestEnv {
        env, controller, ..
    } = setup_new_env();

    let top_up_should_happen_when_cycles_below = 400_000_000_000;
    let funding_canister_id = create_canister_with_cycles(&env, controller, 100_000_000_000_000);

    let funded_canister_id = install_funded_canister(
        &env,
        controller,
        top_up_should_happen_when_cycles_below + 5_000_000_000,
    );

    env.set_controllers(
        funded_canister_id,
        Some(controller),
        vec![controller, funding_canister_id],
    )
    .unwrap();

    install_advanced_funding_canister(
        &env,
        controller,
        funding_canister_id,
        vec![funded_canister_id],
    );

    let funded_canister_cycles_balance = env.cycle_balance(funded_canister_id);
    if funded_canister_cycles_balance <= top_up_should_happen_when_cycles_below {
        panic!("Funded canister's cycles balance is too low to run the test");
    }

    // wait for the fund manager to complete and release the lock
    env.tick();
    env.tick();

    advance_time_to_burn_cycles(
        &env,
        controller_test_id(),
        funded_canister_id,
        top_up_should_happen_when_cycles_below - 5_000_000_000,
    );

    env.tick();
    env.tick();

    let deposited_cycles = query_deposited_cycles(&env, funding_canister_id);

    // check if the deposited cycles are stored correctly for the funding canister and the funded canister
    for record in &deposited_cycles {
        if record.canister_id == funded_canister_id {
            assert_eq!(record.deposited_cycles, 250_000_000_000);
        } else {
            assert_eq!(record.deposited_cycles, 0);
        }
    }

    advance_time_to_burn_cycles(
        &env,
        controller_test_id(),
        funded_canister_id,
        top_up_should_happen_when_cycles_below - 5_000_000_000,
    );

    env.tick();
    env.tick();

    let deposited_cycles = query_deposited_cycles(&env, funding_canister_id);

    // check if the deposited cycles are stored correctly for the funding canister and the funded canister
    for record in &deposited_cycles {
        if record.canister_id == funded_canister_id {
            assert_eq!(record.deposited_cycles, 500_000_000_000);
        } else {
            assert_eq!(record.deposited_cycles, 0);
        }
    }
}

#[test]
fn can_mint_cycles_to_top_up_self() {
    let TestEnv {
        env, controller, ..
    } = setup_new_env();

    let advanced_funding_canister_id =
        create_canister_with_cycles(&env, controller, 400_000_000_000);

    // install the funding canister to start monitoring itself and mint cycles
    install_advanced_funding_canister(&env, controller, advanced_funding_canister_id, vec![]);

    // 4 ticks are important to ensure ICP is not converted to cycles immediately when acquired below
    env.tick();
    env.tick();
    env.tick();
    env.tick();

    let account_id = AccountIdentifier::new(&advanced_funding_canister_id, &DEFAULT_SUBACCOUNT);
    send_icp_to_account(&env, controller, account_id, 100 * ICP, 0, None).unwrap();
    let pre_cycle_balance = env.cycle_balance(advanced_funding_canister_id);

    let pre_account_balance = get_icp_account_balance(&env, account_id);
    assert_eq!(pre_account_balance, 100 * ICP);

    env.tick();
    env.advance_time(Duration::from_secs(24 * 60 * 60));
    env.tick();
    env.tick();
    env.tick();
    env.tick();
    env.tick();
    env.tick();

    let post_account_balance = get_icp_account_balance(&env, account_id);
    let post_cycle_balance = env.cycle_balance(advanced_funding_canister_id);

    assert_ne!(post_account_balance, 100 * ICP);
    assert!(post_account_balance < pre_account_balance);
    assert!(post_cycle_balance > pre_cycle_balance);

    // assert that while we lose some cycles during the process, it'll be roughly what we expect
    assert!(
        post_cycle_balance - pre_cycle_balance > 749_000_000_000
            && post_cycle_balance - pre_cycle_balance < 750_000_000_000
    );
}
