// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// tests the invalid creation and deletion of a parent object

//# init --addresses test=0x0 --accounts A B

//# publish

module test::m {
    use sui::tx_context::TxContext;

    struct S has key, store {
        info: sui::object::Info,
    }

    public entry fun t(ctx: &mut TxContext) {
        let parent = sui::object::new(ctx);
        let child = S { info: sui::object::new(ctx) };
        sui::transfer::transfer_to_object_id(child, &mut parent);
        sui::object::delete(parent);
    }
}

//# run test::m::t --sender A
