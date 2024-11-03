// options:
// printWidth: 50

module prettier::use_declaration {
    use sui::coin::Coin;
    use sui::coin::Coin as C;
    use sui::coin::{Self as c, Coin as C};
    use sui::coin::very_long_function_name_very_long_function_name;
    use beep::staked_sui::StakedSui;

    use sui::transfer_policy::{Self as policy, TransferPolicy, TransferPolicyCap, TransferRequest};
    use sui::transfer_policy::TransferPolicyCap as cap;
    use sui::{
        transfer_policy::{TransferPolicy, TransferPolicyCap, TransferRequest},
        transfer_policy::TransferPolicyCap as cap,
    };

    public use fun my_custom_function_with_a_long_name as TransferPolicyCap.very_long_function_name;

    friend been::here;
}
