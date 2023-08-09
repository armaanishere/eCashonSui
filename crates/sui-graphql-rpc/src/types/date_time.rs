// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use async_graphql::*;
use move_core_types::u256::U256;

// ISO-8601 Date and Time
// Encoded as a 64-bit unix timestamp
struct DateTime(u64);

/// TODO: implement DateTime scalar type
#[Scalar]
impl ScalarType for DateTime {
    fn parse(_value: Value) -> InputValueResult<Self> {
        unimplemented!()
    }

    fn to_value(&self) -> Value {
        unimplemented!()
    }
}
