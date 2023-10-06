// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

use super::big_int::BigInt;
use super::move_object::MoveObject;
use async_graphql::*;

use sui_types::coin::Coin as NativeSuiCoin;

pub(crate) struct Coin {
    pub move_obj: MoveObject,
}

#[Object]
impl Coin {
    async fn balance(&self) -> Option<BigInt> {
        self.move_obj
            .native_object
            .data
            .try_as_move()
            .map(|x| {
                if x.is_coin() {
                    Some(NativeSuiCoin::extract_balance_if_coin(
                        &self.move_obj.native_object,
                    ))
                } else {
                    None
                }
            })
            .flatten()
            .map(|x| x.expect("Coin should have balance."))
            .flatten()
            .map(BigInt::from)
    }

    async fn as_move_object(&self) -> Option<MoveObject> {
        Some(self.move_obj.clone())
    }
}
