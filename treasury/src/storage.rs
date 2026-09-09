//! Instance storage for the fee rate, with TTL bumping.

use soroban_sdk::{Env, Symbol};

const ONE_DAY_LEDGERS: u32 = 17280; // assumes 5s a ledger

const LEDGER_THRESHOLD_INSTANCE: u32 = ONE_DAY_LEDGERS * 30; // ~ 30 days
const LEDGER_BUMP_INSTANCE: u32 = LEDGER_THRESHOLD_INSTANCE + ONE_DAY_LEDGERS; // ~ 31 days

const RATE_KEY: &str = "Rate";

/// Bump the instance rent for the contract
pub fn extend_instance(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(LEDGER_THRESHOLD_INSTANCE, LEDGER_BUMP_INSTANCE);
}

/// Fetch the fee rate (7 decimals). The constructor always stores one, so the `0` fallback is
/// unreachable on an initialized contract.
pub fn get_rate(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get::<Symbol, u32>(&Symbol::new(e, RATE_KEY))
        .unwrap_or(0)
}

/// Store the fee rate (7 decimals). The caller validates the range.
pub fn set_rate(e: &Env, rate: u32) {
    e.storage()
        .instance()
        .set::<Symbol, u32>(&Symbol::new(e, RATE_KEY), &rate);
}
