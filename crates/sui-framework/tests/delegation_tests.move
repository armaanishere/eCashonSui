// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module sui::delegation_tests {
    use sui::coin;
    use sui::test_scenario::{Self, Scenario};
    use sui::sui_system::{Self, SuiSystemState};
    use sui::staking_pool::{Self, StakedSui};
    use sui::test_utils::assert_eq;
    use sui::validator_set;
    use std::vector;


    use sui::governance_test_utils::{
        Self,
        add_validator,
        add_validator_candidate,
        advance_epoch,
        advance_epoch_with_reward_amounts,
        create_validator_for_testing,
        create_sui_system_state_for_testing,
        delegate_to,
        remove_validator,
        total_sui_balance,
        undelegate,
    };

    const VALIDATOR_ADDR_1: address = @0x1;
    const VALIDATOR_ADDR_2: address = @0x2;

    const DELEGATOR_ADDR_1: address = @0x42;
    const DELEGATOR_ADDR_2: address = @0x43;
    const DELEGATOR_ADDR_3: address = @0x44;

    const NEW_VALIDATOR_ADDR: address = @0x1a4623343cd42be47d67314fce0ad042f3c82685544bc91d8c11d24e74ba7357;
    const NEW_VALIDATOR_PUBKEY: vector<u8> = x"99f25ef61f8032b914636460982c5cc6f134ef1ddae76657f2cbfec1ebfc8d097374080df6fcf0dcb8bc4b0d8e0af5d80ebbff2b4c599f54f42d6312dfc314276078c1cc347ebbbec5198be258513f386b930d02c2749a803e2330955ebd1a10";
    const NEW_VALIDATOR_POP: vector<u8> = x"8080980b89554e7f03b625ba4104d05d19b523a737e2d09a69d4498a1bcac154fcb29f6334b7e8b99b8f3aa95153232d";

    #[test]
    fun test_split_join_staked_sui() {
        let scenario_val = test_scenario::begin(DELEGATOR_ADDR_1);
        let scenario = &mut scenario_val;
        // All this is just to generate a dummy StakedSui object to split and join later
        set_up_sui_system_state(scenario);
        governance_test_utils::delegate_to(DELEGATOR_ADDR_1, VALIDATOR_ADDR_1, 60, scenario);

        test_scenario::next_tx(scenario, DELEGATOR_ADDR_1);
        {
            let staked_sui = test_scenario::take_from_sender<StakedSui>(scenario);
            let ctx = test_scenario::ctx(scenario);
            staking_pool::split_staked_sui(&mut staked_sui, 20, ctx);
            test_scenario::return_to_sender(scenario, staked_sui);
        };

        // Verify the correctness of the split and send the join txn
        test_scenario::next_tx(scenario, DELEGATOR_ADDR_1);
        {
            let staked_sui_ids = test_scenario::ids_for_sender<StakedSui>(scenario);
            assert!(vector::length(&staked_sui_ids) == 2, 101); // staked sui split to 2 coins

            let part1 = test_scenario::take_from_sender_by_id<StakedSui>(scenario, *vector::borrow(&staked_sui_ids, 0));
            let part2 = test_scenario::take_from_sender_by_id<StakedSui>(scenario, *vector::borrow(&staked_sui_ids, 1));

            let amount1 = staking_pool::staked_sui_amount(&part1);
            let amount2 = staking_pool::staked_sui_amount(&part2);
            assert!(amount1 == 20 || amount1 == 40, 102);
            assert!(amount2 == 20 || amount2 == 40, 103);
            assert!(amount1 + amount2 == 60, 104);

            staking_pool::join_staked_sui(&mut part1, part2);
            assert!(staking_pool::staked_sui_amount(&part1) == 60, 105);
            test_scenario::return_to_sender(scenario, part1);
        };
        test_scenario::end(scenario_val);
    }

    #[test]
    #[expected_failure(abort_code = staking_pool::EIncompatibleStakedSui)]
    fun test_join_different_epochs() {
        let scenario_val = test_scenario::begin(DELEGATOR_ADDR_1);
        let scenario = &mut scenario_val;
        set_up_sui_system_state(scenario);
        // Create two instances of staked sui w/ different epoch activations
        governance_test_utils::delegate_to(DELEGATOR_ADDR_1, VALIDATOR_ADDR_1, 60, scenario);
        governance_test_utils::advance_epoch(scenario);
        governance_test_utils::delegate_to(DELEGATOR_ADDR_1, VALIDATOR_ADDR_1, 60, scenario);

        // Verify that these cannot be merged
        test_scenario::next_tx(scenario, DELEGATOR_ADDR_1);
        {
            let staked_sui_ids = test_scenario::ids_for_sender<StakedSui>(scenario);
            let part1 = test_scenario::take_from_sender_by_id<StakedSui>(scenario, *vector::borrow(&staked_sui_ids, 0));
            let part2 = test_scenario::take_from_sender_by_id<StakedSui>(scenario, *vector::borrow(&staked_sui_ids, 1));

            staking_pool::join_staked_sui(&mut part1, part2);

            test_scenario::return_to_sender(scenario, part1);
        };
        test_scenario::end(scenario_val);
    }

    #[test]
    #[expected_failure(abort_code = staking_pool::EIncompatibleStakedSui)]
    fun test_join_different_locked_coins() {
        let scenario_val = test_scenario::begin(DELEGATOR_ADDR_1);
        let scenario = &mut scenario_val;
        set_up_sui_system_state(scenario);
        // Create staked sui w/ locked coin and regular staked sui
        governance_test_utils::delegate_to(DELEGATOR_ADDR_1, VALIDATOR_ADDR_1, 60, scenario);
        governance_test_utils::delegate_locked_to(DELEGATOR_ADDR_1, VALIDATOR_ADDR_1, 60, 2, scenario);

        // Verify that these cannot be merged
        test_scenario::next_tx(scenario, DELEGATOR_ADDR_1);
        {
            let staked_sui_ids = test_scenario::ids_for_sender<StakedSui>(scenario);
            let part1 = test_scenario::take_from_sender_by_id<StakedSui>(scenario, *vector::borrow(&staked_sui_ids, 0));
            let part2 = test_scenario::take_from_sender_by_id<StakedSui>(scenario, *vector::borrow(&staked_sui_ids, 1));

            staking_pool::join_staked_sui(&mut part1, part2);

            test_scenario::return_to_sender(scenario, part1);
        };
        test_scenario::end(scenario_val);
    }

    #[test]
    fun test_add_remove_delegation_flow() {
        let scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;
        set_up_sui_system_state(scenario);

        test_scenario::next_tx(scenario, DELEGATOR_ADDR_1);
        {
            let system_state = test_scenario::take_shared<SuiSystemState>(scenario);
            let system_state_mut_ref = &mut system_state;

            let ctx = test_scenario::ctx(scenario);

            // Create a delegation to VALIDATOR_ADDR_1.
            sui_system::request_add_delegation(
                system_state_mut_ref, coin::mint_for_testing(60, ctx), VALIDATOR_ADDR_1, ctx);

            assert!(sui_system::validator_stake_amount(system_state_mut_ref, VALIDATOR_ADDR_1) == 100, 101);
            assert!(sui_system::validator_stake_amount(system_state_mut_ref, VALIDATOR_ADDR_2) == 100, 102);

            test_scenario::return_shared(system_state);
        };

        governance_test_utils::advance_epoch(scenario);

        test_scenario::next_tx(scenario, DELEGATOR_ADDR_1);
        {

            let staked_sui = test_scenario::take_from_sender<StakedSui>(scenario);
            assert!(staking_pool::staked_sui_amount(&staked_sui) == 60, 105);


            let system_state = test_scenario::take_shared<SuiSystemState>(scenario);
            let system_state_mut_ref = &mut system_state;

            assert!(sui_system::validator_stake_amount(system_state_mut_ref, VALIDATOR_ADDR_1) == 160, 103);
            assert!(sui_system::validator_stake_amount(system_state_mut_ref, VALIDATOR_ADDR_2) == 100, 104);

            let ctx = test_scenario::ctx(scenario);

            // Undelegate from VALIDATOR_ADDR_1
            sui_system::request_withdraw_delegation(system_state_mut_ref, staked_sui, ctx);

            assert!(sui_system::validator_stake_amount(system_state_mut_ref, VALIDATOR_ADDR_1) == 160, 107);
            test_scenario::return_shared(system_state);
        };

        governance_test_utils::advance_epoch(scenario);

        test_scenario::next_tx(scenario, DELEGATOR_ADDR_1);
        {
            let system_state = test_scenario::take_shared<SuiSystemState>(scenario);
            assert!(sui_system::validator_stake_amount(&mut system_state, VALIDATOR_ADDR_1) == 100, 107);
            test_scenario::return_shared(system_state);
        };
        test_scenario::end(scenario_val);
    }

    #[test]
    fun test_remove_delegation_post_active_flow_no_rewards() {
        test_remove_delegation_post_active_flow(false)
    }

    #[test]
    fun test_remove_delegation_post_active_flow_with_rewards() {
        test_remove_delegation_post_active_flow(true)
    }

    fun test_remove_delegation_post_active_flow(should_distribute_rewards: bool) {
        let scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;
        set_up_sui_system_state(scenario);

        governance_test_utils::delegate_to(DELEGATOR_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);

        governance_test_utils::advance_epoch(scenario);

        governance_test_utils::assert_validator_total_stake_amounts(
            vector[VALIDATOR_ADDR_1, VALIDATOR_ADDR_2],
            vector[200, 100],
            scenario
        );

        if (should_distribute_rewards) {
            // advance the epoch and set rewards at 10 SUI for each 100 SUI staked.
            governance_test_utils::advance_epoch_with_reward_amounts(0, 40, scenario);
        } else {
            governance_test_utils::advance_epoch(scenario);
        };

        governance_test_utils::remove_validator(VALIDATOR_ADDR_1, scenario);

        governance_test_utils::advance_epoch(scenario);

        // 110 = stake + rewards for that stake
        // 5 = validator rewards
        let reward_amt = if (should_distribute_rewards) 10 else 0;
        let validator_reward_amt = if (should_distribute_rewards) 5 else 0;

        // Make sure delegation withdrawal happens
        test_scenario::next_tx(scenario, DELEGATOR_ADDR_1);
        {
            let system_state = test_scenario::take_shared<SuiSystemState>(scenario);
            let system_state_mut_ref = &mut system_state;

            assert!(!validator_set::is_active_validator_by_sui_address(
                        sui_system::validators(system_state_mut_ref),
                        VALIDATOR_ADDR_1
                    ), 0);

            let staked_sui = test_scenario::take_from_sender<StakedSui>(scenario);
            assert_eq(staking_pool::staked_sui_amount(&staked_sui), 100);

            // Undelegate from VALIDATOR_ADDR_1
            assert_eq(total_sui_balance(DELEGATOR_ADDR_1, scenario), 0);
            let ctx = test_scenario::ctx(scenario);
            sui_system::request_withdraw_delegation(system_state_mut_ref, staked_sui, ctx);

            // Make sure they have all of their stake.
            assert_eq(total_sui_balance(DELEGATOR_ADDR_1, scenario), 100 + reward_amt);

            test_scenario::return_shared(system_state);
        };

        // Validator undelegates now.
        assert_eq(total_sui_balance(VALIDATOR_ADDR_1, scenario), 0);
        undelegate(VALIDATOR_ADDR_1, 0, scenario);
        if (should_distribute_rewards) undelegate(VALIDATOR_ADDR_1, 0, scenario);

        // Make sure have all of their stake. NB there is no epoch change. This is immediate.
        assert_eq(total_sui_balance(VALIDATOR_ADDR_1, scenario), 100 + reward_amt + validator_reward_amt);

        test_scenario::end(scenario_val);
    }

    #[test]
    fun test_earns_rewards_at_last_epoch() {
        let scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;
        set_up_sui_system_state(scenario);

        delegate_to(DELEGATOR_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);

        advance_epoch(scenario);

        remove_validator(VALIDATOR_ADDR_1, scenario);

        // Add some rewards after the validator requests to leave. Since the validator is still active
        // this epoch, they should get the rewards from this epoch.
        advance_epoch_with_reward_amounts(0, 40, scenario);

        // Each 100 MIST of stake gets 10 MIST and validators shares the 10 MIST from the storage fund
        // so validator gets another 5 MIST.
        let reward_amt = 10;
        let validator_reward_amt = 5;

        // Make sure delegation withdrawal happens
        test_scenario::next_tx(scenario, DELEGATOR_ADDR_1);
        {
            let system_state = test_scenario::take_shared<SuiSystemState>(scenario);
            let system_state_mut_ref = &mut system_state;

            let staked_sui = test_scenario::take_from_sender<StakedSui>(scenario);
            assert_eq(staking_pool::staked_sui_amount(&staked_sui), 100);

            // Undelegate from VALIDATOR_ADDR_1
            assert_eq(total_sui_balance(DELEGATOR_ADDR_1, scenario), 0);
            let ctx = test_scenario::ctx(scenario);
            sui_system::request_withdraw_delegation(system_state_mut_ref, staked_sui, ctx);

            // Make sure they have all of their stake.
            assert_eq(total_sui_balance(DELEGATOR_ADDR_1, scenario), 100 + reward_amt);

            test_scenario::return_shared(system_state);
        };

        // Validator undelegates now.
        assert_eq(total_sui_balance(VALIDATOR_ADDR_1, scenario), 0);
        undelegate(VALIDATOR_ADDR_1, 0, scenario);
        undelegate(VALIDATOR_ADDR_1, 0, scenario);

        // Make sure have all of their stake. NB there is no epoch change. This is immediate.
        assert_eq(total_sui_balance(VALIDATOR_ADDR_1, scenario), 100 + reward_amt + validator_reward_amt);

        test_scenario::end(scenario_val);
    }

    #[test]
    #[expected_failure(abort_code = validator_set::ENotAValidator)]
    fun test_add_delegation_post_active_flow() {
        let scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;
        set_up_sui_system_state(scenario);

        governance_test_utils::delegate_to(DELEGATOR_ADDR_1, VALIDATOR_ADDR_1, 100, scenario);

        governance_test_utils::advance_epoch(scenario);

        governance_test_utils::remove_validator(VALIDATOR_ADDR_1, scenario);

        governance_test_utils::advance_epoch(scenario);

        // Make sure the validator is no longer active.
        test_scenario::next_tx(scenario, DELEGATOR_ADDR_1);
        {
            let system_state = test_scenario::take_shared<SuiSystemState>(scenario);
            let system_state_mut_ref = &mut system_state;

            assert!(!validator_set::is_active_validator_by_sui_address(
                        sui_system::validators(system_state_mut_ref),
                        VALIDATOR_ADDR_1
                    ), 0);

            test_scenario::return_shared(system_state);
        };

        // Now try and delegate to the old validator/staking pool. This should fail!
        governance_test_utils::delegate_to(DELEGATOR_ADDR_1, VALIDATOR_ADDR_1, 60, scenario);

        test_scenario::end(scenario_val);
    }

    #[test]
    fun test_add_preactive_remove_preactive() {
        let scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;
        set_up_sui_system_state(scenario);

        governance_test_utils::add_validator_candidate(NEW_VALIDATOR_ADDR, NEW_VALIDATOR_PUBKEY, NEW_VALIDATOR_POP, scenario);

        // Delegate 100 MIST to the preactive validator
        governance_test_utils::delegate_to(DELEGATOR_ADDR_1, NEW_VALIDATOR_ADDR, 100, scenario);

        // Advance epoch twice with some rewards
        advance_epoch_with_reward_amounts(0, 400, scenario);
        advance_epoch_with_reward_amounts(0, 900, scenario);

        // Undelegate from the preactive validator. There should be no rewards earned.
        governance_test_utils::undelegate(DELEGATOR_ADDR_1, 0, scenario);
        assert_eq(total_sui_balance(DELEGATOR_ADDR_1, scenario), 100);

        test_scenario::end(scenario_val);
    }

    #[test]
    #[expected_failure(abort_code = validator_set::ENotAValidator)]
    fun test_add_preactive_remove_pending_failure() {
        let scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;
        set_up_sui_system_state(scenario);

        governance_test_utils::add_validator_candidate(NEW_VALIDATOR_ADDR, NEW_VALIDATOR_PUBKEY, NEW_VALIDATOR_POP, scenario);

        governance_test_utils::add_validator(NEW_VALIDATOR_ADDR, scenario);

        // Delegate 100 MIST to the pending validator. This should fail because pending active validators don't accept
        // new delegations or withdraws.
        governance_test_utils::delegate_to(DELEGATOR_ADDR_1, NEW_VALIDATOR_ADDR, 100, scenario);

        test_scenario::end(scenario_val);
    }

    #[test]
    fun test_add_preactive_remove_active() {
        let scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;
        set_up_sui_system_state(scenario);

        add_validator_candidate(NEW_VALIDATOR_ADDR, NEW_VALIDATOR_PUBKEY, NEW_VALIDATOR_POP, scenario);

        // Delegate 100 MIST to the preactive validator
        delegate_to(DELEGATOR_ADDR_1, NEW_VALIDATOR_ADDR, 100, scenario);
        advance_epoch_with_reward_amounts(0, 70, scenario);
        delegate_to(DELEGATOR_ADDR_2, NEW_VALIDATOR_ADDR, 300, scenario);
        delegate_to(DELEGATOR_ADDR_3, NEW_VALIDATOR_ADDR, 100, scenario);

        // Now the preactive becomes active
        add_validator(NEW_VALIDATOR_ADDR, scenario);
        advance_epoch(scenario);

        advance_epoch_with_reward_amounts(0, 80, scenario);

        // delegator 1 and 3 undelegate from the validator and earns 9 MIST.
        // Although they delegate in different epochs, they earn the same rewards as long as they undelegate
        // in the same epoch because the validator was preactive when they delegated.
        undelegate(DELEGATOR_ADDR_1, 0, scenario);
        assert_eq(total_sui_balance(DELEGATOR_ADDR_1, scenario), 109);
        undelegate(DELEGATOR_ADDR_3, 0, scenario);
        assert_eq(total_sui_balance(DELEGATOR_ADDR_3, scenario), 109);

        advance_epoch_with_reward_amounts(0, 100, scenario);
        undelegate(DELEGATOR_ADDR_2, 0, scenario);
        // delegator 2 earns about 27 MIST from the previous epoch and 5/8 of the 100 MIST.
        assert_eq(total_sui_balance(DELEGATOR_ADDR_2, scenario), 300 + 27 + 59);

        test_scenario::end(scenario_val);
    }

    #[test]
    fun test_add_preactive_remove_post_active() {
        let scenario_val = test_scenario::begin(VALIDATOR_ADDR_1);
        let scenario = &mut scenario_val;
        set_up_sui_system_state(scenario);

        add_validator_candidate(NEW_VALIDATOR_ADDR, NEW_VALIDATOR_PUBKEY, NEW_VALIDATOR_POP, scenario);

        // Delegate 100 MIST to the preactive validator
        delegate_to(DELEGATOR_ADDR_1, NEW_VALIDATOR_ADDR, 100, scenario);

        // Now the preactive becomes active
        add_validator(NEW_VALIDATOR_ADDR, scenario);
        advance_epoch(scenario);

        advance_epoch_with_reward_amounts(0, 80, scenario); // delegator 1 earns 20 MIST here.

        // And now the validator leaves the validator set.
        remove_validator(NEW_VALIDATOR_ADDR, scenario);

        advance_epoch(scenario);

        undelegate(DELEGATOR_ADDR_1, 0, scenario);
        assert_eq(total_sui_balance(DELEGATOR_ADDR_1, scenario), 100 + 20);

        test_scenario::end(scenario_val);
    }

    fun set_up_sui_system_state(scenario: &mut Scenario) {
        let ctx = test_scenario::ctx(scenario);

        let validators = vector[
            create_validator_for_testing(VALIDATOR_ADDR_1, 100, ctx),
            create_validator_for_testing(VALIDATOR_ADDR_2, 100, ctx)
        ];
        create_sui_system_state_for_testing(validators, 300, 100, ctx);
    }
}
