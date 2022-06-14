// Copyright (c) 2022, Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[test_only]
module NFTs::AuctionTests {
    use std::vector;

    use sui::coin::{Self, Coin};
    use sui::SUI::SUI;
    use sui::id::{Self, VersionedID};
    use sui::test_scenario::Self;
    use sui::tx_context::{Self, TxContext};

    use NFTs::Auction::{Self, Bid};
    use NFTs::AuctionLib::Auction;

    // Error codes.
    const EWRONG_ITEM_VALUE: u64 = 1;

    // Example of an object type that could be sold at an auction.
    struct SomeItemToSell has key, store {
        id: VersionedID,
        value: u64,
    }

    // Initializes the "state of the world" that mimics what should
    // be available in Sui genesis state (e.g., mints and distributes
    // coins to users).
    fun init(ctx: &mut TxContext, bidders: vector<address>) {
        while (!vector::is_empty(&bidders)) {
            let bidder = vector::pop_back(&mut bidders);
            let coin = coin::mint_for_testing(100, ctx);
            coin::transfer<SUI>(coin, bidder);
        };
    }

    #[test]
    fun simple_auction_test() {
        let auctioneer = @0xABBA;
        let owner = @0xACE;
        let bidder1 = @0xFACE;
        let bidder2 = @0xCAFE;


        let scenario = &mut test_scenario::begin(&auctioneer);
        {
            let bidders = vector::empty();
            vector::push_back(&mut bidders, bidder1);
            vector::push_back(&mut bidders, bidder2);
            init(test_scenario::ctx(scenario), bidders);
        };

        // a transaction by the item owner to put it for auction
        test_scenario::next_tx(scenario, &owner);
        let ctx = test_scenario::ctx(scenario);
        let to_sell = SomeItemToSell {
            id: tx_context::new_id(ctx),
            value: 42,
        };
        // generate unique auction ID (it would be more natural to
        // generate one in crate_auction and return it, but we cannot
        // do this at the moment)
        let id = tx_context::new_id(ctx);
        // we need to dereference (copy) right here rather wherever
        // auction_id is used - otherwise id would still be considered
        // borrowed and could not be passed argument to a function
        // consuming it
        let auction_id = *id::inner(&id);
        Auction::create_auction(to_sell, id, auctioneer, ctx);

        // a transaction by the first bidder to create and put a bid
        test_scenario::next_tx(scenario, &bidder1);
        {
            let coin = test_scenario::take_owned<Coin<SUI>>(scenario);

            Auction::bid(coin, auction_id, auctioneer, test_scenario::ctx(scenario));
        };

        // a transaction by the auctioneer to update state of the auction
        test_scenario::next_tx(scenario, &auctioneer);
        {
            let auction = test_scenario::take_owned<Auction<SomeItemToSell>>(scenario);

            let bid = test_scenario::take_owned<Bid>(scenario);
            Auction::update_auction(&mut auction, bid, test_scenario::ctx(scenario));

            test_scenario::return_owned(scenario, auction);
        };
        // a transaction by the second bidder to create and put a bid (a
        // bid will fail as it has the same value as that of the first
        // bidder's)
        test_scenario::next_tx(scenario, &bidder2);
        {
            let coin = test_scenario::take_owned<Coin<SUI>>(scenario);

            Auction::bid(coin, auction_id, auctioneer, test_scenario::ctx(scenario));
        };

        // a transaction by the auctioneer to update state of the auction
        test_scenario::next_tx(scenario, &auctioneer);
        {
            let auction = test_scenario::take_owned<Auction<SomeItemToSell>>(scenario);

            let bid = test_scenario::take_owned<Bid>(scenario);
            Auction::update_auction(&mut auction, bid, test_scenario::ctx(scenario));

            test_scenario::return_owned(scenario, auction);
        };

        // a transaction by the auctioneer to end auction
        test_scenario::next_tx(scenario, &auctioneer);
        {
            let auction = test_scenario::take_owned<Auction<SomeItemToSell>>(scenario);

            Auction::end_auction(auction, test_scenario::ctx(scenario));
        };

        // a transaction to check if the first bidder won (as the
        // second bidder's bid was the same as that of the first one)
        test_scenario::next_tx(scenario, &bidder1);
        {
            let acquired_item = test_scenario::take_owned<SomeItemToSell>(scenario);

            assert!(acquired_item.value == 42, EWRONG_ITEM_VALUE);

            test_scenario::return_owned(scenario, acquired_item);
        };
    }
}
