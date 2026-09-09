use soroban_sdk::{Address, Env, Symbol};

pub struct PoolFactoryEvents {}

impl PoolFactoryEvents {
    /// Emitted when a pool and its backstop are deployed by the factory
    ///
    /// - topics - `["deploy"]`
    /// - data - `[pool_address: Address, backstop_address: Address, hook: Option<Address>]`
    ///
    /// ### Arguments
    /// * `pool_address` - The address of the pool
    /// * `backstop_address` - The address of the pool's backstop
    /// * `hook` - The hook the pool and its backstop call, if any
    pub fn deploy(
        e: &Env,
        pool_address: Address,
        backstop_address: Address,
        hook: Option<Address>,
    ) {
        let topics = (Symbol::new(e, "deploy"),);
        e.events()
            .publish(topics, (pool_address, backstop_address, hook));
    }
}
