#![cfg(test)]

use sep_41_token::testutils::{MockTokenClient, MockTokenWASM};
use soroban_sdk::{
    testutils::{Address as _, BytesN as _, Events},
    vec, Address, BytesN, Env, IntoVal, String, Symbol,
};

use crate::{BackstopInit, PoolFactoryClient, PoolFactoryContract, PoolInitMeta};

mod pool {
    soroban_sdk::contractimport!(file = "../target/wasm32v1-none/optimized/pool.wasm");
}

mod backstop {
    soroban_sdk::contractimport!(file = "../target/wasm32v1-none/optimized/backstop.wasm");
}

const SCALAR_7: i128 = 1_0000000;

/// Register the factory with both WASMs uploaded, and a 7 decimal mock token as the backstop token
fn setup<'a>(e: &Env) -> (Address, PoolFactoryClient<'a>, Address) {
    let pool_hash = e.deployer().upload_contract_wasm(pool::WASM);
    let backstop_hash = e.deployer().upload_contract_wasm(backstop::WASM);
    let pool_init_meta = PoolInitMeta {
        pool_hash,
        backstop_hash,
        treasury: Address::generate(e),
    };
    let pool_factory_address = e.register(PoolFactoryContract {}, (pool_init_meta,));
    let pool_factory_client = PoolFactoryClient::new(e, &pool_factory_address);

    let asset = e.register(MockTokenWASM, ());
    MockTokenClient::new(e, &asset).initialize(
        &Address::generate(e),
        &7,
        &"USDC".into_val(e),
        &"USDC".into_val(e),
    );
    (pool_factory_address, pool_factory_client, asset)
}

fn backstop_init(e: &Env, asset: &Address) -> BackstopInit {
    BackstopInit {
        backstop_token: asset.clone(),
        decimals_offset: 0,
        name: String::from_str(e, "Backstop Share"),
        symbol: String::from_str(e, "BSS"),
    }
}

#[test]
fn test_pool_factory() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    e.mock_all_auths_allowing_non_root_auth();

    let (pool_factory_address, pool_factory_client, asset) = setup(&e);

    let bombadil = Address::generate(&e);
    let oracle = Address::generate(&e);
    let backstop_rate: u32 = 0_1000000;
    let max_positions: u32 = 6;
    let min_collateral: i128 = 1_0000000;
    let min_backstop: i128 = 100_000 * SCALAR_7;
    let hook = Address::generate(&e);

    let name1 = String::from_str(&e, "pool1");
    let name2 = String::from_str(&e, "pool2");
    let salt = BytesN::<32>::random(&e);

    let deployed_pool_address_1 = pool_factory_client.deploy(
        &bombadil,
        &name1,
        &salt,
        &oracle,
        &backstop_rate,
        &max_positions,
        &min_collateral,
        &min_backstop,
        &backstop_init(&e, &asset),
        &Some(hook.clone()),
    );
    // snapshot the events before reading storage, which starts a new invocation
    let events = e.events().all();
    let backstop_address_1 = e.as_contract(&deployed_pool_address_1, || {
        e.storage()
            .instance()
            .get::<_, Address>(&Symbol::new(&e, "Backstop"))
            .unwrap()
    });
    assert_eq!(
        events.filter_by_contract(&pool_factory_address),
        vec![
            &e,
            (
                pool_factory_address.clone(),
                (Symbol::new(&e, "deploy"),).into_val(&e),
                (
                    deployed_pool_address_1.clone(),
                    backstop_address_1.clone(),
                    Some(hook.clone())
                )
                    .into_val(&e)
            )
        ]
    );

    let salt = BytesN::<32>::random(&e);
    let deployed_pool_address_2 = pool_factory_client.deploy(
        &bombadil,
        &name2,
        &salt,
        &oracle,
        &backstop_rate,
        &max_positions,
        &min_collateral,
        &0,
        &backstop_init(&e, &asset),
        &None,
    );
    let backstop_address_2 = pool::Client::new(&e, &deployed_pool_address_2).get_backstop();

    // the pool is initialized and bound to its backstop
    assert_eq!(
        pool::Client::new(&e, &deployed_pool_address_1).get_backstop(),
        backstop_address_1
    );
    e.as_contract(&deployed_pool_address_1, || {
        assert_eq!(
            e.storage()
                .instance()
                .get::<_, Address>(&Symbol::new(&e, "Admin"))
                .unwrap(),
            bombadil.clone()
        );
        assert_eq!(
            e.storage()
                .instance()
                .get::<_, Address>(&Symbol::new(&e, "Backstop"))
                .unwrap(),
            backstop_address_1.clone()
        );
        assert_eq!(
            e.storage()
                .instance()
                .get::<_, pool::PoolConfig>(&Symbol::new(&e, "Config"))
                .unwrap(),
            pool::PoolConfig {
                oracle: oracle,
                min_collateral: min_collateral,
                bstop_rate: backstop_rate,
                status: 6,
                max_positions: 6
            }
        );
    });

    // the backstop is initialized and bound to its pool, paying the factory's treasury
    let treasury = pool_factory_client.get_pool_init_meta().treasury;
    let backstop_client_1 = backstop::Client::new(&e, &backstop_address_1);
    assert_eq!(backstop_client_1.pool(), deployed_pool_address_1);
    assert_eq!(backstop_client_1.treasury(), treasury);
    assert_eq!(backstop_client_1.backstop_token(), asset);
    assert_eq!(backstop_client_1.hook(), Some(hook.clone()));
    assert_eq!(
        pool::Client::new(&e, &deployed_pool_address_1).get_hook(),
        Some(hook.clone())
    );
    assert_eq!(
        pool::Client::new(&e, &deployed_pool_address_2).get_hook(),
        None
    );
    assert_eq!(backstop_client_1.decimals(), 7);
    assert_eq!(
        pool::Client::new(&e, &deployed_pool_address_1).get_min_backstop(),
        min_backstop
    );
    assert_eq!(
        backstop_client_1.name(),
        String::from_str(&e, "Backstop Share")
    );
    let backstop_client_2 = backstop::Client::new(&e, &backstop_address_2);
    assert_eq!(backstop_client_2.pool(), deployed_pool_address_2);
    assert_eq!(backstop_client_2.treasury(), treasury);
    assert_eq!(backstop_client_2.backstop_token(), asset);
    assert_eq!(backstop_client_2.hook(), None);

    assert_eq!(
        pool::Client::new(&e, &deployed_pool_address_2).get_min_backstop(),
        0
    );

    assert_ne!(deployed_pool_address_1, deployed_pool_address_2);
    assert_ne!(backstop_address_1, backstop_address_2);
    assert!(pool_factory_client.is_pool(&deployed_pool_address_1));
    assert!(pool_factory_client.is_pool(&deployed_pool_address_2));
    assert!(!pool_factory_client.is_pool(&backstop_address_1));
    assert!(!pool_factory_client.is_pool(&Address::generate(&e)));
}

#[test]
#[should_panic(expected = "Error(Contract, #1300)")]
fn test_pool_factory_invalid_pool_init_args_backstop_rate() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    e.mock_all_auths_allowing_non_root_auth();
    let (_, pool_factory_client, asset) = setup(&e);

    let bombadil = Address::generate(&e);
    let oracle = Address::generate(&e);
    let backstop_rate: u32 = 1_0000000;
    let max_positions: u32 = 6;
    let min_collateral: i128 = 1_0000000;

    let name1 = String::from_str(&e, "pool1");
    let salt = BytesN::<32>::random(&e);

    pool_factory_client.deploy(
        &bombadil,
        &name1,
        &salt,
        &oracle,
        &backstop_rate,
        &max_positions,
        &min_collateral,
        &0,
        &backstop_init(&e, &asset),
        &None,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1300)")]
fn test_pool_factory_invalid_pool_init_args_max_positions() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    e.mock_all_auths_allowing_non_root_auth();
    let (_, pool_factory_client, asset) = setup(&e);

    let bombadil = Address::generate(&e);
    let oracle = Address::generate(&e);
    let backstop_rate: u32 = 0_1000000;
    let max_positions: u32 = 1;
    let min_collateral: i128 = 1_0000000;

    let name1 = String::from_str(&e, "pool1");
    let salt = BytesN::<32>::random(&e);

    pool_factory_client.deploy(
        &bombadil,
        &name1,
        &salt,
        &oracle,
        &backstop_rate,
        &max_positions,
        &min_collateral,
        &0,
        &backstop_init(&e, &asset),
        &None,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1300)")]
fn test_pool_factory_invalid_pool_init_args_max_positions_large() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    e.mock_all_auths_allowing_non_root_auth();
    let (_, pool_factory_client, asset) = setup(&e);

    let bombadil = Address::generate(&e);
    let oracle = Address::generate(&e);
    let backstop_rate: u32 = 0_1000000;
    let max_positions: u32 = 61;
    let min_collateral: i128 = 1_0000000;

    let name1 = String::from_str(&e, "pool1");
    let salt = BytesN::<32>::random(&e);

    pool_factory_client.deploy(
        &bombadil,
        &name1,
        &salt,
        &oracle,
        &backstop_rate,
        &max_positions,
        &min_collateral,
        &0,
        &backstop_init(&e, &asset),
        &None,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1300)")]
fn test_pool_factory_invalid_pool_init_args_min_collateral() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    e.mock_all_auths_allowing_non_root_auth();
    let (_, pool_factory_client, asset) = setup(&e);

    let bombadil = Address::generate(&e);
    let oracle = Address::generate(&e);
    let backstop_rate: u32 = 0_1000000;
    let max_positions: u32 = 60;
    let min_collateral: i128 = -1;

    let name1 = String::from_str(&e, "pool1");
    let salt = BytesN::<32>::random(&e);

    pool_factory_client.deploy(
        &bombadil,
        &name1,
        &salt,
        &oracle,
        &backstop_rate,
        &max_positions,
        &min_collateral,
        &0,
        &backstop_init(&e, &asset),
        &None,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1300)")]
fn test_pool_factory_invalid_pool_init_args_min_backstop() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    e.mock_all_auths_allowing_non_root_auth();
    let (_, pool_factory_client, asset) = setup(&e);

    let bombadil = Address::generate(&e);
    let oracle = Address::generate(&e);
    let backstop_rate: u32 = 0_1000000;
    let max_positions: u32 = 60;
    let min_collateral: i128 = 0;

    let name1 = String::from_str(&e, "pool1");
    let salt = BytesN::<32>::random(&e);

    pool_factory_client.deploy(
        &bombadil,
        &name1,
        &salt,
        &oracle,
        &backstop_rate,
        &max_positions,
        &min_collateral,
        &-1,
        &backstop_init(&e, &asset),
        &None,
    );
}

#[test]
fn test_pool_factory_frontrun_protection() {
    let e = Env::default();
    e.cost_estimate().budget().reset_unlimited();
    e.mock_all_auths();
    let (_, pool_factory_client, asset) = setup(&e);

    let bombadil = Address::generate(&e);
    let sauron = Address::generate(&e);

    let oracle = Address::generate(&e);
    let backstop_rate: u32 = 0_1000000;
    let max_positions: u32 = 6;
    let min_collateral: i128 = 0;

    let name1 = String::from_str(&e, "pool1");
    let name2 = String::from_str(&e, "pool_front_run");
    let salt = BytesN::<32>::random(&e);

    // verify two different users don't get the same pool address with the same
    // salt parameter
    let deployed_pool_address_sauron = pool_factory_client.deploy(
        &sauron,
        &name2,
        &salt,
        &oracle,
        &backstop_rate,
        &max_positions,
        &min_collateral,
        &0,
        &backstop_init(&e, &asset),
        &None,
    );

    let deployed_pool_address_bombadil = pool_factory_client.deploy(
        &bombadil,
        &name1,
        &salt,
        &oracle,
        &backstop_rate,
        &max_positions,
        &min_collateral,
        &0,
        &backstop_init(&e, &asset),
        &None,
    );

    assert!(deployed_pool_address_sauron != deployed_pool_address_bombadil);
    assert!(pool_factory_client.is_pool(&deployed_pool_address_sauron));
    assert!(pool_factory_client.is_pool(&deployed_pool_address_bombadil));
}
