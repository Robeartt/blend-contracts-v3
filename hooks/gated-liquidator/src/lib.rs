#![no_std]

//! Gated liquidator hook: only listed addresses may fill auctions.
//!
//! Requests of type 6 (fill user liquidation), 7 (fill bad debt) and 8 (fill interest) are
//! accepted only when `from` is on the owner's list. Every other request passes, and the backstop
//! has no rule. Meant for pools whose collateral may only be held by known parties.

mod storage;

#[cfg(test)]
mod test;

/// The pool's types, from its WASM spec
pub mod pool {
    soroban_sdk::contractimport!(file = "../../target/wasm32v1-none/optimized/pool.wasm");
}

use soroban_sdk::{contract, contracterror, contractimpl, panic_with_error, Address, Env, Vec};
use stellar_access::ownable::{self as ownable, Ownable};
use stellar_macros::only_owner;

use pool::{Positions, Request, Reserve};

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum GatedLiquidatorError {
    /// The filler is not on the list
    NotAllowed = 1500,
}

#[contract]
pub struct GatedLiquidatorHook;

#[contractimpl]
impl GatedLiquidatorHook {
    /// Initialize the hook
    ///
    /// ### Arguments
    /// * `owner` - The address that manages the list
    pub fn __constructor(e: Env, owner: Address) {
        ownable::set_owner(&e, &owner);
    }

    /// (Owner only) Put `address` on the list of liquidators, or take it off
    #[only_owner]
    pub fn set_allowed(e: Env, address: Address, allowed: bool) {
        storage::extend_instance(&e);
        storage::set_allowed(&e, &address, allowed);
    }

    /// Whether `address` may fill auctions
    pub fn is_allowed(e: Env, address: Address) -> bool {
        storage::is_allowed(&e, &address)
    }

    /// Pool hook: a batch with an auction fill must come from a listed address
    pub fn on_submit(
        e: Env,
        from: Address,
        _spender: Address,
        _to: Address,
        requests: Vec<Request>,
        _reserves: Vec<Reserve>,
        _positions: Positions,
    ) {
        storage::extend_instance(&e);
        for request in requests.iter() {
            if (6..=8).contains(&request.request_type) && !storage::is_allowed(&e, &from) {
                panic_with_error!(&e, GatedLiquidatorError::NotAllowed);
            }
        }
    }

    /// Backstop hook: no rule
    pub fn on_backstop_deposit(_e: Env, _from: Address, _amount: i128, _shares: i128) {}
}

#[contractimpl(contracttrait)]
impl Ownable for GatedLiquidatorHook {}
