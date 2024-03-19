// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module sui::clock_tests {
    use sui::clock;
    use sui::tx_context;

    #[test]
    fun creating_a_clock_and_incrementing_it() {
        let mut ctx = tx_context::dummy();
        let mut clock = clock::create_for_testing(&mut ctx);

        clock.increment_for_testing(42);
        assert!(clock.timestamp_ms() == 42, 1);

        clock.set_for_testing(50);
        assert!(clock.timestamp_ms() == 50, 1);

        clock.destroy_for_testing();
    }
}
