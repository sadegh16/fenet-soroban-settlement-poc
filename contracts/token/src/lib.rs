#![no_std]

//! Outcome / collateral token for the FENET settlement PoC.
//!
//! This is a standard SEP-41 fungible token built on the audited OpenZeppelin
//! Stellar `fungible` module (`Base` + `FungibleBurnable`). Token logic is NOT
//! hand-rolled; only a thin `minter` access gate is added on top so that the
//! market contract can be the sole entity allowed to `mint` / `burn` shares.
//!
//! The same contract is reused for three on-chain roles in the PoC:
//!   * `tEURC`  - a testnet placeholder for EURC (the deployer is the minter
//!                and can faucet test balances to users).
//!   * `YES`    - the YES outcome share (market is the minter).
//!   * `NO`     - the NO outcome share (market is the minter).

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, panic_with_error, Address, Env,
    MuxedAddress, String,
};
use stellar_tokens::fungible::{burnable::FungibleBurnable, Base, FungibleToken};

#[contracttype]
enum DataKey {
    Minter,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TokenError {
    NotInitialized = 1,
}

#[contract]
pub struct OutcomeToken;

#[contractimpl]
impl OutcomeToken {
    /// Initialize the token.
    ///
    /// * `minter`   - the only address allowed to call `mint`. For YES/NO this
    ///                is wired to the market contract via `set_minter` after the
    ///                market is deployed.
    /// * `decimals` - number of decimals (e.g. 7).
    /// * `name`     - human readable name.
    /// * `symbol`   - ticker symbol.
    pub fn __constructor(e: &Env, minter: Address, decimals: u32, name: String, symbol: String) {
        Base::set_metadata(e, decimals, name, symbol);
        e.storage().instance().set(&DataKey::Minter, &minter);
    }

    /// Current minter (owner of supply control).
    pub fn minter(e: &Env) -> Address {
        match e.storage().instance().get(&DataKey::Minter) {
            Some(m) => m,
            None => panic_with_error!(e, TokenError::NotInitialized),
        }
    }

    /// Hand minter rights to a new address. Callable only by the current minter.
    /// Used once at deploy time to transfer YES/NO minting from the deployer to
    /// the market contract.
    pub fn set_minter(e: &Env, new_minter: Address) {
        let current = Self::minter(e);
        current.require_auth();
        e.storage().instance().set(&DataKey::Minter, &new_minter);
    }

    /// Mint `amount` tokens to `to`. Callable only by the current minter.
    pub fn mint(e: &Env, to: Address, amount: i128) {
        let minter = Self::minter(e);
        minter.require_auth();
        Base::mint(e, &to, amount);
    }
}

#[contractimpl(contracttrait)]
impl FungibleToken for OutcomeToken {
    type ContractType = Base;
}

#[contractimpl(contracttrait)]
impl FungibleBurnable for OutcomeToken {}
