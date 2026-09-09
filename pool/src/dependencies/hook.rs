use soroban_sdk::{contractclient, Address, Env, Vec};

use crate::{pool::Reserve, Positions, Request};

/// The hook a pool calls after a submit. One hook contract per pool serves both the pool and its
/// backstop; the pool's hook address is immutable and set at construction.
///
/// The pool is on the call stack while the hook runs, so the hook cannot call it back: everything
/// about the pool is passed in. The backstop, the oracle and the factory are not on the stack and
/// can be read.
#[allow(dead_code)]
#[contractclient(name = "PoolHookClient")]
pub trait PoolHook {
    /// Called at most once per submit or flash loan, after every request is applied, the health
    /// check has passed and the tokens have moved, when the batch contains at least one entry:
    /// Supply, SupplyCollateral, Borrow, an auction fill or a flash loan. Batches of exits only
    /// (Withdraw, WithdrawCollateral, Repay, DeleteLiquidationAuction) are never hooked.
    ///
    /// Reject by panicking: the whole batch reverts.
    ///
    /// ### Arguments
    /// * `from` - The address whose positions were modified
    /// * `spender` - The address that sent tokens to the pool
    /// * `to` - The address that received tokens from the pool
    /// * `requests` - The batch as submitted, in order, exits included
    /// * `reserves` - Every reserve the batch touched, post-accrual and post-action
    /// * `positions` - `from`'s positions after the batch
    fn on_submit(
        e: Env,
        from: Address,
        spender: Address,
        to: Address,
        requests: Vec<Request>,
        reserves: Vec<Reserve>,
        positions: Positions,
    );
}
