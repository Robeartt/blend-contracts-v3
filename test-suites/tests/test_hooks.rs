#![cfg(test)]

//! The three reference hooks against a live pool and backstop: entries are gated, exits never are.

use hook_allowlist::{AllowlistHook, AllowlistHookClient};
use hook_gated_liquidator::{GatedLiquidatorHook, GatedLiquidatorHookClient};
use hook_supplier_only::{SupplierOnlyHook, SupplierOnlyHookClient};
use pool::{Request, RequestType};
use soroban_sdk::{testutils::Address as _, vec, Address, Error, Vec};
use test_suites::{
    populate_fixture,
    test_fixture::{TestFixture, TokenIndex, SCALAR_7},
};

/// The hooks' rejection error, propagated through the pool or backstop unchanged
const NOT_ALLOWED: u32 = 1500;

fn assert_rejected<T>(result: Result<T, Result<Error, soroban_sdk::InvokeError>>) {
    match result {
        Err(Ok(error)) => assert_eq!(error, Error::from_contract_error(NOT_ALLOWED)),
        Err(Err(_)) => panic!("invoke error instead of the hook's rejection"),
        Ok(_) => panic!("accepted instead of rejected by the hook"),
    }
}

fn supply_collateral(fixture: &TestFixture, asset: TokenIndex, amount: i128) -> Vec<Request> {
    vec![
        &fixture.env,
        Request {
            request_type: RequestType::SupplyCollateral as u32,
            address: fixture.tokens[asset].address.clone(),
            amount,
        },
    ]
}

#[test]
fn test_allowlist_hook_permissioned_pool() {
    let fixture = TestFixture::create(true);
    let bombadil = fixture.bombadil.clone();
    let frodo = fixture.users[0].clone();
    let hook_id = fixture.env.register(AllowlistHook, (&bombadil,));
    let hook = AllowlistHookClient::new(&fixture.env, &hook_id);
    // the whale must be listed before the fixture supplies, borrows and deposits with it
    hook.set_allowed(&frodo, &true);
    let fixture = populate_fixture(fixture, Some(hook_id.clone()));
    let pool_fixture = &fixture.pools[0];
    assert_eq!(pool_fixture.pool.get_hook(), Some(hook_id.clone()));
    assert_eq!(pool_fixture.backstop.hook(), Some(hook_id.clone()));

    let samwise = Address::generate(&fixture.env);
    fixture.tokens[TokenIndex::XLM].mint(&samwise, &(100_000 * SCALAR_7));
    fixture.tokens[TokenIndex::USDC].mint(&samwise, &(1_000 * SCALAR_7));

    // unlisted: no entry into the pool or the backstop
    assert_rejected(pool_fixture.pool.try_submit(
        &samwise,
        &samwise,
        &samwise,
        &supply_collateral(&fixture, TokenIndex::XLM, 10_000 * SCALAR_7),
    ));
    assert_rejected(pool_fixture.backstop.try_deposit(
        &samwise,
        &pool_fixture.pool.address,
        &(1_000 * SCALAR_7),
    ));
    // a listed spender cannot be used to enter on an unlisted user's behalf either
    assert_rejected(pool_fixture.pool.try_submit(
        &samwise,
        &frodo,
        &samwise,
        &supply_collateral(&fixture, TokenIndex::XLM, 10_000 * SCALAR_7),
    ));

    // listed: enters both
    hook.set_allowed(&samwise, &true);
    let requests = vec![
        &fixture.env,
        Request {
            request_type: RequestType::SupplyCollateral as u32,
            address: fixture.tokens[TokenIndex::XLM].address.clone(),
            amount: 10_000 * SCALAR_7,
        },
        Request {
            request_type: RequestType::Borrow as u32,
            address: fixture.tokens[TokenIndex::XLM].address.clone(),
            amount: 1_000 * SCALAR_7,
        },
    ];
    let positions = pool_fixture
        .pool
        .submit(&samwise, &samwise, &samwise, &requests);
    assert_eq!(positions.collateral.len(), 1);
    assert_eq!(positions.liabilities.len(), 1);
    let shares =
        pool_fixture
            .backstop
            .deposit(&samwise, &pool_fixture.pool.address, &(1_000 * SCALAR_7));
    assert!(shares > 0);

    // delisted: cannot enter, can always leave
    hook.set_allowed(&samwise, &false);
    assert_rejected(pool_fixture.pool.try_submit(
        &samwise,
        &samwise,
        &samwise,
        &supply_collateral(&fixture, TokenIndex::XLM, 1 * SCALAR_7),
    ));
    let requests = vec![
        &fixture.env,
        Request {
            request_type: RequestType::Repay as u32,
            address: fixture.tokens[TokenIndex::XLM].address.clone(),
            amount: 2_000 * SCALAR_7,
        },
        Request {
            request_type: RequestType::WithdrawCollateral as u32,
            address: fixture.tokens[TokenIndex::XLM].address.clone(),
            amount: 100_000 * SCALAR_7,
        },
    ];
    let positions = pool_fixture
        .pool
        .submit(&samwise, &samwise, &samwise, &requests);
    assert_eq!(positions.effective_count(), 0);
    pool_fixture
        .backstop
        .queue_withdrawal(&samwise, &pool_fixture.pool.address, &shares);
    fixture.jump(17 * 24 * 60 * 60 + 1);
    let tokens = pool_fixture
        .backstop
        .withdraw(&samwise, &pool_fixture.pool.address, &shares);
    assert!(tokens > 0);
    assert_eq!(
        pool_fixture
            .backstop
            .user_balance(&pool_fixture.pool.address, &samwise)
            .shares,
        0
    );
}

#[test]
fn test_gated_liquidator_hook_gates_fills_only() {
    let fixture = TestFixture::create(true);
    let bombadil = fixture.bombadil.clone();
    let hook_id = fixture.env.register(GatedLiquidatorHook, (&bombadil,));
    let hook = GatedLiquidatorHookClient::new(&fixture.env, &hook_id);
    let fixture = populate_fixture(fixture, Some(hook_id.clone()));
    let pool_fixture = &fixture.pools[0];

    let samwise = Address::generate(&fixture.env);
    fixture.tokens[TokenIndex::XLM].mint(&samwise, &(100_000 * SCALAR_7));
    fixture.tokens[TokenIndex::USDC].mint(&samwise, &(10_000 * SCALAR_7));

    // let two years of interest accrue and open an interest auction
    for _ in 0..104 {
        fixture.jump(60 * 60 * 24 * 7);
    }
    pool_fixture.pool.new_auction(
        &2u32,
        &pool_fixture.backstop.address,
        &vec![
            &fixture.env,
            fixture.tokens[TokenIndex::USDC].address.clone(),
        ],
        &vec![
            &fixture.env,
            fixture.tokens[TokenIndex::STABLE].address.clone(),
            fixture.tokens[TokenIndex::XLM].address.clone(),
            fixture.tokens[TokenIndex::WETH].address.clone(),
        ],
        &100u32,
    );
    fixture.jump_with_sequence(200 * 5);

    // anyone can still use the pool
    pool_fixture.pool.submit(
        &samwise,
        &samwise,
        &samwise,
        &supply_collateral(&fixture, TokenIndex::XLM, 10_000 * SCALAR_7),
    );

    let fill = vec![
        &fixture.env,
        Request {
            request_type: RequestType::FillInterestAuction as u32,
            address: pool_fixture.backstop.address.clone(),
            amount: 100,
        },
    ];

    // an unlisted filler is refused, a listed one fills
    assert_rejected(
        pool_fixture
            .pool
            .try_submit(&samwise, &samwise, &samwise, &fill),
    );
    hook.set_allowed(&samwise, &true);
    let pre_fill = fixture.tokens[TokenIndex::STABLE].balance(&samwise);
    pool_fixture
        .pool
        .submit(&samwise, &samwise, &samwise, &fill);
    assert!(fixture.tokens[TokenIndex::STABLE].balance(&samwise) > pre_fill);
}

#[test]
fn test_supplier_only_hook_gates_supply_only() {
    let fixture = TestFixture::create(true);
    let bombadil = fixture.bombadil.clone();
    // the whale stands in for the operator's fee vault
    let fee_vault = fixture.users[0].clone();
    let hook_id = fixture
        .env
        .register(SupplierOnlyHook, (&bombadil, &fee_vault));
    let hook = SupplierOnlyHookClient::new(&fixture.env, &hook_id);
    let fixture = populate_fixture(fixture, Some(hook_id.clone()));
    let pool_fixture = &fixture.pools[0];
    assert_eq!(hook.get_supplier(), fee_vault);

    let samwise = Address::generate(&fixture.env);
    fixture.tokens[TokenIndex::XLM].mint(&samwise, &(100_000 * SCALAR_7));
    let supply = vec![
        &fixture.env,
        Request {
            request_type: RequestType::Supply as u32,
            address: fixture.tokens[TokenIndex::XLM].address.clone(),
            amount: 10_000 * SCALAR_7,
        },
    ];

    // only the fee vault supplies; collateral is open to everyone
    assert_rejected(
        pool_fixture
            .pool
            .try_submit(&samwise, &samwise, &samwise, &supply),
    );
    let positions = pool_fixture.pool.submit(
        &samwise,
        &samwise,
        &samwise,
        &supply_collateral(&fixture, TokenIndex::XLM, 10_000 * SCALAR_7),
    );
    assert_eq!(positions.collateral.len(), 1);
    let positions = pool_fixture
        .pool
        .submit(&fee_vault, &fee_vault, &fee_vault, &supply);
    assert_eq!(positions.supply.len(), 1);

    // the backstop is open to everyone
    fixture.tokens[TokenIndex::USDC].mint(&samwise, &(1_000 * SCALAR_7));
    assert!(
        pool_fixture
            .backstop
            .deposit(&samwise, &pool_fixture.pool.address, &(1_000 * SCALAR_7))
            > 0
    );
}
