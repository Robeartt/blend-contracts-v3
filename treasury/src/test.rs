use soroban_sdk::{
    testutils::{Address as _, MockAuth, MockAuthInvoke},
    token::{StellarAssetClient, TokenClient},
    Address, Env, IntoVal,
};

use crate::{TreasuryContract, TreasuryContractClient, MAX_PROTOCOL_RATE};

fn setup() -> (Env, TreasuryContractClient<'static>, Address) {
    let e = Env::default();
    e.mock_all_auths();
    let owner = Address::generate(&e);
    let contract_id = e.register(TreasuryContract, (&owner, &0_1000000u32));
    let client = TreasuryContractClient::new(&e, &contract_id);
    (e, client, owner)
}

#[test]
fn test_get_rate() {
    let (_e, client, owner) = setup();
    assert_eq!(client.get_rate(), 0_1000000);
    assert_eq!(client.get_owner(), Some(owner));
}

#[test]
fn test_set_rate() {
    let (_e, client, _owner) = setup();
    client.set_rate(&0_2000000);
    assert_eq!(client.get_rate(), 0_2000000);
    client.set_rate(&0);
    assert_eq!(client.get_rate(), 0);
    client.set_rate(&MAX_PROTOCOL_RATE);
    assert_eq!(client.get_rate(), MAX_PROTOCOL_RATE);
}

#[test]
#[should_panic(expected = "Error(Contract, #1400)")]
fn test_set_rate_above_cap() {
    let (_e, client, _owner) = setup();
    client.set_rate(&(MAX_PROTOCOL_RATE + 1));
}

#[test]
#[should_panic(expected = "Error(Contract, #1400)")]
fn test_constructor_rate_above_cap() {
    let e = Env::default();
    let owner = Address::generate(&e);
    e.register(TreasuryContract, (&owner, &(MAX_PROTOCOL_RATE + 1)));
}

#[test]
#[should_panic(expected = "Error(Auth, InvalidAction)")]
fn test_set_rate_not_owner() {
    let (e, client, _owner) = setup();
    let sauron = Address::generate(&e);
    e.set_auths(&[]);
    client
        .mock_auths(&[MockAuth {
            address: &sauron,
            invoke: &MockAuthInvoke {
                contract: &client.address,
                fn_name: "set_rate",
                args: (0_2000000u32,).into_val(&e),
                sub_invokes: &[],
            },
        }])
        .set_rate(&0_2000000);
}

#[test]
fn test_withdraw() {
    let (e, client, owner) = setup();
    let token_admin = Address::generate(&e);
    let token_id = e.register_stellar_asset_contract_v2(token_admin.clone());
    StellarAssetClient::new(&e, &token_id.address()).mint(&client.address, &1_000);

    client.withdraw(&token_id.address(), &owner, &500);

    let token = TokenClient::new(&e, &token_id.address());
    assert_eq!(token.balance(&owner), 500);
    assert_eq!(token.balance(&client.address), 500);
}
