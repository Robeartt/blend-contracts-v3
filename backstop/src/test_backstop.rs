#![cfg(test)]
#![allow(clippy::zero_prefixed_literal, clippy::inconsistent_digit_grouping)]

use crate::testutils::{fixture, hooked_fixture, jump, HookCall, ONE_DAY};
use crate::Q4W;
use mock_pool::Positions;
use soroban_sdk::{
    map,
    testutils::{
        Address as _, AuthorizedFunction, AuthorizedInvocation, ContractEvents, Events, MockAuth,
        MockAuthInvoke,
    },
    vec, xdr, Address, Env, IntoVal, MuxedAddress, String, Symbol, TryFromVal, Val, Vec,
};

const SCALAR_7: i128 = 1_0000000;
const LOCK: u64 = 17 * ONE_DAY;

/// Assert the last contract event in `events`, captured right after the invocation under test
fn assert_last_event(
    e: &Env,
    events: &ContractEvents,
    contract: &Address,
    topics: Vec<Val>,
    data: Val,
) {
    let last = events.events().last().expect("no events").clone();
    let contract_id = match xdr::ScVal::try_from_val(e, &contract.to_val()).unwrap() {
        xdr::ScVal::Address(xdr::ScAddress::Contract(id)) => id,
        _ => panic!("not a contract address"),
    };
    let expected_topics = match xdr::ScVal::try_from_val(e, &topics.to_val()).unwrap() {
        xdr::ScVal::Vec(Some(topics)) => topics.0,
        _ => panic!("topics are not a vec"),
    };
    let xdr::ContractEventBody::V0(body) = last.body;
    assert_eq!(last.contract_id, Some(contract_id));
    assert_eq!(body.topics, expected_topics);
    assert_eq!(body.data, xdr::ScVal::try_from_val(e, &data).unwrap());
}

/// Event data in the `vec` format
fn data_vec(e: &Env, items: &[Val]) -> Val {
    Vec::<Val>::from_slice(e, items).into_val(e)
}

/********** Constructor / Getters **********/

#[test]
fn test_constructor() {
    let f = fixture(3);
    assert_eq!(f.backstop_client.pool(), f.pool);
    assert_eq!(f.backstop_client.hook(), None);
    assert_eq!(f.backstop_client.treasury(), f.treasury);
    assert_eq!(f.backstop_client.backstop_token(), f.token);
    assert_eq!(f.backstop_client.decimals(), 7 + 3);
    assert_eq!(
        f.backstop_client.name(),
        String::from_str(&f.e, "Pool Backstop Share")
    );
    assert_eq!(f.backstop_client.symbol(), String::from_str(&f.e, "PBS"));
    assert_eq!(f.backstop_client.total_supply(), 0);

    let pool_data = f.backstop_client.pool_data(&f.pool);
    assert_eq!(pool_data.tokens, 0);
    assert_eq!(pool_data.shares, 0);
    assert_eq!(pool_data.q4w_pct, 0);

    let user_balance = f
        .backstop_client
        .user_balance(&f.pool, &Address::generate(&f.e));
    assert_eq!(user_balance.shares, 0);
    assert_eq!(user_balance.q4w.len(), 0);
}

#[test]
fn test_constructor_with_hook() {
    let (f, hook) = hooked_fixture();
    assert_eq!(f.backstop_client.hook(), Some(hook.address.clone()));
    assert_eq!(f.backstop_client.decimals(), 7);
}

/********** Deposit **********/

#[test]
fn test_deposit() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);

    let shares = f.backstop_client.deposit(&user, &f.pool, &(40 * SCALAR_7));
    let events = f.e.events().all();
    assert_eq!(
        f.e.auths()[0],
        (
            user.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    f.backstop.clone(),
                    Symbol::new(&f.e, "deposit"),
                    vec![
                        &f.e,
                        user.to_val(),
                        f.pool.to_val(),
                        (40 * SCALAR_7).into_val(&f.e)
                    ]
                )),
                sub_invocations: std::vec![AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        f.token.clone(),
                        Symbol::new(&f.e, "transfer"),
                        vec![
                            &f.e,
                            user.to_val(),
                            f.backstop.to_val(),
                            (40 * SCALAR_7).into_val(&f.e)
                        ]
                    )),
                    sub_invocations: std::vec![]
                }]
            }
        )
    );
    assert_eq!(shares, 40 * SCALAR_7);
    assert_eq!(f.backstop_client.balance(&user), 40 * SCALAR_7);
    assert_eq!(f.backstop_client.total_supply(), 40 * SCALAR_7);
    assert_eq!(f.token_client.balance(&user), 60 * SCALAR_7);
    assert_eq!(f.token_client.balance(&f.backstop), 40 * SCALAR_7);

    let user_balance = f.backstop_client.user_balance(&f.pool, &user);
    assert_eq!(user_balance.shares, 40 * SCALAR_7);
    assert_eq!(user_balance.q4w.len(), 0);
    let pool_data = f.backstop_client.pool_data(&f.pool);
    assert_eq!(pool_data.tokens, 40 * SCALAR_7);
    assert_eq!(pool_data.shares, 40 * SCALAR_7);
    assert_eq!(pool_data.q4w_pct, 0);

    assert_last_event(
        &f.e,
        &events,
        &f.backstop,
        (Symbol::new(&f.e, "deposit"), f.pool.clone(), user.clone()).into_val(&f.e),
        data_vec(
            &f.e,
            &[
                (40 * SCALAR_7).into_val(&f.e),
                (40 * SCALAR_7).into_val(&f.e),
            ],
        ),
    );
}

#[test]
fn test_deposit_share_price() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    let other = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 40 * SCALAR_7);

    // double the tokens per share with a plain token transfer
    f.token_client
        .transfer(&other, &f.backstop, &(40 * SCALAR_7));
    let pool_data = f.backstop_client.pool_data(&f.pool);
    assert_eq!(pool_data.tokens, 80 * SCALAR_7);
    assert_eq!(pool_data.shares, 40 * SCALAR_7);

    // shares = 20 * (40 + 1e-7) / (80 + 1e-7), floored
    let shares = f.deposit(&other, 20 * SCALAR_7);
    assert_eq!(shares, 10 * SCALAR_7);
    assert_eq!(f.backstop_client.total_supply(), 50 * SCALAR_7);

    // the shares are worth what was paid, less the virtual offset's rounding
    f.backstop_client
        .queue_withdrawal(&other, &f.pool, &(10 * SCALAR_7));
    jump(&f.e, LOCK);
    let tokens = f
        .backstop_client
        .withdraw(&other, &f.pool, &(10 * SCALAR_7));
    assert_eq!(
        tokens,
        (10 * SCALAR_7 * (100 * SCALAR_7 + 1)) / (50 * SCALAR_7 + 1)
    );
    assert_eq!(tokens, 19_9999999);
}

#[test]
#[should_panic(expected = "Error(Contract, #1000)")]
fn test_deposit_from_pool() {
    let f = fixture(0);
    f.token_client.mint(&f.pool, &(100 * SCALAR_7));
    f.backstop_client
        .deposit(&f.pool, &f.pool, &(40 * SCALAR_7));
}

#[test]
#[should_panic(expected = "Error(Contract, #1005)")]
fn test_deposit_zero_shares() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.backstop_client.deposit(&user, &f.pool, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #1004)")]
fn test_deposit_wrong_pool() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.backstop_client
        .deposit(&user, &Address::generate(&f.e), &(40 * SCALAR_7));
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_deposit_negative() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.backstop_client.deposit(&user, &f.pool, &-1);
}

#[test]
fn test_inflation_attack() {
    // 3 extra decimals on shares: 1000 virtual shares against 1 virtual token
    let f = fixture(3);
    let sauron = f.user_with(20_000 * SCALAR_7);
    let pippen = f.user_with(20_000 * SCALAR_7);

    // 1. Attacker deposits a small amount as the initial depositor
    let sauron_shares = f.deposit(&sauron, 100);
    assert_eq!(sauron_shares, 100 * 1000);

    // 2. Attacker sends a large amount to the backstop before the victim can perform a deposit
    f.token_client
        .transfer(&sauron, &f.backstop, &(10_000 * SCALAR_7));

    // the victim's deposit would mint no shares and is refused instead of being diluted
    let result = f.backstop_client.try_deposit(&pippen, &f.pool, &100);
    assert_eq!(
        result.err(),
        Some(Ok(soroban_sdk::Error::from_contract_error(1005)))
    );

    // a deposit large enough to earn shares is priced at the inflated rate, nothing is lost
    let pippen_shares = f.deposit(&pippen, 11_000 * SCALAR_7);
    assert_eq!(
        pippen_shares,
        (11_000 * SCALAR_7 * (100_000 + 1000)) / (10_000 * SCALAR_7 + 100 + 1)
    );
    assert_eq!(pippen_shares, 111_099);

    // the victim can exit with (almost) everything they put in
    f.backstop_client
        .queue_withdrawal(&pippen, &f.pool, &pippen_shares);
    jump(&f.e, LOCK);
    let tokens = f.backstop_client.withdraw(&pippen, &f.pool, &pippen_shares);
    assert_eq!(tokens, 10999_9528574);
    assert!(tokens > 11_000 * SCALAR_7 - SCALAR_7 / 10);
}

/********** Queue Withdrawal **********/

#[test]
fn test_queue_withdrawal() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    let now = f.e.ledger().timestamp();

    let entry = f
        .backstop_client
        .queue_withdrawal(&user, &f.pool, &(40 * SCALAR_7));
    let events = f.e.events().all();
    assert_eq!(
        f.e.auths()[0],
        (
            user.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    f.backstop.clone(),
                    Symbol::new(&f.e, "queue_withdrawal"),
                    vec![
                        &f.e,
                        user.to_val(),
                        f.pool.to_val(),
                        (40 * SCALAR_7).into_val(&f.e)
                    ]
                )),
                sub_invocations: std::vec![]
            }
        )
    );
    assert_eq!(
        entry,
        Q4W {
            amount: 40 * SCALAR_7,
            exp: now + 17 * 24 * 60 * 60,
        }
    );

    // shares are escrowed by the backstop, the supply is unchanged
    assert_eq!(f.backstop_client.balance(&user), 60 * SCALAR_7);
    assert_eq!(f.backstop_client.balance(&f.backstop), 40 * SCALAR_7);
    assert_eq!(f.backstop_client.total_supply(), 100 * SCALAR_7);
    let user_balance = f.backstop_client.user_balance(&f.pool, &user);
    assert_eq!(user_balance.shares, 60 * SCALAR_7);
    assert_eq!(user_balance.q4w, vec![&f.e, entry.clone()]);
    let pool_data = f.backstop_client.pool_data(&f.pool);
    assert_eq!(pool_data.tokens, 100 * SCALAR_7);
    assert_eq!(pool_data.shares, 100 * SCALAR_7);
    assert_eq!(pool_data.q4w_pct, 0_4000000);

    assert_last_event(
        &f.e,
        &events,
        &f.backstop,
        (
            Symbol::new(&f.e, "queue_withdrawal"),
            f.pool.clone(),
            user.clone(),
        )
            .into_val(&f.e),
        data_vec(
            &f.e,
            &[(40 * SCALAR_7).into_val(&f.e), entry.exp.into_val(&f.e)],
        ),
    );

    // queue the rest a day later, entries are ordered oldest first
    jump(&f.e, ONE_DAY);
    let entry_2 = f
        .backstop_client
        .queue_withdrawal(&user, &f.pool, &(60 * SCALAR_7));
    assert_eq!(entry_2.exp, now + ONE_DAY + LOCK);
    let user_balance = f.backstop_client.user_balance(&f.pool, &user);
    assert_eq!(user_balance.shares, 0);
    assert_eq!(user_balance.q4w, vec![&f.e, entry, entry_2]);
    assert_eq!(f.backstop_client.balance(&f.backstop), 100 * SCALAR_7);
    assert_eq!(f.backstop_client.pool_data(&f.pool).q4w_pct, SCALAR_7);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_queue_withdrawal_over_balance() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(100 * SCALAR_7 + 1));
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_queue_withdrawal_over_balance_after_queue() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(60 * SCALAR_7));
    // queued shares are no longer part of the balance
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(40 * SCALAR_7 + 1));
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_queue_withdrawal_negative() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client.queue_withdrawal(&user, &f.pool, &-1);
}

#[test]
#[should_panic(expected = "Error(Contract, #1007)")]
fn test_queue_withdrawal_max_entries() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    for _ in 0..20 {
        f.backstop_client
            .queue_withdrawal(&user, &f.pool, &SCALAR_7);
    }
    assert_eq!(f.backstop_client.user_balance(&f.pool, &user).q4w.len(), 20);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &SCALAR_7);
}

#[test]
#[should_panic(expected = "Error(Contract, #1004)")]
fn test_queue_withdrawal_wrong_pool() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &Address::generate(&f.e), &SCALAR_7);
}

/********** Dequeue Withdrawal **********/

#[test]
fn test_dequeue_withdrawal() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    let entry_1 = f
        .backstop_client
        .queue_withdrawal(&user, &f.pool, &(50 * SCALAR_7));
    jump(&f.e, ONE_DAY);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(30 * SCALAR_7));

    // newest entries are dequeued first, the older one is partially consumed
    f.backstop_client
        .dequeue_withdrawal(&user, &f.pool, &(40 * SCALAR_7));
    let events = f.e.events().all();
    let user_balance = f.backstop_client.user_balance(&f.pool, &user);
    assert_eq!(user_balance.shares, 60 * SCALAR_7);
    assert_eq!(
        user_balance.q4w,
        vec![
            &f.e,
            Q4W {
                amount: 40 * SCALAR_7,
                exp: entry_1.exp,
            }
        ]
    );
    assert_eq!(f.backstop_client.balance(&user), 60 * SCALAR_7);
    assert_eq!(f.backstop_client.balance(&f.backstop), 40 * SCALAR_7);
    assert_eq!(f.backstop_client.pool_data(&f.pool).q4w_pct, 0_4000000);
    assert_last_event(
        &f.e,
        &events,
        &f.backstop,
        (
            Symbol::new(&f.e, "dequeue_withdrawal"),
            f.pool.clone(),
            user.clone(),
        )
            .into_val(&f.e),
        (40 * SCALAR_7).into_val(&f.e),
    );

    // dequeue the rest, the queue entry is removed
    f.backstop_client
        .dequeue_withdrawal(&user, &f.pool, &(40 * SCALAR_7));
    let user_balance = f.backstop_client.user_balance(&f.pool, &user);
    assert_eq!(user_balance.shares, 100 * SCALAR_7);
    assert_eq!(user_balance.q4w.len(), 0);
    assert_eq!(f.backstop_client.balance(&f.backstop), 0);
    assert_eq!(f.backstop_client.pool_data(&f.pool).q4w_pct, 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #10)")]
fn test_dequeue_withdrawal_over_queued() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(50 * SCALAR_7));
    f.backstop_client
        .dequeue_withdrawal(&user, &f.pool, &(50 * SCALAR_7 + 1));
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_dequeue_withdrawal_negative() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(50 * SCALAR_7));
    f.backstop_client.dequeue_withdrawal(&user, &f.pool, &-1);
}

#[test]
#[should_panic(expected = "Error(Contract, #1004)")]
fn test_dequeue_withdrawal_wrong_pool() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(50 * SCALAR_7));
    f.backstop_client
        .dequeue_withdrawal(&user, &Address::generate(&f.e), &SCALAR_7);
}

/********** Withdraw **********/

#[test]
fn test_withdraw() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(40 * SCALAR_7));

    // still locked one second before the expiration
    jump(&f.e, LOCK - 1);
    let result = f
        .backstop_client
        .try_withdraw(&user, &f.pool, &(40 * SCALAR_7));
    assert_eq!(
        result.err(),
        Some(Ok(soroban_sdk::Error::from_contract_error(1001)))
    );

    jump(&f.e, 1);
    let tokens = f.backstop_client.withdraw(&user, &f.pool, &(40 * SCALAR_7));
    let events = f.e.events().all();
    assert_eq!(
        f.e.auths()[0],
        (
            user.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    f.backstop.clone(),
                    Symbol::new(&f.e, "withdraw"),
                    vec![
                        &f.e,
                        user.to_val(),
                        f.pool.to_val(),
                        (40 * SCALAR_7).into_val(&f.e)
                    ]
                )),
                sub_invocations: std::vec![]
            }
        )
    );
    assert_eq!(tokens, 40 * SCALAR_7);
    assert_eq!(f.token_client.balance(&user), 40 * SCALAR_7);
    assert_eq!(f.token_client.balance(&f.backstop), 60 * SCALAR_7);
    assert_eq!(f.backstop_client.balance(&f.backstop), 0);
    assert_eq!(f.backstop_client.balance(&user), 60 * SCALAR_7);
    assert_eq!(f.backstop_client.total_supply(), 60 * SCALAR_7);
    let user_balance = f.backstop_client.user_balance(&f.pool, &user);
    assert_eq!(user_balance.shares, 60 * SCALAR_7);
    assert_eq!(user_balance.q4w.len(), 0);
    let pool_data = f.backstop_client.pool_data(&f.pool);
    assert_eq!(pool_data.tokens, 60 * SCALAR_7);
    assert_eq!(pool_data.shares, 60 * SCALAR_7);
    assert_eq!(pool_data.q4w_pct, 0);

    assert_last_event(
        &f.e,
        &events,
        &f.backstop,
        (Symbol::new(&f.e, "withdraw"), f.pool.clone(), user.clone()).into_val(&f.e),
        data_vec(
            &f.e,
            &[
                (40 * SCALAR_7).into_val(&f.e),
                (40 * SCALAR_7).into_val(&f.e),
            ],
        ),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1001)")]
fn test_withdraw_not_expired() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(40 * SCALAR_7));
    jump(&f.e, LOCK - 1);
    f.backstop_client.withdraw(&user, &f.pool, &(40 * SCALAR_7));
}

#[test]
fn test_withdraw_multiple_entries() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(30 * SCALAR_7));
    jump(&f.e, ONE_DAY);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(20 * SCALAR_7));
    jump(&f.e, ONE_DAY);
    let entry_3 = f
        .backstop_client
        .queue_withdrawal(&user, &f.pool, &(10 * SCALAR_7));

    // only the first two entries are unlocked
    jump(&f.e, LOCK - ONE_DAY);

    // oldest entries are consumed first, a partially consumed entry remains
    let tokens = f.backstop_client.withdraw(&user, &f.pool, &(35 * SCALAR_7));
    assert_eq!(tokens, 35 * SCALAR_7);
    let q4w = f.backstop_client.user_balance(&f.pool, &user).q4w;
    assert_eq!(q4w.len(), 2);
    assert_eq!(q4w.get_unchecked(0).amount, 15 * SCALAR_7);
    assert_eq!(q4w.get_unchecked(1), entry_3);
    assert_eq!(f.backstop_client.balance(&f.backstop), 25 * SCALAR_7);

    // consuming into the locked entry fails
    let result = f
        .backstop_client
        .try_withdraw(&user, &f.pool, &(16 * SCALAR_7));
    assert_eq!(
        result.err(),
        Some(Ok(soroban_sdk::Error::from_contract_error(1001)))
    );

    // more than queued fails once everything is unlocked
    jump(&f.e, ONE_DAY);
    let result = f
        .backstop_client
        .try_withdraw(&user, &f.pool, &(25 * SCALAR_7 + 1));
    assert_eq!(
        result.err(),
        Some(Ok(soroban_sdk::Error::from_contract_error(10)))
    );
    f.backstop_client.withdraw(&user, &f.pool, &(25 * SCALAR_7));
    let user_balance = f.backstop_client.user_balance(&f.pool, &user);
    assert_eq!(user_balance.shares, 40 * SCALAR_7);
    assert_eq!(user_balance.q4w.len(), 0);
    assert_eq!(f.token_client.balance(&user), 60 * SCALAR_7);
    assert_eq!(f.backstop_client.balance(&f.backstop), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #1011)")]
fn test_withdraw_bad_debt_exists() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(40 * SCALAR_7));
    jump(&f.e, LOCK);

    f.pool_client.set_positions(
        &f.backstop,
        &Positions {
            liabilities: map![&f.e, (0, 123)],
            collateral: map![&f.e],
            supply: map![&f.e],
        },
    );
    f.backstop_client.withdraw(&user, &f.pool, &(40 * SCALAR_7));
}

#[test]
fn test_withdraw_bad_debt_only_checks_liabilities() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(40 * SCALAR_7));
    jump(&f.e, LOCK);

    // collateral or supply positions of the backstop do not block withdrawals
    f.pool_client.set_positions(
        &f.backstop,
        &Positions {
            liabilities: map![&f.e],
            collateral: map![&f.e, (0, 123)],
            supply: map![&f.e, (1, 456)],
        },
    );
    let tokens = f.backstop_client.withdraw(&user, &f.pool, &(40 * SCALAR_7));
    assert_eq!(tokens, 40 * SCALAR_7);
}

#[test]
#[should_panic(expected = "Error(Contract, #1006)")]
fn test_withdraw_zero_tokens() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(40 * SCALAR_7));
    jump(&f.e, LOCK);

    // the backstop is drained
    f.backstop_client.draw(&f.pool, &(100 * SCALAR_7), &f.pool);
    f.backstop_client.withdraw(&user, &f.pool, &1);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_withdraw_negative() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(40 * SCALAR_7));
    jump(&f.e, LOCK);
    f.backstop_client.withdraw(&user, &f.pool, &-1);
}

#[test]
#[should_panic(expected = "Error(Contract, #1004)")]
fn test_withdraw_wrong_pool() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(40 * SCALAR_7));
    jump(&f.e, LOCK);
    f.backstop_client
        .withdraw(&user, &Address::generate(&f.e), &(40 * SCALAR_7));
}

/********** Pool Data / User Balance **********/

#[test]
fn test_pool_data_and_user_balance() {
    let f = fixture(0);
    let user = f.user_with(1_000 * SCALAR_7);
    let other = f.user_with(1_000 * SCALAR_7);
    f.deposit(&user, 300 * SCALAR_7);
    f.deposit(&other, 100 * SCALAR_7);
    let entry = f
        .backstop_client
        .queue_withdrawal(&user, &f.pool, &(100 * SCALAR_7));
    // a plain transfer raises the tokens without touching the shares
    f.token_client
        .transfer(&user, &f.backstop, &(50 * SCALAR_7));

    let pool_data = f.backstop_client.pool_data(&f.pool);
    assert_eq!(pool_data.tokens, 450 * SCALAR_7);
    assert_eq!(pool_data.shares, 400 * SCALAR_7);
    assert_eq!(pool_data.q4w_pct, 0_2500000);

    let user_balance = f.backstop_client.user_balance(&f.pool, &user);
    assert_eq!(user_balance.shares, 200 * SCALAR_7);
    assert_eq!(user_balance.q4w, vec![&f.e, entry]);
    let other_balance = f.backstop_client.user_balance(&f.pool, &other);
    assert_eq!(other_balance.shares, 100 * SCALAR_7);
    assert_eq!(other_balance.q4w.len(), 0);

    // q4w_pct rounds up: 100 / 300 shares
    f.backstop_client
        .queue_withdrawal(&other, &f.pool, &(100 * SCALAR_7));
    jump(&f.e, LOCK);
    f.backstop_client
        .withdraw(&other, &f.pool, &(100 * SCALAR_7));
    let pool_data = f.backstop_client.pool_data(&f.pool);
    assert_eq!(pool_data.shares, 300 * SCALAR_7);
    assert_eq!(pool_data.q4w_pct, 0_3333334);
}

#[test]
#[should_panic(expected = "Error(Contract, #1004)")]
fn test_pool_data_wrong_pool() {
    let f = fixture(0);
    f.backstop_client.pool_data(&Address::generate(&f.e));
}

#[test]
#[should_panic(expected = "Error(Contract, #1004)")]
fn test_user_balance_wrong_pool() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .user_balance(&Address::generate(&f.e), &user);
}

/********** Draw **********/

#[test]
fn test_draw() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    let to = Address::generate(&f.e);
    f.deposit(&user, 100 * SCALAR_7);

    f.backstop_client.draw(&f.pool, &(30 * SCALAR_7), &to);
    let events = f.e.events().all();
    assert_eq!(
        f.e.auths()[0],
        (
            f.pool.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    f.backstop.clone(),
                    Symbol::new(&f.e, "draw"),
                    vec![
                        &f.e,
                        f.pool.to_val(),
                        (30 * SCALAR_7).into_val(&f.e),
                        to.to_val()
                    ]
                )),
                sub_invocations: std::vec![]
            }
        )
    );
    assert_eq!(f.token_client.balance(&to), 30 * SCALAR_7);
    assert_eq!(f.token_client.balance(&f.backstop), 70 * SCALAR_7);
    let pool_data = f.backstop_client.pool_data(&f.pool);
    assert_eq!(pool_data.tokens, 70 * SCALAR_7);
    assert_eq!(pool_data.shares, 100 * SCALAR_7);
    assert_last_event(
        &f.e,
        &events,
        &f.backstop,
        (Symbol::new(&f.e, "draw"), f.pool.clone()).into_val(&f.e),
        data_vec(&f.e, &[to.to_val(), (30 * SCALAR_7).into_val(&f.e)]),
    );

    // shares lost value
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(100 * SCALAR_7));
    jump(&f.e, LOCK);
    let tokens = f
        .backstop_client
        .withdraw(&user, &f.pool, &(100 * SCALAR_7));
    assert_eq!(
        tokens,
        (100 * SCALAR_7 * (70 * SCALAR_7 + 1)) / (100 * SCALAR_7 + 1)
    );
}

#[test]
fn test_draw_everything() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client.draw(&f.pool, &(100 * SCALAR_7), &f.pool);
    assert_eq!(f.token_client.balance(&f.pool), 100 * SCALAR_7);
    assert_eq!(f.backstop_client.pool_data(&f.pool).tokens, 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #1003)")]
fn test_draw_over_assets() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .draw(&f.pool, &(100 * SCALAR_7 + 1), &user);
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_draw_negative() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client.draw(&f.pool, &-1, &user);
}

#[test]
#[should_panic(expected = "Error(Contract, #1004)")]
fn test_draw_wrong_pool() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .draw(&Address::generate(&f.e), &SCALAR_7, &user);
}

/********** Donate **********/

#[test]
fn test_donate_rate_zero() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    let donor = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    assert_eq!(f.treasury_client.get_rate(), 0);

    f.backstop_client.donate(&donor, &f.pool, &(50 * SCALAR_7));
    let events = f.e.events().all();
    let authorizers: std::vec::Vec<Address> = f.e.auths().iter().map(|a| a.0.clone()).collect();
    assert!(authorizers.contains(&donor));
    assert!(authorizers.contains(&f.pool));

    assert_eq!(f.token_client.balance(&donor), 50 * SCALAR_7);
    assert_eq!(f.token_client.balance(&f.treasury), 0);
    assert_eq!(f.token_client.balance(&f.backstop), 150 * SCALAR_7);
    let pool_data = f.backstop_client.pool_data(&f.pool);
    assert_eq!(pool_data.tokens, 150 * SCALAR_7);
    assert_eq!(pool_data.shares, 100 * SCALAR_7);
    // the donor received no shares
    assert_eq!(f.backstop_client.balance(&donor), 0);
    assert_last_event(
        &f.e,
        &events,
        &f.backstop,
        (Symbol::new(&f.e, "donate"), f.pool.clone(), donor.clone()).into_val(&f.e),
        (50 * SCALAR_7).into_val(&f.e),
    );
}

#[test]
fn test_donate_with_rate() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    let donor = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.treasury_client.set_rate(&0_1000000);

    f.backstop_client.donate(&donor, &f.pool, &(50 * SCALAR_7));
    let events = f.e.events().all();
    assert_eq!(f.token_client.balance(&donor), 50 * SCALAR_7);
    assert_eq!(f.token_client.balance(&f.treasury), 5 * SCALAR_7);
    assert_eq!(f.token_client.balance(&f.backstop), 145 * SCALAR_7);
    assert_eq!(f.backstop_client.pool_data(&f.pool).tokens, 145 * SCALAR_7);
    // the event carries the backstop's share
    assert_last_event(
        &f.e,
        &events,
        &f.backstop,
        (Symbol::new(&f.e, "donate"), f.pool.clone(), donor.clone()).into_val(&f.e),
        (45 * SCALAR_7).into_val(&f.e),
    );

    // the treasury's share is floored, the backstop gets the remainder
    f.backstop_client.donate(&donor, &f.pool, &33);
    assert_eq!(f.token_client.balance(&donor), 50 * SCALAR_7 - 33);
    assert_eq!(f.token_client.balance(&f.treasury), 5 * SCALAR_7 + 3);
    assert_eq!(f.token_client.balance(&f.backstop), 145 * SCALAR_7 + 30);
}

#[test]
fn test_donate_rate_clamped() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    let donor = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.treasury_client.set_rate(&0_8000000);
    assert_eq!(f.treasury_client.get_rate(), 0_8000000);

    // the treasury never takes more than MAX_PROTOCOL_RATE (50%)
    f.backstop_client.donate(&donor, &f.pool, &(50 * SCALAR_7));
    let events = f.e.events().all();
    assert_eq!(f.token_client.balance(&donor), 50 * SCALAR_7);
    assert_eq!(f.token_client.balance(&f.treasury), 25 * SCALAR_7);
    assert_eq!(f.token_client.balance(&f.backstop), 125 * SCALAR_7);
    assert_last_event(
        &f.e,
        &events,
        &f.backstop,
        (Symbol::new(&f.e, "donate"), f.pool.clone(), donor.clone()).into_val(&f.e),
        (25 * SCALAR_7).into_val(&f.e),
    );
}

#[test]
fn test_donate_zero() {
    let f = fixture(0);
    let donor = f.user_with(100 * SCALAR_7);
    f.treasury_client.set_rate(&0_1000000);
    f.backstop_client.donate(&donor, &f.pool, &0);
    assert_eq!(f.token_client.balance(&donor), 100 * SCALAR_7);
    assert_eq!(f.token_client.balance(&f.treasury), 0);
    assert_eq!(f.token_client.balance(&f.backstop), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #1000)")]
fn test_donate_from_pool() {
    let f = fixture(0);
    f.token_client.mint(&f.pool, &(100 * SCALAR_7));
    // the pool authorizes as both `from` and `pool_address`
    let invoke = MockAuthInvoke {
        contract: &f.backstop,
        fn_name: "donate",
        args: (f.pool.clone(), f.pool.clone(), 50 * SCALAR_7).into_val(&f.e),
        sub_invokes: &[],
    };
    f.e.mock_auths(&[
        MockAuth {
            address: &f.pool,
            invoke: &invoke,
        },
        MockAuth {
            address: &f.pool,
            invoke: &invoke,
        },
    ]);
    f.backstop_client.donate(&f.pool, &f.pool, &(50 * SCALAR_7));
}

#[test]
#[should_panic(expected = "Error(Contract, #1000)")]
fn test_donate_from_backstop() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client
        .donate(&f.backstop, &f.pool, &(50 * SCALAR_7));
}

#[test]
#[should_panic(expected = "Error(Contract, #1004)")]
fn test_donate_wrong_pool() {
    let f = fixture(0);
    let donor = f.user_with(100 * SCALAR_7);
    f.backstop_client
        .donate(&donor, &Address::generate(&f.e), &(50 * SCALAR_7));
}

#[test]
#[should_panic(expected = "Error(Contract, #8)")]
fn test_donate_negative() {
    let f = fixture(0);
    let donor = f.user_with(100 * SCALAR_7);
    f.backstop_client.donate(&donor, &f.pool, &-1);
}

/********** Share Transfers **********/

#[test]
fn test_transfer() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    let other = Address::generate(&f.e);
    f.deposit(&user, 100 * SCALAR_7);

    f.backstop_client
        .transfer(&user, MuxedAddress::from(other.clone()), &(25 * SCALAR_7));
    assert_eq!(f.backstop_client.balance(&user), 75 * SCALAR_7);
    assert_eq!(f.backstop_client.balance(&other), 25 * SCALAR_7);
    assert_eq!(
        f.backstop_client.user_balance(&f.pool, &other).shares,
        25 * SCALAR_7
    );

    // the receiver can queue and withdraw what they were sent
    f.backstop_client
        .queue_withdrawal(&other, &f.pool, &(25 * SCALAR_7));
    jump(&f.e, LOCK);
    let tokens = f
        .backstop_client
        .withdraw(&other, &f.pool, &(25 * SCALAR_7));
    assert_eq!(tokens, 25 * SCALAR_7);
    assert_eq!(f.token_client.balance(&other), 25 * SCALAR_7);
}

#[test]
fn test_transfer_from() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    let spender = Address::generate(&f.e);
    let other = Address::generate(&f.e);
    f.deposit(&user, 100 * SCALAR_7);

    f.backstop_client.approve(
        &user,
        &spender,
        &(30 * SCALAR_7),
        &(f.e.ledger().sequence() + 100),
    );
    assert_eq!(f.backstop_client.allowance(&user, &spender), 30 * SCALAR_7);
    f.backstop_client
        .transfer_from(&spender, &user, &other, &(25 * SCALAR_7));
    assert_eq!(f.backstop_client.balance(&user), 75 * SCALAR_7);
    assert_eq!(f.backstop_client.balance(&other), 25 * SCALAR_7);
    assert_eq!(f.backstop_client.allowance(&user, &spender), 5 * SCALAR_7);
}

#[test]
#[should_panic(expected = "Error(Contract, #1000)")]
fn test_transfer_to_backstop() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client.transfer(
        &user,
        MuxedAddress::from(f.backstop.clone()),
        &(25 * SCALAR_7),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1000)")]
fn test_transfer_from_to_backstop() {
    let f = fixture(0);
    let user = f.user_with(100 * SCALAR_7);
    let spender = Address::generate(&f.e);
    f.deposit(&user, 100 * SCALAR_7);
    f.backstop_client.approve(
        &user,
        &spender,
        &(25 * SCALAR_7),
        &(f.e.ledger().sequence() + 100),
    );
    f.backstop_client
        .transfer_from(&spender, &user, &f.backstop, &(25 * SCALAR_7));
}

/********** Hook **********/

#[test]
fn test_hook_called_on_deposit() {
    let (f, hook) = hooked_fixture();
    let user = f.user_with(100 * SCALAR_7);
    let other = f.user_with(100 * SCALAR_7);
    assert_eq!(hook.last_call(), None);

    f.deposit(&user, 40 * SCALAR_7);
    assert_eq!(
        hook.last_call(),
        Some(HookCall {
            from: user.clone(),
            amount: 40 * SCALAR_7,
            shares: 40 * SCALAR_7,
        })
    );

    // the hook sees the shares actually minted at the current price
    f.token_client
        .transfer(&other, &f.backstop, &(40 * SCALAR_7));
    let shares = f.deposit(&other, 20 * SCALAR_7);
    assert_eq!(shares, 10 * SCALAR_7);
    assert_eq!(
        hook.last_call(),
        Some(HookCall {
            from: other.clone(),
            amount: 20 * SCALAR_7,
            shares: 10 * SCALAR_7,
        })
    );
}

#[test]
fn test_hook_rejects_deposit() {
    let (f, hook) = hooked_fixture();
    let user = f.user_with(100 * SCALAR_7);
    hook.set_rejecting(&true);
    let result = f
        .backstop_client
        .try_deposit(&user, &f.pool, &(40 * SCALAR_7));
    assert_eq!(
        result.err(),
        Some(Ok(soroban_sdk::Error::from_contract_error(1000)))
    );
    // the deposit was reverted
    assert_eq!(f.backstop_client.balance(&user), 0);
    assert_eq!(f.backstop_client.total_supply(), 0);
    assert_eq!(f.token_client.balance(&user), 100 * SCALAR_7);
    assert_eq!(f.token_client.balance(&f.backstop), 0);
    assert_eq!(hook.last_call(), None);

    // and goes through once the hook accepts again
    hook.set_rejecting(&false);
    f.deposit(&user, 40 * SCALAR_7);
    assert_eq!(f.backstop_client.balance(&user), 40 * SCALAR_7);
}

#[test]
fn test_hook_not_called_on_exits() {
    let (f, hook) = hooked_fixture();
    let user = f.user_with(100 * SCALAR_7);
    let donor = f.user_with(100 * SCALAR_7);
    f.deposit(&user, 100 * SCALAR_7);
    let call = hook.last_call();
    hook.set_rejecting(&true);

    // queue, dequeue, transfer, donate, draw and withdraw all pass with a rejecting hook
    f.backstop_client
        .queue_withdrawal(&user, &f.pool, &(50 * SCALAR_7));
    f.backstop_client
        .dequeue_withdrawal(&user, &f.pool, &(10 * SCALAR_7));
    f.backstop_client.transfer(
        &user,
        MuxedAddress::from(Address::generate(&f.e)),
        &(5 * SCALAR_7),
    );
    f.backstop_client.donate(&donor, &f.pool, &(20 * SCALAR_7));
    f.backstop_client.draw(&f.pool, &(20 * SCALAR_7), &f.pool);
    jump(&f.e, LOCK);
    f.backstop_client.withdraw(&user, &f.pool, &(40 * SCALAR_7));
    assert_eq!(f.token_client.balance(&user), 40 * SCALAR_7);
    assert_eq!(hook.last_call(), call);
}
