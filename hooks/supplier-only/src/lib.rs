#![no_std]

//! Supplier-only hook: a pool-operator fee with no core change.
//!
//! `Supply` (request type 0) is accepted only from the operator's supplier, meant to be a fee
//! vault (Script3 `fee-vault-v2`) that takes the operator's cut of depositors' interest before

//! passing the rest on. `SupplyCollateral` bypasses it by design: collateral earns interest too,
//! but gating it would gate borrowing. Every other request passes, and the backstop has no rule.

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
pub enum SupplierOnlyError {
    /// A supply came from an address other than the supplier
    NotSupplier = 1500,
}

#[contract]
pub struct SupplierOnlyHook;

#[contractimpl]
impl SupplierOnlyHook {
    /// Initialize the hook
    ///
    /// ### Arguments
    /// * `owner` - The address that may change the supplier
    /// * `supplier` - The only address whose `Supply` requests are accepted
    pub fn __constructor(e: Env, owner: Address, supplier: Address) {
        ownable::set_owner(&e, &owner);
        storage::set_supplier(&e, &supplier);
    }

    /// (Owner only) Change the supplier
    #[only_owner]
    pub fn set_supplier(e: Env, supplier: Address) {
        storage::extend_instance(&e);
        storage::set_supplier(&e, &supplier);
    }

    /// The only address whose `Supply` requests are accepted
    pub fn get_supplier(e: Env) -> Address {
        storage::get_supplier(&e)
    }

    /// Pool hook: a batch with a `Supply` must come from the supplier
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
            if request.request_type == 0 && from != storage::get_supplier(&e) {
                panic_with_error!(&e, SupplierOnlyError::NotSupplier);
            }
        }
    }

    /// Backstop hook: no rule
    pub fn on_backstop_deposit(_e: Env, _from: Address, _amount: i128, _shares: i128) {}
}

#[contractimpl(contracttrait)]
impl Ownable for SupplierOnlyHook {}
