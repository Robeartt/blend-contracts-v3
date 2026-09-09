use soroban_sdk::{map, testutils::Address as _, vec, Address, Env, Vec};

use crate::{
    pool::{Positions, Request, Reserve},
    SupplierOnlyHook, SupplierOnlyHookClient,
};

fn setup() -> (Env, SupplierOnlyHookClient<'static>, Address) {
    let e = Env::default();
    e.mock_all_auths();
    let owner = Address::generate(&e);
    let fee_vault = Address::generate(&e);
    let hook = e.register(SupplierOnlyHook, (&owner, &fee_vault));
    let client = SupplierOnlyHookClient::new(&e, &hook);
    (e, client, fee_vault)
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
fn test_supplier_management() {
    let (e, client, fee_vault) = setup();
    assert_eq!(client.get_supplier(), fee_vault);
    let other = Address::generate(&e);
    client.set_supplier(&other);
    assert_eq!(client.get_supplier(), other);
}

#[test]
fn test_supply_from_supplier_passes() {
    let (e, client, fee_vault) = setup();
    let requests = vec![&e, request(&e, 0), request(&e, 1)];
    client.on_submit(
        &fee_vault,
        &fee_vault,
        &fee_vault,
        &requests,
        &no_reserves(&e),
        &empty_positions(&e),
    );
}

#[test]
fn test_collateral_and_borrow_pass_for_anyone() {
    let (e, client, _) = setup();
    let sam = Address::generate(&e);
    let requests = vec![&e, request(&e, 2), request(&e, 4), request(&e, 6)];
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
fn test_supply_from_other_rejected() {
    let (e, client, _) = setup();
    let sam = Address::generate(&e);
    let requests = vec![&e, request(&e, 2), request(&e, 0)];
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
    let (e, client, _) = setup();
    let sam = Address::generate(&e);
    client.on_backstop_deposit(&sam, &100, &100);
}
