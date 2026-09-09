use soroban_sdk::{map, testutils::Address as _, vec, Address, Env, Vec};

use crate::{
    pool::{Positions, Request, Reserve},
    AllowlistHook, AllowlistHookClient,
};

fn setup() -> (Env, AllowlistHookClient<'static>, Address) {
    let e = Env::default();
    e.mock_all_auths();
    let owner = Address::generate(&e);
    let hook = e.register(AllowlistHook, (&owner,));
    let client = AllowlistHookClient::new(&e, &hook);
    (e, client, owner)
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

#[test]
fn test_list_management() {
    let (e, client, owner) = setup();
    let frodo = Address::generate(&e);
    assert_eq!(client.get_owner(), Some(owner));
    assert!(!client.is_allowed(&frodo));
    client.set_allowed(&frodo, &true);
    assert!(client.is_allowed(&frodo));
    client.set_allowed(&frodo, &false);
    assert!(!client.is_allowed(&frodo));
}

#[test]
fn test_on_submit_all_parties_listed() {
    let (e, client, _) = setup();
    let frodo = Address::generate(&e);
    let sam = Address::generate(&e);
    client.set_allowed(&frodo, &true);
    client.set_allowed(&sam, &true);
    let requests = vec![
        &e,
        Request {
            request_type: 0,
            address: Address::generate(&e),
            amount: 1,
        },
    ];
    client.on_submit(
        &frodo,
        &sam,
        &frodo,
        &requests,
        &no_reserves(&e),
        &empty_positions(&e),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1500)")]
fn test_on_submit_unlisted_from() {
    let (e, client, _) = setup();
    let frodo = Address::generate(&e);
    client.on_submit(
        &frodo,
        &frodo,
        &frodo,
        &vec![&e],
        &no_reserves(&e),
        &empty_positions(&e),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1500)")]
fn test_on_submit_unlisted_spender() {
    let (e, client, _) = setup();
    let frodo = Address::generate(&e);
    let sam = Address::generate(&e);
    client.set_allowed(&frodo, &true);
    client.on_submit(
        &frodo,
        &sam,
        &frodo,
        &vec![&e],
        &no_reserves(&e),
        &empty_positions(&e),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #1500)")]
fn test_on_submit_unlisted_to() {
    let (e, client, _) = setup();
    let frodo = Address::generate(&e);
    let sam = Address::generate(&e);
    client.set_allowed(&frodo, &true);
    client.on_submit(
        &frodo,
        &frodo,
        &sam,
        &vec![&e],
        &no_reserves(&e),
        &empty_positions(&e),
    );
}

#[test]
fn test_on_backstop_deposit_listed() {
    let (e, client, _) = setup();
    let frodo = Address::generate(&e);
    client.set_allowed(&frodo, &true);
    client.on_backstop_deposit(&frodo, &100, &100);
}

#[test]
#[should_panic(expected = "Error(Contract, #1500)")]
fn test_on_backstop_deposit_unlisted() {
    let (e, client, _) = setup();
    let frodo = Address::generate(&e);
    client.on_backstop_deposit(&frodo, &100, &100);
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn test_set_allowed_not_owner() {
    let e = Env::default();
    let owner = Address::generate(&e);
    let hook = e.register(AllowlistHook, (&owner,));
    let client = AllowlistHookClient::new(&e, &hook);
    // no auths mocked: the owner has not signed
    client.set_allowed(&Address::generate(&e), &true);
}
