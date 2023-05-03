// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

/// 在共享对象示例的基础上加入事件（Event）
/// Extended example of a shared object. Now with addition of events!
module examples::donuts_with_events {
    use sui::transfer;
    use sui::sui::SUI;
    use sui::coin::{Self, Coin};
    use sui::object::{Self, ID, UID};
    use sui::balance::{Self, Balance};
    use sui::tx_context::{Self, TxContext};

    // 使用事件是只需要添加`sui::event`依赖
    // This is the only dependency you need for events.
    use sui::event;

    /// 错误码 0：对应所付费用低于甜甜圈价格   
    /// For when Coin balance is too low.
    const ENotEnough: u64 = 0;

    /// 商店所有者权限凭证：获取利润
    /// Capability that grants an owner the right to collect profits.
    struct ShopOwnerCap has key { id: UID }

    /// 一个可被购买的甜甜圈对象
    /// A purchasable Donut. For simplicity's sake we ignore implementation.
    struct Donut has key { id: UID }

    struct DonutShop has key {
        id: UID,
        price: u64,
        balance: Balance<SUI>
    }

    // ====== Events ======

    /// 当用户购买甜甜圈时触发本事件
    /// For when someone has purchased a donut.
    struct DonutBought has copy, drop {
        id: ID
    }

    /// 当店主提取利润时触发本事件
    /// For when DonutShop owner has collected profits.
    struct ProfitsCollected has copy, drop {
        amount: u64
    }

    // ====== Functions ======

    fun init(ctx: &mut TxContext) {
        transfer::transfer(ShopOwnerCap {
            id: object::new(ctx)
        }, tx_context::sender(ctx));

        transfer::share_object(DonutShop {
            id: object::new(ctx),
            price: 1000,
            balance: balance::zero()
        })
    }

    /// 购买一个甜甜圈
    /// Buy a donut.
    public entry fun buy_donut(
        shop: &mut DonutShop, payment: &mut Coin<SUI>, ctx: &mut TxContext
    ) {
        assert!(coin::value(payment) >= shop.price, ENotEnough);

        let coin_balance = coin::balance_mut(payment);
        let paid = balance::split(coin_balance, shop.price);
        let id = object::new(ctx);

        balance::join(&mut shop.balance, paid);

        // 生成包含新甜甜圈ID的`DonutBought`事件
        // Emit the event using future object's ID.
        event::emit(DonutBought { id: object::uid_to_inner(&id) });
        transfer::transfer(Donut { id }, tx_context::sender(ctx))
    }

    /// 吃掉甜甜圈 ：）
    /// Consume donut and get nothing...
    public entry fun eat_donut(d: Donut) {
        let Donut { id } = d;
        object::delete(id);
    }

    /// 收集利润，需要`ShopOwnerCap`凭证
    /// Take coin from `DonutShop` and transfer it to tx sender.
    /// Requires authorization with `ShopOwnerCap`.
    public entry fun collect_profits(
        _: &ShopOwnerCap, shop: &mut DonutShop, ctx: &mut TxContext
    ) {
        let amount = balance::value(&shop.balance);
        let profits = coin::take(&mut shop.balance, amount, ctx);

        // 生成包含转账金额的`ProfitsCollected`事件
        // simply create new type instance and emit it
        event::emit(ProfitsCollected { amount });

        transfer::public_transfer(profits, tx_context::sender(ctx))
    }
}
