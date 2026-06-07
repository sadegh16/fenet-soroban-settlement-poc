#![no_std]

//! FENET binary outcome-market settlement contract (Soroban testnet PoC).
//!
//! A single market wraps one collateral token (`tEURC` in the PoC, a testnet
//! placeholder for EURC) and two SEP-41 outcome-share tokens, `YES` and `NO`.
//!
//! Lifecycle:
//!   * `mint(to, amount)`   - lock `amount` collateral, mint `amount` YES AND
//!                            `amount` NO to `to`. (a complete set costs 1 unit)
//!   * `merge(from, amount)`- burn `amount` YES AND `amount` NO, return
//!                            `amount` collateral. (inverse of mint)
//!   * `set_outcome(winner)`- admin-only, once-only. Resolves the market.
//!                            STUB: in production this is driven by an oracle /
//!                            resolver, which is out of scope for this PoC.
//!   * `redeem(holder, amount)` - after resolution, burn `amount` of the
//!                            WINNING share and pay out `amount` collateral.
//!
//! Core invariant while Unresolved:
//!   collateral_balance(market) == YES.total_supply == NO.total_supply
//!
//! The market is the sole minter/burner of YES and NO (wired via the token's
//! `set_minter`), so supply can only move through these entrypoints.

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, contracttype, Address, Env, MuxedAddress,
};

/// Minimal client over the outcome / collateral tokens. The signatures match
/// the OpenZeppelin SEP-41 surface used by `outcome-token` (transfer takes a
/// `MuxedAddress`, mint/burn take plain `Address`).
#[contractclient(name = "TokenClient")]
pub trait TokenInterface {
    fn transfer(env: Env, from: Address, to: MuxedAddress, amount: i128);
    fn balance(env: Env, account: Address) -> i128;
    fn total_supply(env: Env) -> i128;
    fn mint(env: Env, to: Address, amount: i128);
    fn burn(env: Env, from: Address, amount: i128);
}

#[contracttype]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Unresolved,
    Yes,
    No,
}

#[contracttype]
enum DataKey {
    Admin,
    Collateral,
    Yes,
    No,
    Outcome,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MarketError {
    NotInitialized = 1,
    InvalidAmount = 2,
    AlreadyResolved = 3,
    NotResolved = 4,
    InvalidOutcome = 5,
}

#[contract]
pub struct Market;

#[contractimpl]
impl Market {
    /// Initialize the market.
    ///
    /// * `admin`      - the only address allowed to call `set_outcome`.
    /// * `collateral` - address of the collateral token (tEURC in the PoC).
    /// * `yes`        - address of the YES share token.
    /// * `no`         - address of the NO share token.
    ///
    /// The caller must, after deployment, transfer minter rights of `yes` and
    /// `no` to this contract's address (`OutcomeToken::set_minter`).
    pub fn __constructor(e: &Env, admin: Address, collateral: Address, yes: Address, no: Address) {
        let s = e.storage().instance();
        s.set(&DataKey::Admin, &admin);
        s.set(&DataKey::Collateral, &collateral);
        s.set(&DataKey::Yes, &yes);
        s.set(&DataKey::No, &no);
        s.set(&DataKey::Outcome, &Outcome::Unresolved);
    }

    /// Buy a complete set: lock `amount` collateral, receive `amount` YES and
    /// `amount` NO. Requires `to`'s authorization. Reverts once resolved.
    pub fn mint(e: &Env, to: Address, amount: i128) {
        to.require_auth();
        require_positive(e, amount);
        require_unresolved(e);

        let collateral = Self::collateral(e);
        let market: MuxedAddress = e.current_contract_address().into();

        TokenClient::new(e, &collateral).transfer(&to, &market, &amount);
        TokenClient::new(e, &Self::yes(e)).mint(&to, &amount);
        TokenClient::new(e, &Self::no(e)).mint(&to, &amount);
    }

    /// Sell a complete set: burn `amount` YES and `amount` NO, receive `amount`
    /// collateral back. Requires `from`'s authorization. Reverts once resolved.
    pub fn merge(e: &Env, from: Address, amount: i128) {
        from.require_auth();
        require_positive(e, amount);
        require_unresolved(e);

        let market = e.current_contract_address();

        TokenClient::new(e, &Self::yes(e)).burn(&from, &amount);
        TokenClient::new(e, &Self::no(e)).burn(&from, &amount);
        let to: MuxedAddress = from.into();
        TokenClient::new(e, &Self::collateral(e)).transfer(&market, &to, &amount);
    }

    /// Resolve the market to `winner` (Yes or No). Admin-only, once-only.
    ///
    /// STUB: in production this entrypoint is replaced/driven by an oracle or
    /// dispute-resolution module (out of scope for this PoC).
    pub fn set_outcome(e: &Env, winner: Outcome) {
        let admin: Address = e
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .unwrap_or_else(|| panic_init(e));
        admin.require_auth();
        require_unresolved(e);
        if winner == Outcome::Unresolved {
            soroban_sdk::panic_with_error!(e, MarketError::InvalidOutcome);
        }
        e.storage().instance().set(&DataKey::Outcome, &winner);
    }

    /// After resolution, burn `amount` of the WINNING share held by `holder`
    /// and pay out `amount` collateral. Requires `holder`'s authorization.
    /// Reverts while Unresolved.
    pub fn redeem(e: &Env, holder: Address, amount: i128) {
        holder.require_auth();
        require_positive(e, amount);

        let outcome = Self::outcome(e);
        let winning = match outcome {
            Outcome::Yes => Self::yes(e),
            Outcome::No => Self::no(e),
            Outcome::Unresolved => soroban_sdk::panic_with_error!(e, MarketError::NotResolved),
        };

        let market = e.current_contract_address();
        TokenClient::new(e, &winning).burn(&holder, &amount);
        let to: MuxedAddress = holder.into();
        TokenClient::new(e, &Self::collateral(e)).transfer(&market, &to, &amount);
    }

    // ---- read-only accessors ----

    pub fn admin(e: &Env) -> Address {
        get_addr(e, &DataKey::Admin)
    }

    pub fn collateral(e: &Env) -> Address {
        get_addr(e, &DataKey::Collateral)
    }

    pub fn yes(e: &Env) -> Address {
        get_addr(e, &DataKey::Yes)
    }

    pub fn no(e: &Env) -> Address {
        get_addr(e, &DataKey::No)
    }

    pub fn outcome(e: &Env) -> Outcome {
        e.storage()
            .instance()
            .get(&DataKey::Outcome)
            .unwrap_or_else(|| panic_init(e))
    }
}

fn get_addr(e: &Env, key: &DataKey) -> Address {
    match e.storage().instance().get(key) {
        Some(a) => a,
        None => panic_init(e),
    }
}

fn panic_init(e: &Env) -> ! {
    soroban_sdk::panic_with_error!(e, MarketError::NotInitialized)
}

fn require_positive(e: &Env, amount: i128) {
    if amount <= 0 {
        soroban_sdk::panic_with_error!(e, MarketError::InvalidAmount);
    }
}

fn require_unresolved(e: &Env) {
    if Market::outcome(e) != Outcome::Unresolved {
        soroban_sdk::panic_with_error!(e, MarketError::AlreadyResolved);
    }
}

#[cfg(test)]
mod test;
