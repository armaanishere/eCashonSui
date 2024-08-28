// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[defines_primitive(u256)]
module std::u256 {
    use std::string::String;

    /// Return the larger of `x` and `y`
    public fun max(x: u256, y: u256): u256 {
        std::macros::num_max!(x, y)
    }

    /// Return the smaller of `x` and `y`
    public fun min(x: u256, y: u256): u256 {
        std::macros::num_min!(x, y)
    }

    /// Return the absolute value of x - y
    public fun diff(x: u256, y: u256): u256 {
        std::macros::num_diff!(x, y)
    }

    /// Calculate x / y, but round up the result.
    public fun divide_and_round_up(x: u256, y: u256): u256 {
        std::macros::num_divide_and_round_up!(x, y)
    }

    /// Return the value of a base raised to a power
    public fun pow(base: u256, exponent: u8): u256 {
        std::macros::num_pow!(base, exponent)
    }

    public fun to_string(mut value: u256): String {
        if (value == 0) {
            return b"0".to_string()
        };
        let mut buffer = vector[];
        while (value != 0) {
            buffer.push_back(((48 + value % 10) as u8));
            value = value / 10;
        };
        buffer.reverse();
        buffer.to_string()
    }

    /// Loops applying `$f` to each number from `$start` to `$stop` (exclusive)
    public macro fun range_do($start: u256, $stop: u256, $f: |u256|) {
        std::macros::range_do!($start, $stop, $f)
    }

    /// Loops applying `$f` to each number from `$start` to `$stop` (inclusive)
    public macro fun range_do_eq($start: u256, $stop: u256, $f: |u256|) {
        std::macros::range_do_eq!($start, $stop, $f)
    }

    /// Loops applying `$f` to each number from `0` to `$stop` (exclusive)
    public macro fun do($stop: u256, $f: |u256|) {
        std::macros::do!($stop, $f)
    }

    /// Loops applying `$f` to each number from `0` to `$stop` (inclusive)
    public macro fun do_eq($stop: u256, $f: |u256|) {
        std::macros::do_eq!($stop, $f)
    }
}
