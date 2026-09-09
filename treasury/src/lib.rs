#![no_std]

//! Blend v3 treasury contract.
//!
//! Holds the protocol fee rate and the collected fees. Every pool the factory deploys reads the
//! rate with [`Treasury::get_rate`] when an interest auction fills and pays the treasury's share
//! of the bid directly. The owner withdraws the accumulated balance.
//!
//! Mirrors the Zenex treasury; the rate is a 7 decimal `u32` here because every other rate on
//! the pool's ABI is.

mod storage;

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractclient, contracterror, contractimpl, panic_with_error, token::TokenClient,
    Address, Env,
};
use stellar_access::ownable::{self as ownable, Ownable};
use stellar_macros::only_owner;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TreasuryError {
    /// The fee rate is above [`MAX_PROTOCOL_RATE`].
    InvalidRate = 1400,
}

/// Fixed-point scale of the fee rate: `SCALAR_7` is 100%.
pub const SCALAR_7: u32 = 1_0000000;

/// The largest share of an interest auction payout the treasury can take (7 decimals).
///
/// The backstop clamps whatever the treasury returns to the same value, so this is a convenience
/// check for the owner, not the guarantee. Keep the two in sync.
pub const MAX_PROTOCOL_RATE: u32 = 0_5000000;

/// Holds the protocol fee rate and the collected fees.
#[contract]
pub struct TreasuryContract;

/// The treasury's cross-contract surface.
#[contractclient(name = "TreasuryClient")]
pub trait Treasury {
    /// Returns the protocol fee rate: the share of each interest auction payout the treasury
    /// takes, as a 7 decimal fraction.
    fn get_rate(e: Env) -> u32;

    /// (Owner only) Sets the protocol fee rate.
    ///
    /// ### Arguments
    /// * `rate` - The new fee rate as a 7 decimal fraction, in `[0, MAX_PROTOCOL_RATE]`
    ///
    /// ### Panics
    /// * `InvalidRate` if `rate` is above `MAX_PROTOCOL_RATE`
    fn set_rate(e: Env, rate: u32);

    /// (Owner only) Withdraws collected protocol fees.
    ///
    /// ### Arguments
    /// * `token` - The token contract to withdraw from
    /// * `to` - The recipient address
    /// * `amount` - The amount to withdraw, in the token's decimals
    fn withdraw(e: Env, token: Address, to: Address, amount: i128);
}

#[contractimpl]
impl TreasuryContract {
    /// Initializes the treasury with an owner and a fee rate.
    ///
    /// ### Arguments
    /// * `owner` - The address that may change the rate and withdraw fees
    /// * `rate` - The protocol fee rate as a 7 decimal fraction, in `[0, MAX_PROTOCOL_RATE]`
    ///
    /// ### Panics
    /// * `InvalidRate` if `rate` is above `MAX_PROTOCOL_RATE`
    pub fn __constructor(e: Env, owner: Address, rate: u32) {
        require_valid_rate(&e, rate);
        ownable::set_owner(&e, &owner);
        storage::set_rate(&e, rate);
    }
}

#[contractimpl]
impl Treasury for TreasuryContract {
    fn get_rate(e: Env) -> u32 {
        storage::extend_instance(&e);
        storage::get_rate(&e)
    }

    #[only_owner]
    fn set_rate(e: Env, rate: u32) {
        storage::extend_instance(&e);
        require_valid_rate(&e, rate);
        storage::set_rate(&e, rate);
    }

    #[only_owner]
    fn withdraw(e: Env, token: Address, to: Address, amount: i128) {
        storage::extend_instance(&e);
        TokenClient::new(&e, &token).transfer(&e.current_contract_address(), &to, &amount);
    }
}

#[contractimpl(contracttrait)]
impl Ownable for TreasuryContract {}

fn require_valid_rate(e: &Env, rate: u32) {
    if rate > MAX_PROTOCOL_RATE {
        panic_with_error!(e, TreasuryError::InvalidRate);
    }
}
