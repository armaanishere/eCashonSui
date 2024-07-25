// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

#[allow(dead_code)]
mod rosetta_client;
#[path = "custom_coins/test_coin_utils.rs"]
mod test_coin_utils;

use sui_rosetta::types::{
    AccountBalanceRequest, AccountBalanceResponse, AccountIdentifier, Currency, CurrencyMetadata,
    NetworkIdentifier, SuiEnv,
};
use sui_rosetta::SUI;
use test_cluster::TestClusterBuilder;
use test_coin_utils::{init_package, mint};

use crate::rosetta_client::{start_rosetta_test_server, RosettaEndpoint};

#[tokio::test]
async fn test_custom_coin_balance() {
    // mint coins to `test_culset.get_address_1()` and `test_culset.get_address_2()`
    const SUI_BALANCE: u64 = 150_000_000_000_000_000;
    const COIN1_BALANCE: u64 = 100_000_000;
    const COIN2_BALANCE: u64 = 200_000_000;
    let test_cluster = TestClusterBuilder::new().build().await;
    let client = test_cluster.wallet.get_client().await.unwrap();
    let keystore = &test_cluster.wallet.config.keystore;

    let (rosetta_client, _handle) = start_rosetta_test_server(client.clone()).await;

    let sender = test_cluster.get_address_0();
    let init_ret = init_package(&client, keystore, sender).await.unwrap();

    let address1 = test_cluster.get_address_1();
    let address2 = test_cluster.get_address_2();
    let balances_to = vec![(COIN1_BALANCE, address1), (COIN2_BALANCE, address2)];
    let coin_type = init_ret.coin_tag.to_canonical_string(true);

    let _mint_res = mint(&client, keystore, init_ret, balances_to)
        .await
        .unwrap();

    // setup AccountBalanceRequest
    let network_identifier = NetworkIdentifier {
        blockchain: "sui".to_string(),
        network: SuiEnv::LocalNet,
    };

    let sui_currency = SUI.clone();
    let test_coin_currency = Currency {
        symbol: "TEST_COIN".to_string(),
        decimals: 6,
        metadata: Some(CurrencyMetadata {
            coin_type: coin_type.clone(),
        }),
    };

    // Verify initial balance and stake
    let request = AccountBalanceRequest {
        network_identifier: network_identifier.clone(),
        account_identifier: AccountIdentifier {
            address: address1,
            sub_account: None,
        },
        block_identifier: Default::default(),
        currencies: vec![sui_currency, test_coin_currency],
    };

    println!(
        "request: {}",
        serde_json::to_string_pretty(&request).unwrap()
    );
    let response: AccountBalanceResponse = rosetta_client
        .call(RosettaEndpoint::Balance, &request)
        .await;
    println!(
        "response: {}",
        serde_json::to_string_pretty(&response).unwrap()
    );
    assert_eq!(response.balances.len(), 2);
    assert_eq!(response.balances[0].value, SUI_BALANCE as i128);
    assert_eq!(
        response.balances[0]
            .currency
            .clone()
            .metadata
            .unwrap()
            .coin_type,
        "0x2::sui::SUI"
    );
    assert_eq!(response.balances[1].value, COIN1_BALANCE as i128);
    assert_eq!(
        response.balances[1]
            .currency
            .clone()
            .metadata
            .unwrap()
            .coin_type,
        coin_type
    );
}
