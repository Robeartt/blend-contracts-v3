use soroban_sdk::{contractclient, Address, Env};

/// The hook a backstop calls after a deposit. One hook contract per pool serves both the pool
/// and its backstop; the backstop's hook address is immutable and set at construction.
#[allow(dead_code)]
#[contractclient(name = "BackstopHookClient")]
pub trait BackstopHook {
    /// Called by the backstop after a deposit has minted `shares` to `from`.
    /// Reject by panicking: the deposit reverts.
    ///
    /// ### Arguments
    /// * `from` - The address that deposited and received the shares
    /// * `amount` - The backstop tokens deposited
    /// * `shares` - The shares minted
    fn on_backstop_deposit(e: Env, from: Address, amount: i128, shares: i128);
}
