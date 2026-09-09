use crate::{
    errors::PoolFactoryError,
    events::PoolFactoryEvents,
    storage::{self, PoolInitMeta},
};
use soroban_sdk::{
    contract, contractclient, contractimpl, contracttype, panic_with_error, Address, Bytes, BytesN,
    Env, IntoVal, String,
};

const SCALAR_7: u32 = 1_0000000;

/// The pool backstop to deploy alongside a pool
#[derive(Clone)]
#[contracttype]
pub struct BackstopInit {
    pub backstop_token: Address, // the backstop token
    pub decimals_offset: u32, // extra decimals on backstop shares over the backstop token (ERC-4626 virtual offset)
    pub name: String,         // the backstop share token name
    pub symbol: String,       // the backstop share token symbol
}

#[contract]
pub struct PoolFactoryContract;

#[contractclient(name = "PoolFactoryClient")]
pub trait PoolFactory {
    /// Deploys and initializes a lending pool and its backstop as a bound pair
    ///
    /// Returns the pool address. The backstop address is available from the pool with `get_backstop`.
    ///
    /// ### Arguments
    /// * `admin` - The admin address for the pool
    /// * `name` - The name of the pool
    /// * `salt` - The salt for the pool address
    /// * `oracle` - The oracle address for the pool
    /// * `backstop_take_rate` - The backstop take rate for the pool (7 decimals)
    /// * `max_positions` - The maximum user positions supported by the pool
    /// * `min_collateral` - The minimum collateral required for a borrow position (oracle decimals)
    /// * `min_backstop` - The backstop tokens required for the pool to activate (backstop token decimals). 0 for no requirement.
    /// * `backstop` - The backstop to deploy for the pool
    /// * `hook` - The hook the pool and its backstop call, if any
    #[allow(clippy::too_many_arguments)]
    fn deploy(
        e: Env,
        admin: Address,
        name: String,
        salt: BytesN<32>,
        oracle: Address,
        backstop_take_rate: u32,
        max_positions: u32,
        min_collateral: i128,
        min_backstop: i128,
        backstop: BackstopInit,
        hook: Option<Address>,
    ) -> Address;

    /// Checks if contract address was deployed by the factory
    ///
    /// Returns true if pool was deployed by factory and false otherwise
    ///
    /// ### Arguments
    /// * `pool_id` - The contract address to be checked
    fn is_pool(e: Env, pool_id: Address) -> bool;

    /// Fetch the WASM hashes and the treasury every deployed pool and backstop is initialized with
    fn get_pool_init_meta(e: Env) -> PoolInitMeta;
}

#[contractimpl]
impl PoolFactoryContract {
    /// Construct the pool factory contract
    ///
    /// ### Arguments
    /// * `pool_init_meta` - The pool initialization metadata    
    pub fn __constructor(e: Env, pool_init_meta: PoolInitMeta) {
        storage::set_pool_init_meta(&e, &pool_init_meta);
    }
}

#[contractimpl]
impl PoolFactory for PoolFactoryContract {
    fn deploy(
        e: Env,
        admin: Address,
        name: String,
        salt: BytesN<32>,
        oracle: Address,
        backstop_take_rate: u32,
        max_positions: u32,
        min_collateral: i128,
        min_backstop: i128,
        backstop: BackstopInit,
        hook: Option<Address>,
    ) -> Address {
        admin.require_auth();
        storage::extend_instance(&e);
        let pool_init_meta = storage::get_pool_init_meta(&e);

        // verify backstop take rate is within [0,1) with 7 decimals
        if backstop_take_rate >= SCALAR_7 {
            panic_with_error!(&e, PoolFactoryError::InvalidPoolInitArgs);
        }

        // verify max positions is at least 2 and less than 64
        // pools have a max of 30 reserves, so 60 is the max number of positions
        if max_positions < 2 || max_positions > 60 {
            panic_with_error!(&e, PoolFactoryError::InvalidPoolInitArgs);
        }

        // verify max positions is at least 2 and less than 64
        // pools have a max of 50 reserves, so 100 is the max number of positions
        if min_collateral < 0 {
            panic_with_error!(&e, PoolFactoryError::InvalidPoolInitArgs);
        }

        if min_backstop < 0 {
            panic_with_error!(&e, PoolFactoryError::InvalidPoolInitArgs);
        }

        let mut as_u8s: [u8; 56] = [0; 56];
        admin.to_string().copy_into_slice(&mut as_u8s);
        let mut salt_as_bytes: Bytes = salt.into_val(&e);
        salt_as_bytes.extend_from_array(&as_u8s);
        let pool_salt = e.crypto().keccak256(&salt_as_bytes).to_bytes();

        // the backstop is bound to the pool's address, which is known before the pool is deployed
        let pool_address = e
            .deployer()
            .with_current_contract(pool_salt.clone())
            .deployed_address();

        let mut backstop_salt_as_bytes: Bytes = pool_salt.clone().into_val(&e);
        backstop_salt_as_bytes.extend_from_array(b"backstop");
        let backstop_salt = e.crypto().keccak256(&backstop_salt_as_bytes).to_bytes();
        let backstop_address = e.deployer().with_current_contract(backstop_salt).deploy_v2(
            pool_init_meta.backstop_hash,
            (
                pool_address.clone(),
                backstop.backstop_token,
                pool_init_meta.treasury,
                backstop.decimals_offset,
                backstop.name,
                backstop.symbol,
                hook.clone(),
            ),
        );

        let deployed_pool_address = e.deployer().with_current_contract(pool_salt).deploy_v2(
            pool_init_meta.pool_hash,
            (
                admin,
                name,
                oracle,
                backstop_take_rate,
                max_positions,
                min_collateral,
                min_backstop,
                backstop_address.clone(),
                hook.clone(),
            ),
        );
        if deployed_pool_address != pool_address {
            panic_with_error!(&e, PoolFactoryError::InternalError);
        }

        storage::set_deployed(&e, &pool_address);

        PoolFactoryEvents::deploy(&e, pool_address.clone(), backstop_address, hook);
        pool_address
    }

    fn is_pool(e: Env, pool_address: Address) -> bool {
        storage::extend_instance(&e);
        storage::is_deployed(&e, &pool_address)
    }

    fn get_pool_init_meta(e: Env) -> PoolInitMeta {
        storage::extend_instance(&e);
        storage::get_pool_init_meta(&e)
    }
}
