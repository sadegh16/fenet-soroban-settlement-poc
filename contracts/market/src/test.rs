#![cfg(test)]

use crate::{Market, MarketClient, Outcome};
use outcome_token::{OutcomeToken, OutcomeTokenClient};
use soroban_sdk::{testutils::Address as _, Address, Env, String};

const DECIMALS: u32 = 7;

struct Setup<'a> {
    env: Env,
    admin: Address,
    user: Address,
    collateral: OutcomeTokenClient<'a>,
    yes: OutcomeTokenClient<'a>,
    no: OutcomeTokenClient<'a>,
    market: MarketClient<'a>,
    market_addr: Address,
}

fn setup() -> Setup<'static> {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let user = Address::generate(&env);
    let deployer = Address::generate(&env);

    // Collateral token (tEURC): deployer is the minter / faucet.
    let collateral_addr = env.register(
        OutcomeToken,
        (
            deployer.clone(),
            DECIMALS,
            String::from_str(&env, "Test EURC"),
            String::from_str(&env, "tEURC"),
        ),
    );
    // YES / NO: deployer is the initial minter, handed to the market below.
    let yes_addr = env.register(
        OutcomeToken,
        (
            deployer.clone(),
            DECIMALS,
            String::from_str(&env, "YES Share"),
            String::from_str(&env, "YES"),
        ),
    );
    let no_addr = env.register(
        OutcomeToken,
        (
            deployer.clone(),
            DECIMALS,
            String::from_str(&env, "NO Share"),
            String::from_str(&env, "NO"),
        ),
    );

    let market_addr = env.register(
        Market,
        (
            admin.clone(),
            collateral_addr.clone(),
            yes_addr.clone(),
            no_addr.clone(),
        ),
    );

    let collateral = OutcomeTokenClient::new(&env, &collateral_addr);
    let yes = OutcomeTokenClient::new(&env, &yes_addr);
    let no = OutcomeTokenClient::new(&env, &no_addr);

    // Wire minter rights of YES/NO to the market (the deploy-time handoff).
    yes.set_minter(&market_addr);
    no.set_minter(&market_addr);

    // Faucet the user some collateral and pre-approve nothing (transfers are
    // done by require_auth in tests via mock_all_auths).
    collateral.mint(&user, &1_000);

    let market = MarketClient::new(&env, &market_addr);

    Setup {
        env,
        admin,
        user,
        collateral,
        yes,
        no,
        market,
        market_addr,
    }
}

/// Core invariant while unresolved.
fn assert_invariant(s: &Setup) {
    let bal = s.collateral.balance(&s.market_addr);
    assert_eq!(bal, s.yes.total_supply());
    assert_eq!(bal, s.no.total_supply());
}

#[test]
fn mint_then_merge_round_trip() {
    let s = setup();

    s.market.mint(&s.user, &100);
    assert_eq!(s.yes.balance(&s.user), 100);
    assert_eq!(s.no.balance(&s.user), 100);
    assert_eq!(s.collateral.balance(&s.user), 900);
    assert_eq!(s.collateral.balance(&s.market_addr), 100);
    assert_invariant(&s);

    s.market.merge(&s.user, &40);
    assert_eq!(s.yes.balance(&s.user), 60);
    assert_eq!(s.no.balance(&s.user), 60);
    assert_eq!(s.collateral.balance(&s.user), 940);
    assert_invariant(&s);

    s.market.merge(&s.user, &60);
    assert_eq!(s.yes.balance(&s.user), 0);
    assert_eq!(s.no.balance(&s.user), 0);
    assert_eq!(s.collateral.balance(&s.user), 1_000);
    assert_eq!(s.collateral.balance(&s.market_addr), 0);
    assert_invariant(&s);
}

#[test]
fn resolve_yes_then_redeem() {
    let s = setup();
    s.market.mint(&s.user, &100);

    s.market.set_outcome(&Outcome::Yes);
    assert_eq!(s.market.outcome(), Outcome::Yes);

    // Redeem winning YES 1:1 for collateral; losing NO stays worthless.
    s.market.redeem(&s.user, &100);
    assert_eq!(s.yes.balance(&s.user), 0);
    assert_eq!(s.no.balance(&s.user), 100); // worthless leftovers
    assert_eq!(s.collateral.balance(&s.user), 1_000);
    assert_eq!(s.collateral.balance(&s.market_addr), 0);
}

#[test]
fn resolve_no_then_redeem() {
    let s = setup();
    s.market.mint(&s.user, &100);

    s.market.set_outcome(&Outcome::No);
    s.market.redeem(&s.user, &100);
    assert_eq!(s.no.balance(&s.user), 0);
    assert_eq!(s.yes.balance(&s.user), 100);
    assert_eq!(s.collateral.balance(&s.user), 1_000);
}

#[test]
fn partial_redeem() {
    let s = setup();
    s.market.mint(&s.user, &100);
    s.market.set_outcome(&Outcome::Yes);

    s.market.redeem(&s.user, &30);
    assert_eq!(s.yes.balance(&s.user), 70);
    assert_eq!(s.collateral.balance(&s.user), 930);
    assert_eq!(s.collateral.balance(&s.market_addr), 70);
}

// ---- failure / revert cases ----

#[test]
#[should_panic]
fn mint_after_resolution_reverts() {
    let s = setup();
    s.market.set_outcome(&Outcome::Yes);
    s.market.mint(&s.user, &10);
}

#[test]
#[should_panic]
fn merge_after_resolution_reverts() {
    let s = setup();
    s.market.mint(&s.user, &50);
    s.market.set_outcome(&Outcome::Yes);
    s.market.merge(&s.user, &10);
}

#[test]
#[should_panic]
fn redeem_before_resolution_reverts() {
    let s = setup();
    s.market.mint(&s.user, &50);
    s.market.redeem(&s.user, &10);
}

#[test]
#[should_panic]
fn merge_more_than_held_reverts() {
    let s = setup();
    s.market.mint(&s.user, &50);
    s.market.merge(&s.user, &51);
}

#[test]
#[should_panic]
fn redeem_more_than_held_reverts() {
    let s = setup();
    s.market.mint(&s.user, &50);
    s.market.set_outcome(&Outcome::Yes);
    s.market.redeem(&s.user, &51);
}

#[test]
#[should_panic]
fn set_outcome_twice_reverts() {
    let s = setup();
    s.market.set_outcome(&Outcome::Yes);
    s.market.set_outcome(&Outcome::No);
}

#[test]
#[should_panic]
fn set_outcome_unresolved_reverts() {
    let s = setup();
    s.market.set_outcome(&Outcome::Unresolved);
}

#[test]
#[should_panic]
fn mint_zero_reverts() {
    let s = setup();
    s.market.mint(&s.user, &0);
}

#[test]
#[should_panic]
fn mint_negative_reverts() {
    let s = setup();
    s.market.mint(&s.user, &-5);
}

#[test]
fn set_outcome_requires_admin_auth() {
    // With mock_all_auths the call succeeds; assert the recorded auth is the
    // admin, proving the entrypoint is admin-gated.
    let s = setup();
    s.market.set_outcome(&Outcome::Yes);
    let auths = s.env.auths();
    assert_eq!(auths.first().unwrap().0, s.admin);
}
