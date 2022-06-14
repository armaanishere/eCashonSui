// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

module Tutorial::TrustedSwap {
    use sui::balance::{Self, Balance};
    use sui::Coin::{Self, Coin};
    use sui::id::{Self, VersionedID};
    use sui::SUI::SUI;
    use sui::transfer;
    use sui::tx_context::{Self, TxContext};

    const MIN_FEE: u64 = 1000;

    struct Object has key, store {
        id: VersionedID,
        scarcity: u8,
        style: u8,
    }

    struct ObjectWrapper has key {
        id: VersionedID,
        original_owner: address,
        to_swap: Object,
        fee: Balance<SUI>,
    }

    public entry fun create_object(scarcity: u8, style: u8, ctx: &mut TxContext) {
        let object = Object {
            id: tx_context::new_id(ctx),
            scarcity,
            style,
        };
        transfer::transfer(object, tx_context::sender(ctx))
    }

    public entry fun transfer_object(object: Object, ctx: &mut TxContext) {
        transfer::transfer(object, tx_context::sender(ctx))
    }

    /// Anyone owns an `Object` can request swapping their object. This object
    /// will be wrapped into `ObjectWrapper` and sent to `service_address`.
    public entry fun request_swap(object: Object, fee: Coin<SUI>, service_address: address, ctx: &mut TxContext) {
        assert!(Coin::value(&fee) >= MIN_FEE, 0);
        let wrapper = ObjectWrapper {
            id: tx_context::new_id(ctx),
            original_owner: tx_context::sender(ctx),
            to_swap: object,
            fee: Coin::into_balance(fee),
        };
        transfer::transfer(wrapper, service_address);
    }

    /// When the admin has two swap requests with objects that are trade-able,
    /// the admin can execute the swap and send them back to the opposite owner.
    public entry fun execute_swap(wrapper1: ObjectWrapper, wrapper2: ObjectWrapper, ctx: &mut TxContext) {
        // Only swap if their scarcity is the same and style is different.
        assert!(wrapper1.to_swap.scarcity == wrapper2.to_swap.scarcity, 0);
        assert!(wrapper1.to_swap.style != wrapper2.to_swap.style, 0);

        // Unpack both wrappers, cross send them to the other owner.
        let ObjectWrapper {
            id: id1,
            original_owner: original_owner1,
            to_swap: object1,
            fee: fee1,
        } = wrapper1;

        let ObjectWrapper {
            id: id2,
            original_owner: original_owner2,
            to_swap: object2,
            fee: fee2,
        } = wrapper2;

        // Perform the swap.
        transfer::transfer(object1, original_owner2);
        transfer::transfer(object2, original_owner1);

        // Service provider takes the fee.
        let service_address = tx_context::sender(ctx);
        balance::join(&mut fee1, fee2);
        transfer::transfer(Coin::from_balance(fee1, ctx), service_address);

        // Effectively delete the wrapper objects.
        id::delete(id1);
        id::delete(id2);
    }
}
