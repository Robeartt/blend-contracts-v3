use soroban_sdk::{map, testutils::Address as _, vec, Address, Env, Vec};

use crate::{
    pool::{Positions, Request, Reserve},
    GatedLiquidatorHook, GatedLiquidatorHookClient,
};

fn setup() -> (Env, GatedLiquidatorHookClient<'static>) {
    let e = Env::default();
    e.mock_all_auths();
    let owner = Address::generate(&e);
    let hook = e.register(GatedLiquidatorHook, (&owner,));
    let client = GatedLiquidatorHookClient::new(&e, &hook);
    (e, client)
}

fn empty_positions(e: &Env) -> Positions {
    Positions {
        liabilities: map![e],
        collateral: map![e],
        supply: map![e],
    }
}

fn no_reserves(e: &Env) -> Vec<Reserve> {
    vec![e]
}

fn request(e: &Env, request_type: u32) -> Request {
    Request {
        request_type,
        address: Address::generate(e),
        amount: 1,
    }
}

#[test]
fn test_non_fill_requests_pass_for_anyone() {
    let (e, client) = setup();
    let sam = Address::generate(&e);
    let requests = vec![
        &e,
        request(&e, 0),
        request(&e, 2),
        request(&e, 4),
        request(&e, 5),
        request(&e, 9),
    ];
    client.on_submit(
        &sam,
        &sam,
        &sam,
        &requests,
        &no_reserves(&e),
        &empty_positions(&e),
    );
}

#[test]
fn test_fill_requests_pass_for_listed_filler() {
    let (e, client) = setup();
    let sam = Address::generate(&e);
    client.set_allowed(&sam, &true);
    for fill in 6..=8u32 {
        let requests = vec![&e, request(&e, 2), request(&e, fill)];
        client.on_submit(
            &sam,
            &sam,
            &sam,
            &requests,
            &no_reserves(&e),
            &empty_positions(&e),
        );
    }
}

#[test]
#[should_panic(expected = "Error(Contract, #1500)")]
fn test_user_liquidation_fill_unlisted() {
    let (e, client) = setup();
    let sam = Address::generate(&e);
    let requests = vec![&e, request(&e, 2), request(&e, 6)];
    client.on_submit(
        &sam,
        &sam,
        &sam,
        &requests,
        &no_reserves(&e),
        &empty_positions(&e),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1500)")]
fn test_interest_fill_unlisted() {
    let (e, client) = setup();
    let sam = Address::generate(&e);
    let requests = vec![&e, request(&e, 8)];
    client.on_submit(
        &sam,
        &sam,
        &sam,
        &requests,
        &no_reserves(&e),
        &empty_positions(&e),
    );
}

#[test]
fn test_backstop_deposit_has_no_rule() {
    let (e, client) = setup();
    let sam = Address::generate(&e);
    client.on_backstop_deposit(&sam, &100, &100);
}
