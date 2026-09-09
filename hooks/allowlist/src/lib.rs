#![no_std]

//! Allowlist hook: a permissioned pool.
//!
//! Every party to an entry into the pool (`from`, `spender`, `to`) and every backstop depositor
//! (`from`) must be on the owner's list. Exits are never hooked by the pool or the backstop, so a
//! delisted address can always repay, withdraw, and queue and withdraw its backstop shares.

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
pub enum AllowlistError {
    /// A party to the entry is not on the list
    NotAllowed = 1500,
}

#[contract]
pub struct AllowlistHook;

#[contractimpl]
impl AllowlistHook {
    /// Initialize the hook
    ///
    /// ### Arguments
    /// * `owner` - The address that manages the list
    pub fn __constructor(e: Env, owner: Address) {
        ownable::set_owner(&e, &owner);
    }

    /// (Owner only) Put `address` on the list, or take it off
    #[only_owner]
    pub fn set_allowed(e: Env, address: Address, allowed: bool) {
        storage::extend_instance(&e);
        storage::set_allowed(&e, &address, allowed);
    }

    /// Whether `address` is on the list
    pub fn is_allowed(e: Env, address: Address) -> bool {
        storage::is_allowed(&e, &address)
    }

    /// Pool hook: every party to an entry must be listed
    pub fn on_submit(
        e: Env,
        from: Address,
        spender: Address,
        to: Address,
        _requests: Vec<Request>,
        _reserves: Vec<Reserve>,
        _positions: Positions,
    ) {
        storage::extend_instance(&e);
        require_allowed(&e, &from);
        if spender != from {
            require_allowed(&e, &spender);
        }
        if to != from && to != spender {
            require_allowed(&e, &to);
        }
    }

    /// Backstop hook: the depositor must be listed
    pub fn on_backstop_deposit(e: Env, from: Address, _amount: i128, _shares: i128) {
        storage::extend_instance(&e);
        require_allowed(&e, &from);
    }
}

#[contractimpl(contracttrait)]
impl Ownable for AllowlistHook {}

fn require_allowed(e: &Env, address: &Address) {
    if !storage::is_allowed(e, address) {
        panic_with_error!(e, AllowlistError::NotAllowed);
    }
}
