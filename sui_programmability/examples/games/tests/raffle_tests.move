// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module games::raffle_tests {

    use std::option;
    use sui::clock;
    use sui::coin::{Self, Coin};
    use sui::random::{Self, update_randomness_state_for_testing, Random};
    use sui::sui::SUI;
    use sui::test_scenario::{Self, Scenario};
    use sui::transfer;

    use games::raffle;

    fun mint(addr: address, amount: u64, scenario: &mut Scenario) {
        transfer::public_transfer(coin::mint_for_testing<SUI>(amount, test_scenario::ctx(scenario)), addr);
        test_scenario::next_tx(scenario, addr);
    }

    #[test]
    fun test_game() {
        let user1 = @0x0;
        let user2 = @0x1;
        let user3 = @0x2;
        let user4 = @0x3;

        let scenario_val = test_scenario::begin(user1);
        let scenario = &mut scenario_val;

        // Setup randomness
        random::create_for_testing(test_scenario::ctx(scenario));
        test_scenario::next_tx(scenario, user1);
        let random_state = test_scenario::take_shared<Random>(scenario);
        update_randomness_state_for_testing(
            &mut random_state,
            0,
            x"1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F1F",
            test_scenario::ctx(scenario)
        );

        // Create the game and get back the output objects.
        mint(user1, 1000, scenario);
        let end_time = 100;
        raffle::create(end_time, 10, test_scenario::ctx(scenario));
        test_scenario::next_tx(scenario, user1);
        let game = test_scenario::take_shared<raffle::Game>(scenario);
        assert!(raffle::get_cost_in_sui(&game) == 10, 1);
        assert!(raffle::get_participants(&game) == 0, 1);
        assert!(raffle::get_end_time(&game) == end_time, 1);
        assert!(raffle::get_winner(&game) == option::none(), 1);
        assert!(raffle::get_balance(&game) == 0, 1);

        let clock = clock::create_for_testing(test_scenario::ctx(scenario));
        clock::set_for_testing(&mut clock, 10);

        // Play with 4 users (everything here is deterministic)
        test_scenario::next_tx(scenario, user1);
        mint(user1, 10, scenario);
        let coin = test_scenario::take_from_sender<Coin<SUI>>(scenario);
        let t1 = raffle::play(&mut game, coin, &clock, test_scenario::ctx(scenario));
        assert!(raffle::get_participants(&game) == 1, 1);
        raffle::destroy_ticket(t1); // loser

        test_scenario::next_tx(scenario, user2);
        mint(user2, 10, scenario);
        let coin = test_scenario::take_from_sender<Coin<SUI>>(scenario);
        let t2 = raffle::play(&mut game, coin, &clock, test_scenario::ctx(scenario));
        assert!(raffle::get_participants(&game) == 2, 1);
        raffle::destroy_ticket(t2); // loser

        test_scenario::next_tx(scenario, user3);
        mint(user3, 10, scenario);
        let coin = test_scenario::take_from_sender<Coin<SUI>>(scenario);
        let t3 = raffle::play(&mut game, coin, &clock, test_scenario::ctx(scenario));
        assert!(raffle::get_participants(&game) == 3, 1);
        raffle::destroy_ticket(t3); // loser

        test_scenario::next_tx(scenario, user4);
        mint(user4, 10, scenario);
        let coin = test_scenario::take_from_sender<Coin<SUI>>(scenario);
        let t4 = raffle::play( &mut game, coin, &clock, test_scenario::ctx(scenario));
        assert!(raffle::get_participants(&game) == 4, 1);
        // this is the winner

        // Determine the winner (-> user3)
        clock::set_for_testing(&mut clock, 101);
        raffle::determine_winner(&mut game, &random_state, &clock, test_scenario::ctx(scenario));
        assert!(raffle::get_winner(&game) == option::some(4), 1);
        assert!(raffle::get_balance(&game) == 40, 1);

        // Take the reward
        let coin = raffle::redeem(t4, &mut game, test_scenario::ctx(scenario));
        assert!(coin::value(&coin) == 40, 1);
        coin::burn_for_testing(coin);

        clock::destroy_for_testing(clock);
        test_scenario::return_shared(game);
        test_scenario::return_shared(random_state);
        test_scenario::end(scenario_val);
    }
}
