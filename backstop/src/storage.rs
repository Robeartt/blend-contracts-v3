use soroban_sdk::{contracttype, unwrap::UnwrapOptimized, vec, Address, Env, Symbol, Vec};

use crate::queue::Q4W;

/********** Ledger Thresholds **********/

const ONE_DAY_LEDGERS: u32 = 17280; // assumes 5s a ledger

const LEDGER_THRESHOLD_INSTANCE: u32 = ONE_DAY_LEDGERS * 30; // ~ 30 days
const LEDGER_BUMP_INSTANCE: u32 = LEDGER_THRESHOLD_INSTANCE + ONE_DAY_LEDGERS; // ~ 31 days

const LEDGER_THRESHOLD_USER: u32 = ONE_DAY_LEDGERS * 100; // ~ 100 days
const LEDGER_BUMP_USER: u32 = LEDGER_THRESHOLD_USER + 20 * ONE_DAY_LEDGERS; // ~ 120 days

/********** Storage Keys **********/

const POOL_KEY: &str = "Pool";
const HOOK_KEY: &str = "Hook";
const TREASURY_KEY: &str = "Treasury";
const TOTAL_Q4W_KEY: &str = "TotalQ4W";

#[derive(Clone)]
#[contracttype]
pub enum BackstopDataKey {
    // A user's withdrawal queue
    Q4W(Address),
}

/********** Instance **********/

/// Bump the instance rent for the contract
pub fn extend_instance(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(LEDGER_THRESHOLD_INSTANCE, LEDGER_BUMP_INSTANCE);
}

/// Fetch the pool this backstop backs
pub fn get_pool(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&Symbol::new(e, POOL_KEY))
        .unwrap_optimized()
}

/// Set the pool this backstop backs
pub fn set_pool(e: &Env, pool: &Address) {
    e.storage()
        .instance()
        .set::<Symbol, Address>(&Symbol::new(e, POOL_KEY), pool);
}

/// Fetch the hook called after deposits, if any
pub fn get_hook(e: &Env) -> Option<Address> {
    e.storage()
        .instance()
        .get(&Symbol::new(e, HOOK_KEY))
        .unwrap_optimized()
}

/// Set the hook called after deposits
pub fn set_hook(e: &Env, hook: &Option<Address>) {
    e.storage()
        .instance()
        .set::<Symbol, Option<Address>>(&Symbol::new(e, HOOK_KEY), hook);
}

/// Fetch the treasury that takes the protocol's share of donations
pub fn get_treasury(e: &Env) -> Address {
    e.storage()
        .instance()
        .get(&Symbol::new(e, TREASURY_KEY))
        .unwrap_optimized()
}

/// Set the treasury
pub fn set_treasury(e: &Env, treasury: &Address) {
    e.storage()
        .instance()
        .set::<Symbol, Address>(&Symbol::new(e, TREASURY_KEY), treasury);
}

/// Fetch the total shares queued for withdrawal
pub fn get_total_q4w(e: &Env) -> i128 {
    e.storage()
        .instance()
        .get(&Symbol::new(e, TOTAL_Q4W_KEY))
        .unwrap_or(0)
}

/// Set the total shares queued for withdrawal
pub fn set_total_q4w(e: &Env, total_q4w: i128) {
    e.storage()
        .instance()
        .set::<Symbol, i128>(&Symbol::new(e, TOTAL_Q4W_KEY), &total_q4w);
}

/********** User Queues **********/

/// Fetch a user's withdrawal queue
pub fn get_user_q4w(e: &Env, user: &Address) -> Vec<Q4W> {
    let key = BackstopDataKey::Q4W(user.clone());
    if let Some(q4w) = e
        .storage()
        .persistent()
        .get::<BackstopDataKey, Vec<Q4W>>(&key)
    {
        e.storage()
            .persistent()
            .extend_ttl(&key, LEDGER_THRESHOLD_USER, LEDGER_BUMP_USER);
        q4w
    } else {
        vec![e]
    }
}

/// Set a user's withdrawal queue. An empty queue removes the entry.
pub fn set_user_q4w(e: &Env, user: &Address, q4w: &Vec<Q4W>) {
    let key = BackstopDataKey::Q4W(user.clone());
    if q4w.is_empty() {
        e.storage().persistent().remove(&key);
    } else {
        e.storage()
            .persistent()
            .set::<BackstopDataKey, Vec<Q4W>>(&key, q4w);
        e.storage()
            .persistent()
            .extend_ttl(&key, LEDGER_THRESHOLD_USER, LEDGER_BUMP_USER);
    }
}
