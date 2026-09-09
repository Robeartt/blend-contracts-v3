#![cfg(test)]

use pool::{PoolClient, Request, RequestType};
use soroban_fixed_point_math::FixedPoint;
use soroban_sdk::{testutils::Address as _, vec, Address, Error, String};
use test_suites::{
    assertions::assert_approx_eq_abs,
    pool::default_reserve_metadata,
    test_fixture::{TestFixture, TokenIndex, DEFAULT_MIN_BACKSTOP, SCALAR_7},
};

#[test]
fn test_backstop_inflation_attack() {
    let mut fixture = TestFixture::create(false);

    let whale = Address::generate(&fixture.env);
    let sauron = Address::generate(&fixture.env);
    let pippen = Address::generate(&fixture.env);

    // create pool with 1 new reserve
    fixture.create_pool(String::from_str(&fixture.env, "Teapot"), 0, 6, 0);

    let xlm_config = default_reserve_metadata();
    fixture.create_pool_reserve(0, TokenIndex::XLM, &xlm_config);
    let pool_address = fixture.pools[0].pool.address.clone();
    let backstop = &fixture.pools[0].backstop;
    let usdc = &fixture.tokens[TokenIndex::USDC];

    // execute inflation attack against pippen
    let starting_balance = 200_000 * SCALAR_7;
    usdc.mint(&whale, &(2 * starting_balance));
    usdc.transfer(&whale, &sauron, &starting_balance);
    usdc.transfer(&whale, &pippen, &starting_balance);

    // 1. Attacker deposits a small amount as the initial depositor
    let sauron_deposit_amount = 100;
    let sauron_shares = backstop.deposit(&sauron, &pool_address, &sauron_deposit_amount);
    assert_eq!(sauron_shares, 100);

    // 2. Attacker tries to send a large amount to the backstop before the victim can perform a deposit
    let inflation_amount = 10_000 * SCALAR_7;
    usdc.transfer(&sauron, &backstop.address, &inflation_amount);

    // the victim's deposit would mint no shares, and is refused rather than diluted
    let deposit_amount = 100;
    let bad_deposit_result = backstop.try_deposit(&pippen, &pool_address, &deposit_amount);
    assert_eq!(
        bad_deposit_result.err(),
        Some(Ok(Error::from_contract_error(1005)))
    );

    // a deposit worth at least a share is priced fairly at the inflated rate
    let pippen_shares = backstop.deposit(&pippen, &pool_address, &(100_000 * SCALAR_7));
    let backstop_data = backstop.pool_data(&pool_address);
    let share_value = backstop_data.tokens / backstop_data.shares;
    assert_approx_eq_abs(
        pippen_shares * share_value,
        100_000 * SCALAR_7,
        share_value + 1,
    );
}

#[test]
fn test_backstop_interest_auction_inflation_attack() {
    let mut fixture = TestFixture::create(false);

    let whale = Address::generate(&fixture.env);
    let sauron = Address::generate(&fixture.env);
    let pippen = Address::generate(&fixture.env);

    // create pool with 1 new reserve
    fixture.create_pool(String::from_str(&fixture.env, "Teapot"), 0, 6, 0);

    let xlm_config = default_reserve_metadata();
    fixture.create_pool_reserve(0, TokenIndex::XLM, &xlm_config);
    let pool_address = fixture.pools[0].pool.address.clone();
    let pool_client = PoolClient::new(&fixture.env, &pool_address);
    pool_client.set_status(&3);
    let backstop = &fixture.pools[0].backstop;
    let usdc = &fixture.tokens[TokenIndex::USDC];

    // send tokens to sauron and pippen
    let starting_balance = 500_000 * SCALAR_7;
    usdc.mint(&whale, &(2 * starting_balance));
    usdc.transfer(&whale, &sauron, &starting_balance);
    usdc.transfer(&whale, &pippen, &starting_balance);
    let xlm_balance = 1_000_000 * SCALAR_7;
    fixture.tokens[TokenIndex::XLM].mint(&sauron, &xlm_balance);

    // 1. Attacker deposits a small amount as the initial depositor
    let sauron_deposit_amount = 100;
    backstop.deposit(&sauron, &pool_address, &sauron_deposit_amount);

    // 2. Attacker tries to force an interest auction to occur to inflate the backstop share value
    let inflation_amount = xlm_balance;
    fixture.tokens[TokenIndex::XLM].transfer(&sauron, &pool_address, &inflation_amount);

    // -> verify that gulp cannot be called until sufficient backstop deposits are present
    let gulp_result = pool_client.try_gulp(&fixture.tokens[TokenIndex::XLM].address);
    assert_eq!(
        gulp_result.err(),
        Some(Ok(Error::from_contract_error(1206)))
    );

    // 3. Attacker enables borrowing on the pool and fills interest auction to cause share inflation
    let remaining_to_threshold = DEFAULT_MIN_BACKSTOP - sauron_deposit_amount;
    backstop.deposit(&sauron, &pool_address, &remaining_to_threshold);
    pool_client.update_status();
    pool_client.gulp(&fixture.tokens[TokenIndex::XLM].address);

    // -> start and fill interest auction
    pool_client.new_auction(
        &2,
        &backstop.address,
        &vec![&fixture.env, usdc.address.clone()],
        &vec![
            &fixture.env,
            fixture.tokens[TokenIndex::XLM].address.clone(),
        ],
        &100,
    );
    fixture.jump_with_sequence(201 * 5);
    let fill_requests = vec![
        &fixture.env,
        Request {
            request_type: RequestType::FillInterestAuction as u32,
            address: backstop.address.clone(),
            amount: 100,
        },
    ];
    pool_client.submit(&sauron, &sauron, &sauron, &fill_requests);

    // -> check new backstop share value
    let backstop_data = backstop.pool_data(&pool_address);
    let shares_to_tokens = backstop_data
        .tokens
        .fixed_div_floor(backstop_data.shares, SCALAR_7)
        .unwrap();

    // 4. Victim uses pool with inflated backstop share value
    let pippen_deposit_amount = SCALAR_7;
    let pippen_shares = backstop.deposit(&pippen, &pool_address, &pippen_deposit_amount);

    // -> verify the victim did not any meaningful amount of funds due to rounding
    let pippen_tokens = pippen_shares
        .fixed_mul_floor(shares_to_tokens, SCALAR_7)
        .unwrap();
    assert_approx_eq_abs(pippen_tokens, pippen_deposit_amount, 10);
}
