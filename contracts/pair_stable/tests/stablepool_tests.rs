#![cfg(not(tarpaulin_include))]

use cosmwasm_std::{Addr, Decimal, StdError};
use itertools::Itertools;
use std::str::FromStr;

use helper::AppExtension;
use palomadex::asset::AssetInfoExt;
use palomadex::cosmwasm_ext::AbsDiff;
use palomadex::observation::OracleObservation;
use palomadex_pair_stable::error::ContractError;

use crate::helper::{f64_to_dec, Helper, TestCoin};

mod helper;

#[ignore]
#[test]
fn provide_and_withdraw_no_fee() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::native("uluna"),
        TestCoin::cw20("USDC"),
        TestCoin::cw20("USDD"),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, Some(0u16)).unwrap();

    let user1 = Addr::unchecked("user1");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
        helper.assets[&test_coins[2]].with_balance(100_000000u128),
    ];
    helper.give_me_money(&assets, &user1);

    helper.provide_liquidity(&user1, &assets).unwrap();

    assert_eq!(299999000, helper.token_balance(&helper.lp_token, &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[2], &user1));

    // The user2 with the same assets should receive the same share
    let user2 = Addr::unchecked("user2");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
        helper.assets[&test_coins[2]].with_balance(100_000000u128),
    ];
    helper.give_me_money(&assets, &user2);
    helper.provide_liquidity(&user2, &assets).unwrap();
    assert_eq!(300_000000, helper.token_balance(&helper.lp_token, &user2));

    // The user3 makes imbalanced provide thus he is charged with fees
    let user3 = Addr::unchecked("user3");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(200_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];
    helper.give_me_money(&assets, &user3);
    helper.provide_liquidity(&user3, &assets).unwrap();
    assert_eq!(299_629321, helper.token_balance(&helper.lp_token, &user3));

    // Providing last asset with explicit zero amount should give nearly the same result
    let user4 = Addr::unchecked("user4");
    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(200_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
        helper.assets[&test_coins[2]].with_balance(0u128),
    ];
    helper.give_me_money(&assets, &user4);
    helper.provide_liquidity(&user4, &assets).unwrap();
    assert_eq!(299_056292, helper.token_balance(&helper.lp_token, &user4));

    helper
        .withdraw_liquidity(&user1, 299999000, vec![])
        .unwrap();

    assert_eq!(0, helper.token_balance(&helper.lp_token, &user1));
    // Previous imbalanced provides resulted in different share in assets
    assert_eq!(150163977, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(100109318, helper.coin_balance(&test_coins[1], &user1));
    assert_eq!(50054659, helper.coin_balance(&test_coins[2], &user1));

    // Checking imbalanced withdraw. Withdrawing only the first asset x 300 with the whole amount of LP tokens
    helper
        .withdraw_liquidity(
            &user2,
            300_000000,
            vec![helper.assets[&test_coins[0]].with_balance(300_000000u128)],
        )
        .unwrap();

    // Previous imbalanced provides resulted in small LP balance residual
    assert_eq!(619390, helper.token_balance(&helper.lp_token, &user2));
    assert_eq!(300_000000, helper.coin_balance(&test_coins[0], &user2));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user2));
    assert_eq!(0, helper.coin_balance(&test_coins[2], &user2));

    // Trying to receive more than possible
    let err = helper
        .withdraw_liquidity(
            &user3,
            100_000000,
            vec![helper.assets[&test_coins[1]].with_balance(101_000000u128)],
        )
        .unwrap_err();
    assert_eq!(
        "Generic error: Not enough LP tokens. You need 100679731 LP tokens.",
        err.root_cause().to_string()
    );

    // Providing more LP tokens than needed. The rest will be kept on the user's balance
    helper
        .withdraw_liquidity(
            &user3,
            200_892384,
            vec![helper.assets[&test_coins[1]].with_balance(101_000000u128)],
        )
        .unwrap();

    // initial balance - spent amount; 100 goes back to the user3
    assert_eq!(
        299_629321 - 100679731,
        helper.token_balance(&helper.lp_token, &user3)
    );
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user3));
    assert_eq!(101_000000, helper.coin_balance(&test_coins[1], &user3));
    assert_eq!(0, helper.coin_balance(&test_coins[2], &user3));
}

#[test]
fn provide_with_different_precision() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::cw20precise("FOO", 4),
        TestCoin::cw20precise("BAR", 5),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, None).unwrap();

    for user_name in ["user1", "user2"] {
        let user = Addr::unchecked(user_name);

        let assets = vec![
            helper.assets[&test_coins[0]].with_balance(100_0000u128),
            helper.assets[&test_coins[1]].with_balance(100_00000u128),
        ];
        helper.give_me_money(&assets, &user);

        helper.provide_liquidity(&user, &assets).unwrap();
    }

    let user1 = Addr::unchecked("user1");

    assert_eq!(19999000, helper.token_balance(&helper.lp_token, &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user1));

    helper.withdraw_liquidity(&user1, 19999000, vec![]).unwrap();

    assert_eq!(0, helper.token_balance(&helper.lp_token, &user1));
    assert_eq!(999950, helper.coin_balance(&test_coins[0], &user1));
    assert_eq!(9999500, helper.coin_balance(&test_coins[1], &user1));

    let user2 = Addr::unchecked("user2");
    assert_eq!(20000000, helper.token_balance(&helper.lp_token, &user2));
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user2));
    assert_eq!(0, helper.coin_balance(&test_coins[1], &user2));

    helper.withdraw_liquidity(&user2, 20000000, vec![]).unwrap();

    assert_eq!(0, helper.token_balance(&helper.lp_token, &user2));
    assert_eq!(999999, helper.coin_balance(&test_coins[0], &user2));
    assert_eq!(9999999, helper.coin_balance(&test_coins[1], &user2));
}

#[test]
fn swap_different_precisions() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::cw20precise("FOO", 4),
        TestCoin::cw20precise("BAR", 5),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, None).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_0000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_00000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    let user = Addr::unchecked("user");
    // 100 x FOO tokens
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_0000u128);
    // Checking direct swap simulation
    let sim_resp = helper
        .simulate_swap(&offer_asset, Some(helper.assets[&test_coins[1]].clone()))
        .unwrap();
    // And reverse swap as well
    let reverse_sim_resp = helper
        .simulate_reverse_swap(
            &helper.assets[&test_coins[1]].with_balance(sim_resp.return_amount.u128()),
            Some(helper.assets[&test_coins[0]].clone()),
        )
        .unwrap();
    assert_eq!(offer_asset.amount, reverse_sim_resp.offer_amount);

    helper.give_me_money(&[offer_asset.clone()], &user);
    helper
        .swap(
            &user,
            &offer_asset,
            Some(helper.assets[&test_coins[1]].clone()),
        )
        .unwrap();
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user));
    // 99.94902 x BAR tokens
    assert_eq!(99_94902, sim_resp.return_amount.u128());
    assert_eq!(99_94902, helper.coin_balance(&test_coins[1], &user));
}

#[ignore]
#[test]
fn check_swaps() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::native("uluna"),
        TestCoin::cw20("USDC"),
        TestCoin::cw20("USDD"),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, None).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000000u128),
        helper.assets[&test_coins[2]].with_balance(100_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    let user = Addr::unchecked("user");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user);

    let err = helper.swap(&user, &offer_asset, None).unwrap_err();
    assert_eq!(
        ContractError::VariableAssetMissed {},
        err.downcast().unwrap()
    );

    let err = helper
        .swap(
            &user,
            &offer_asset,
            Some(helper.assets[&test_coins[0]].clone()),
        )
        .unwrap_err();
    assert_eq!(ContractError::SameAssets {}, err.downcast().unwrap());

    helper
        .swap(
            &user,
            &offer_asset,
            Some(helper.assets[&test_coins[1]].clone()),
        )
        .unwrap();
    assert_eq!(0, helper.coin_balance(&test_coins[0], &user));
    assert_eq!(99_949011, helper.coin_balance(&test_coins[1], &user));
}

#[test]
fn check_wrong_initializations() {
    let owner = Addr::unchecked("owner");

    let err = Helper::new(&owner, vec![TestCoin::native("uluna")], 100u64, None).unwrap_err();

    assert_eq!(
        ContractError::InvalidNumberOfAssets(2),
        err.downcast().unwrap()
    );

    let err = Helper::new(
        &owner,
        vec![
            TestCoin::native("one"),
            TestCoin::cw20("two"),
            TestCoin::native("three"),
            TestCoin::cw20("four"),
            TestCoin::native("five"),
            TestCoin::cw20("six"),
        ],
        100u64,
        None,
    )
    .unwrap_err();

    assert_eq!(
        ContractError::InvalidNumberOfAssets(2),
        err.downcast().unwrap()
    );

    let err = Helper::new(
        &owner,
        vec![TestCoin::native("uluna"), TestCoin::native("uluna")],
        100u64,
        None,
    )
    .unwrap_err();

    assert_eq!(
        err.root_cause().to_string(),
        "Doubling assets in asset infos"
    );

    // 2 assets in the pool is okay
    Helper::new(
        &owner,
        vec![TestCoin::native("one"), TestCoin::cw20("two")],
        100u64,
        None,
    )
    .unwrap();
}

#[ignore]
#[test]
fn check_withdraw_charges_fees() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![
        TestCoin::native("uluna"),
        TestCoin::cw20("USDC"),
        TestCoin::cw20("USDD"),
    ];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, None).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000_000000u128),
        helper.assets[&test_coins[2]].with_balance(100_000_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();

    let user1 = Addr::unchecked("user1");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(100_000000u128);

    // Usual swap for reference
    helper.give_me_money(&[offer_asset.clone()], &user1);
    helper
        .swap(
            &user1,
            &offer_asset,
            Some(helper.assets[&test_coins[1]].clone()),
        )
        .unwrap();
    let usual_swap_amount = helper.coin_balance(&test_coins[1], &user1);
    assert_eq!(99_950000, usual_swap_amount);

    // Trying to swap LUNA -> USDC via provide/withdraw
    let user2 = Addr::unchecked("user2");
    helper.give_me_money(&[offer_asset.clone()], &user2);

    // Provide 100 x LUNA
    helper.provide_liquidity(&user2, &[offer_asset]).unwrap();

    // Withdraw 100 x USDC
    let lp_tokens_amount = helper.token_balance(&helper.lp_token, &user2);
    let err = helper
        .withdraw_liquidity(
            &user2,
            lp_tokens_amount,
            vec![helper.assets[&test_coins[1]].with_balance(100_000000u128)],
        )
        .unwrap_err();
    assert_eq!(
        err.root_cause().to_string(),
        "Generic error: Not enough LP tokens. You need 100025002 LP tokens."
    );

    helper
        .withdraw_liquidity(
            &user2,
            lp_tokens_amount,
            vec![helper.assets[&test_coins[1]].with_balance(usual_swap_amount)],
        )
        .unwrap();

    // A small residual of LP tokens is left
    assert_eq!(8, helper.token_balance(&helper.lp_token, &user2));
    assert_eq!(
        usual_swap_amount,
        helper.coin_balance(&test_coins[1], &user2)
    );
}

#[test]
fn check_twap_based_prices() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::cw20("USDX")];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, None).unwrap();

    let check_prices = |helper: &Helper| {
        let prices = helper.query_prices().unwrap();

        test_coins
            .iter()
            .cartesian_product(test_coins.iter())
            .filter(|(a, b)| a != b)
            .for_each(|(from_coin, to_coin)| {
                let price = prices
                    .cumulative_prices
                    .iter()
                    .filter(|(from, to, _)| {
                        from.eq(&helper.assets[&from_coin]) && to.eq(&helper.assets[&to_coin])
                    })
                    .collect::<Vec<_>>();
                assert_eq!(price.len(), 1);
                assert!(!price[0].2.is_zero());
            });
    };

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();
    helper.app.next_block(1000);
    check_prices(&helper);

    let user1 = Addr::unchecked("user1");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(1000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user1);

    helper
        .swap(
            &user1,
            &offer_asset,
            Some(helper.assets[&test_coins[1]].clone()),
        )
        .unwrap();

    helper.app.next_block(86400);
    check_prices(&helper);

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];
    helper.give_me_money(&assets, &user1);

    // Imbalanced provide
    helper.provide_liquidity(&user1, &assets).unwrap();
    helper.app.next_block(14 * 86400);
    check_prices(&helper);

    let offer_asset = helper.assets[&test_coins[1]].with_balance(10_000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user1);
    helper
        .swap(
            &user1,
            &offer_asset,
            Some(helper.assets[&test_coins[0]].clone()),
        )
        .unwrap();
    helper.app.next_block(86400);
    check_prices(&helper);
}

#[test]
fn check_pool_prices() {
    let owner = Addr::unchecked("owner");

    let test_coins = vec![TestCoin::native("uusd"), TestCoin::cw20("USDX")];

    let mut helper = Helper::new(&owner, test_coins.clone(), 100u64, None).unwrap();

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000_000_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000_000_000000u128),
    ];
    helper.provide_liquidity(&owner, &assets).unwrap();
    helper.app.next_block(1000);

    let err = helper.query_observe(0).unwrap_err();
    assert_eq!(
        err,
        StdError::generic_err("Querier contract error: Generic error: Buffer is empty")
    );

    let user1 = Addr::unchecked("user1");
    let offer_asset = helper.assets[&test_coins[0]].with_balance(1000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user1);

    helper
        .swap(
            &user1,
            &offer_asset,
            Some(helper.assets[&test_coins[1]].clone()),
        )
        .unwrap();

    helper.app.next_block(86400);
    assert_eq!(
        helper.query_observe(0).unwrap(),
        OracleObservation {
            timestamp: helper.app.block_info().time.seconds(),
            price: Decimal::from_str("1.000500348223145698").unwrap()
        }
    );

    let assets = vec![
        helper.assets[&test_coins[0]].with_balance(100_000000u128),
        helper.assets[&test_coins[1]].with_balance(100_000000u128),
    ];
    helper.give_me_money(&assets, &user1);

    // Imbalanced provide
    helper.provide_liquidity(&user1, &assets).unwrap();
    helper.app.next_block(14 * 86400);

    let offer_asset = helper.assets[&test_coins[1]].with_balance(10_000_000000u128);
    helper.give_me_money(&[offer_asset.clone()], &user1);
    helper
        .swap(
            &user1,
            &offer_asset,
            Some(helper.assets[&test_coins[0]].clone()),
        )
        .unwrap();

    // One more swap to trigger price update in the next step
    helper
        .swap(
            &owner,
            &offer_asset,
            Some(helper.assets[&test_coins[0]].clone()),
        )
        .unwrap();

    helper.app.next_block(86400);

    assert_eq!(
        helper.query_observe(0).unwrap(),
        OracleObservation {
            timestamp: helper.app.block_info().time.seconds(),
            price: Decimal::from_str("0.999999778261572849").unwrap()
        }
    );

    let price1 = helper.observe_price(0).unwrap();
    helper.app.next_block(10);
    // Swapping the lowest amount possible which results in positive return amount
    helper
        .swap(
            &user1,
            &helper.assets[&test_coins[1]].with_balance(2u128),
            None,
        )
        .unwrap();
    let price2 = helper.observe_price(0).unwrap();
    // With such a small swap size contract doesn't store observation
    assert_eq!(price1, price2);

    helper.app.next_block(10);
    // Swap the smallest possible amount which gets observation saved
    helper
        .swap(
            &user1,
            &helper.assets[&test_coins[1]].with_balance(1005u128),
            None,
        )
        .unwrap();
    let price3 = helper.observe_price(0).unwrap();
    // Prove that price didn't jump that much
    let diff = price3.diff(price2);
    assert!(
        diff / price2 < f64_to_dec(0.005),
        "price jumped from {price2} to {price3} which is more than 0.5%"
    );
    helper.app.next_block(10);
}
