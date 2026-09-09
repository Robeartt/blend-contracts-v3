mod backstop_contract {
    soroban_sdk::contractimport!(file = "../target/wasm32v1-none/optimized/backstop.wasm");
}
pub use backstop_contract::WASM as BACKSTOP_WASM;
