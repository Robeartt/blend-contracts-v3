#![cfg(test)]

use soroban_sdk::{testutils::Address as _, Address, Error, String};
use test_suites::{
    create_fixture_with_data,
    test_fixture::{TokenIndex, DEFAULT_MIN_BACKSTOP, DEFAULT_PROTOCOL_RATE, SCALAR_7},
};

/// Test the backstop against the live pool: deposits, the withdrawal queue, the pool status it
/// drives, donations split with the treasury, and share value.
#[test]
fn test_backstop() {
    let fixture = create_fixture_with_data(false);
    let frodo = fixture.users.get(0).unwrap();
    let pool_fixture = &fixture.pools[0];
    let pool = &pool_fixture.pool;
    let backstop = &pool_fixture.backstop;
    let usdc = &fixture.tokens[TokenIndex::USDC];
    let sam = Address::generate(&fixture.env);

    // Verify the factory bound the pair
    assert_eq!(backstop.pool(), pool.address);
    assert_eq!(pool.get_backstop(), backstop.address);
    assert_eq!(backstop.backstop_token(), usdc.address);
    assert_eq!(backstop.treasury(), fixture.treasury.address);
    assert_eq!(pool.get_min_backstop(), DEFAULT_MIN_BACKSTOP);
    assert_eq!(backstop.hook(), None);
    assert_eq!(backstop.decimals(), 7);
    assert_eq!(
        backstop.name(),
        String::from_str(&fixture.env, "Backstop Share")
    );

    // Every call is for this backstop's pool
    let other_pool = Address::generate(&fixture.env);
    let wrong_pool = backstop.try_deposit(&sam, &other_pool, &SCALAR_7);
    assert_eq!(wrong_pool.err(), Some(Ok(Error::from_contract_error(1004))));
    let wrong_pool = backstop.try_pool_data(&other_pool);
    assert_eq!(wrong_pool.err(), Some(Ok(Error::from_contract_error(1004))));

    // Frodo deposited 50k in setup, the pool is active
    let backstop_data = backstop.pool_data(&pool.address);
    assert_eq!(backstop_data.tokens, 50_000 * SCALAR_7);
    assert_eq!(backstop_data.shares, 50_000 * SCALAR_7);
    assert_eq!(backstop_data.q4w_pct, 0);
    assert_eq!(backstop.total_supply(), 50_000 * SCALAR_7);
    assert_eq!(fixture.read_pool_config(0).status, 1);

    // Sam deposits 12.5k, making up 20% of the backstop
    let sam_deposit = 12_500 * SCALAR_7;
    usdc.mint(&sam, &sam_deposit);
    let sam_shares = backstop.deposit(&sam, &pool.address, &sam_deposit);
    assert_eq!(sam_shares, sam_deposit);
    let sam_balance = backstop.user_balance(&pool.address, &sam);
    assert_eq!(sam_balance.shares, sam_shares);
    assert_eq!(sam_balance.q4w.len(), 0);
    assert_eq!(backstop.balance(&sam), sam_shares);
    assert_eq!(usdc.balance(&sam), 0);
    assert_eq!(usdc.balance(&backstop.address), 62_500 * SCALAR_7);

    // Frodo donates 5k as the pool would (auths are mocked, so the pool's is recorded): the
    // treasury takes 10% and the rest appreciates every share by 7.2%
    let donation = 5_000 * SCALAR_7;
    let frodo_usdc = usdc.balance(frodo);
    backstop.donate(frodo, &pool.address, &donation);
    assert!(fixture
        .env
        .auths()
        .iter()
        .any(|(signer, _)| signer == &pool.address));
    let to_treasury = donation * i128::from(DEFAULT_PROTOCOL_RATE) / SCALAR_7;
    assert_eq!(to_treasury, 500 * SCALAR_7);
    assert_eq!(usdc.balance(&fixture.treasury.address), to_treasury);
    assert_eq!(usdc.balance(frodo), frodo_usdc - donation);
    assert_eq!(usdc.balance(&backstop.address), 67_000 * SCALAR_7);
    let backstop_data = backstop.pool_data(&pool.address);
    assert_eq!(backstop_data.tokens, 67_000 * SCALAR_7);
    assert_eq!(backstop_data.shares, 62_500 * SCALAR_7);

    // Sam queues everything for withdrawal: 20% queued keeps the pool active
    let q4w = backstop.queue_withdrawal(&sam, &pool.address, &sam_shares);
    assert_eq!(q4w.amount, sam_shares);
    assert_eq!(
        q4w.exp,
        fixture.env.ledger().timestamp() + 17 * 24 * 60 * 60
    );
    let sam_balance = backstop.user_balance(&pool.address, &sam);
    assert_eq!(sam_balance.shares, 0);
    assert_eq!(sam_balance.q4w.len(), 1);
    assert_eq!(sam_balance.q4w.get_unchecked(0), q4w);
    assert_eq!(backstop.balance(&sam), 0);
    assert_eq!(backstop.balance(&backstop.address), sam_shares);
    assert_eq!(backstop.pool_data(&pool.address).q4w_pct, 0_2000000);
    assert_eq!(pool.update_status(), 1);

    // Frodo queues 40% of his: 52% queued puts the pool on ice
    let frodo_q4w = 20_000 * SCALAR_7;
    backstop.queue_withdrawal(frodo, &pool.address, &frodo_q4w);
    assert_eq!(backstop.pool_data(&pool.address).q4w_pct, 0_5200000);
    assert_eq!(pool.update_status(), 3);

    // Queued shares cannot be withdrawn early
    fixture.jump(17 * 24 * 60 * 60 - 1);
    let early = backstop.try_withdraw(&sam, &pool.address, &sam_shares);
    assert_eq!(early.err(), Some(Ok(Error::from_contract_error(1001))));

    // Frodo dequeues, the pool is healthy again
    backstop.dequeue_withdrawal(frodo, &pool.address, &frodo_q4w);
    let frodo_balance = backstop.user_balance(&pool.address, frodo);
    assert_eq!(frodo_balance.shares, 50_000 * SCALAR_7);
    assert_eq!(frodo_balance.q4w.len(), 0);
    assert_eq!(backstop.pool_data(&pool.address).q4w_pct, 0_2000000);
    assert_eq!(pool.update_status(), 1);

    // The lock passes and Sam withdraws for his share of the donation
    fixture.jump(1);
    let tokens = backstop.withdraw(&sam, &pool.address, &sam_shares);
    assert_eq!(tokens, 13_399_9999999);
    assert_eq!(usdc.balance(&sam), tokens);
    assert_eq!(backstop.balance(&backstop.address), 0);
    assert_eq!(backstop.total_supply(), 50_000 * SCALAR_7);
    let sam_balance = backstop.user_balance(&pool.address, &sam);
    assert_eq!(sam_balance.shares, 0);
    assert_eq!(sam_balance.q4w.len(), 0);
    let backstop_data = backstop.pool_data(&pool.address);
    assert_eq!(backstop_data.tokens, 67_000 * SCALAR_7 - tokens);
    assert_eq!(backstop_data.shares, 50_000 * SCALAR_7);
    assert_eq!(backstop_data.q4w_pct, 0);

    // Shares can be transferred and the receiver can queue them
    backstop.transfer(frodo, &sam, &(1_000 * SCALAR_7));
    assert_eq!(
        backstop.user_balance(&pool.address, &sam).shares,
        1_000 * SCALAR_7
    );
    backstop.queue_withdrawal(&sam, &pool.address, &(1_000 * SCALAR_7));
    assert_eq!(backstop.user_balance(&pool.address, &sam).q4w.len(), 1);

    // draw requires the pool's authorization (auths are mocked, so it is recorded)
    backstop.draw(&pool.address, &SCALAR_7, &sam);
    assert_eq!(fixture.env.auths()[0].0, pool.address);
    assert_eq!(usdc.balance(&sam), tokens + SCALAR_7);
}
