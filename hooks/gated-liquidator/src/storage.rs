use soroban_sdk::{contracttype, Address, Env};

const ONE_DAY_LEDGERS: u32 = 17280; // assumes 5s a ledger

const LEDGER_THRESHOLD_INSTANCE: u32 = ONE_DAY_LEDGERS * 30; // ~ 30 days
const LEDGER_BUMP_INSTANCE: u32 = LEDGER_THRESHOLD_INSTANCE + ONE_DAY_LEDGERS; // ~ 31 days

const LEDGER_THRESHOLD_USER: u32 = ONE_DAY_LEDGERS * 100; // ~ 100 days
const LEDGER_BUMP_USER: u32 = LEDGER_THRESHOLD_USER + 20 * ONE_DAY_LEDGERS; // ~ 120 days

#[derive(Clone)]
#[contracttype]
pub enum GatedLiquidatorDataKey {
    Allowed(Address),
}

/// Bump the instance rent for the contract
pub fn extend_instance(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(LEDGER_THRESHOLD_INSTANCE, LEDGER_BUMP_INSTANCE);
}

/// Whether `address` is on the list. Bumps the entry when it is.
pub fn is_allowed(e: &Env, address: &Address) -> bool {
    let key = GatedLiquidatorDataKey::Allowed(address.clone());
    if let Some(allowed) = e.storage().persistent().get::<_, bool>(&key) {
        e.storage()
            .persistent()
            .extend_ttl(&key, LEDGER_THRESHOLD_USER, LEDGER_BUMP_USER);
        allowed
    } else {
        false
    }
}

/// Put `address` on the list, or take it off. Removes the entry when delisting.
pub fn set_allowed(e: &Env, address: &Address, allowed: bool) {
    let key = GatedLiquidatorDataKey::Allowed(address.clone());
    if allowed {
        e.storage().persistent().set(&key, &true);
        e.storage()
            .persistent()
            .extend_ttl(&key, LEDGER_THRESHOLD_USER, LEDGER_BUMP_USER);
    } else {
        e.storage().persistent().remove(&key);
    }
}
