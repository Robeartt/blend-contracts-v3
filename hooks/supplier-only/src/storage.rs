use soroban_sdk::{unwrap::UnwrapOptimized, Address, Env, Symbol};

const ONE_DAY_LEDGERS: u32 = 17280; // assumes 5s a ledger

const LEDGER_THRESHOLD_INSTANCE: u32 = ONE_DAY_LEDGERS * 30; // ~ 30 days
const LEDGER_BUMP_INSTANCE: u32 = LEDGER_THRESHOLD_INSTANCE + ONE_DAY_LEDGERS; // ~ 31 days

const SUPPLIER_KEY: &str = "Supplier";

/// Bump the instance rent for the contract
pub fn extend_instance(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(LEDGER_THRESHOLD_INSTANCE, LEDGER_BUMP_INSTANCE);
}

/// Fetch the supplier
pub fn get_supplier(e: &Env) -> Address {
    e.storage()
        .instance()
        .get::<Symbol, Address>(&Symbol::new(e, SUPPLIER_KEY))
        .unwrap_optimized()
}

/// Set the supplier
pub fn set_supplier(e: &Env, supplier: &Address) {
    e.storage()
        .instance()
        .set::<Symbol, Address>(&Symbol::new(e, SUPPLIER_KEY), supplier);
}
