//! Regression test: sell quotes must use the latest stored fee configuration.
//!
//! A prior class of bugs in quote paths is accidentally reading a cached or
//! default fee config after the admin mutates it. This test ensures the contract
//! re-reads storage on each quote call and reflects the updated config.

mod contract_test_env;

use contract_test_env::{
    register_creator_keys, register_test_creator, set_pricing_and_fees, test_env_with_auths,
};
use creator_keys::fee;
use creator_keys::CreatorKeysContractClient;
use soroban_sdk::{testutils::Address as _, Address, Env};

fn register_holder_with_one_key(
    env: &Env,
    client: &CreatorKeysContractClient<'_>,
    creator: &Address,
) -> Address {
    let holder = Address::generate(env);
    let price = client.get_buy_quote(creator).price;
    client.buy_key(creator, &holder, &price);
    holder
}

fn expected_sell_quote_net(
    key_price: i128,
    creator_bps: u32,
    protocol_bps: u32,
) -> (i128, i128, i128) {
    let (creator_fee, protocol_fee) =
        fee::checked_compute_fee_split(key_price, creator_bps, protocol_bps)
            .expect("fee split in range");
    let net = key_price
        .checked_sub(creator_fee)
        .and_then(|x| x.checked_sub(protocol_fee))
        .expect("net payout");
    (creator_fee, protocol_fee, net)
}

#[test]
fn sell_quote_reflects_fee_config_updates_without_stale_reads() {
    let env = test_env_with_auths();
    let (client, _) = register_creator_keys(&env);

    let key_price = 10_000_i128;
    // Initial fee config (90/10).
    let admin = set_pricing_and_fees(&env, &client, key_price, 9000, 1000);

    let creator = register_test_creator(&env, &client, "cr_fee_update");
    let holder = register_holder_with_one_key(&env, &client, &creator);

    let q_before = client.get_sell_quote(&creator, &holder);
    let (creator_fee_before, protocol_fee_before, net_before) =
        expected_sell_quote_net(key_price, 9000, 1000);
    assert_eq!(
        (
            q_before.creator_fee,
            q_before.protocol_fee,
            q_before.total_amount
        ),
        (creator_fee_before, protocol_fee_before, net_before),
        "sell quote should match initial fee config"
    );

    // Mutate fee config (80/20) using the same admin.
    client.set_fee_config(&admin, &8000_u32, &2000_u32);

    let q_after = client.get_sell_quote(&creator, &holder);
    let (creator_fee_after, protocol_fee_after, net_after) =
        expected_sell_quote_net(key_price, 8000, 2000);
    assert_eq!(
        (
            q_after.creator_fee,
            q_after.protocol_fee,
            q_after.total_amount
        ),
        (creator_fee_after, protocol_fee_after, net_after),
        "sell quote should reflect updated fee config"
    );

    assert_ne!(
        (
            q_before.creator_fee,
            q_before.protocol_fee,
            q_before.total_amount
        ),
        (
            q_after.creator_fee,
            q_after.protocol_fee,
            q_after.total_amount
        ),
        "fee config update must change sell quote output"
    );
}
