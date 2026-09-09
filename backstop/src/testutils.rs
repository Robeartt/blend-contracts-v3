#![cfg(test)]

use crate::{BackstopContract, BackstopContractClient};
use mock_pool::{MockPool, MockPoolClient};
use sep_41_token::testutils::{MockTokenClient, MockTokenWASM};
use soroban_sdk::{
    contract, contractimpl, contracttype, panic_with_error,
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, Env, IntoVal, String, Symbol,
};

/********** Mock Treasury **********/

/// A treasury that reports whatever rate it was given, without any cap
#[contract]
pub struct MockTreasury;

#[contractimpl]
impl MockTreasury {
    pub fn set_rate(e: Env, rate: u32) {
        e.storage().instance().set(&Symbol::new(&e, "Rate"), &rate);
    }

    pub fn get_rate(e: Env) -> u32 {
        e.storage()
            .instance()
            .get(&Symbol::new(&e, "Rate"))
            .unwrap_or(0)
    }
}

/********** Mock Hook **********/

/// The last deposit a mock hook saw
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct HookCall {
    pub from: Address,
    pub amount: i128,
    pub shares: i128,
}

/// A hook that records the last deposit and rejects when `rejecting` is set
#[contract]
pub struct MockBackstopHook;

#[contractimpl]
impl MockBackstopHook {
    pub fn set_rejecting(e: Env, rejecting: bool) {
        e.storage()
            .instance()
            .set(&Symbol::new(&e, "Reject"), &rejecting);
    }

    pub fn last_call(e: Env) -> Option<HookCall> {
        e.storage().instance().get(&Symbol::new(&e, "Last"))
    }

    pub fn on_backstop_deposit(e: Env, from: Address, amount: i128, shares: i128) {
        if e.storage()
            .instance()
            .get::<Symbol, bool>(&Symbol::new(&e, "Reject"))
            .unwrap_or(false)
        {
            panic_with_error!(&e, crate::BackstopError::BadRequest);
        }
        e.storage().instance().set(
            &Symbol::new(&e, "Last"),
            &HookCall {
                from,
                amount,
                shares,
            },
        );
    }
}

/********** Env **********/

pub(crate) const ONE_DAY: u64 = 24 * 60 * 60;

/// The protocol version every test ledger runs at
pub(crate) const PROTOCOL_VERSION: u32 = 27;

pub(crate) fn create_env() -> Env {
    let e = Env::default();
    e.mock_all_auths();
    e.ledger().set(LedgerInfo {
        timestamp: 1441065600, // Sept 1st, 2015
        protocol_version: PROTOCOL_VERSION,
        sequence_number: 150,
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 500000,
        min_persistent_entry_ttl: 500000,
        max_entry_ttl: 9999999,
    });
    e
}

pub(crate) fn jump(e: &Env, seconds: u64) {
    e.ledger().with_mut(|li| {
        li.timestamp += seconds;
        li.sequence_number += (seconds / 5) as u32;
    });
}

/********** Contracts **********/

pub(crate) fn create_token<'a>(e: &Env, admin: &Address) -> (Address, MockTokenClient<'a>) {
    let contract_address = e.register(MockTokenWASM, ());
    let client = MockTokenClient::new(e, &contract_address);
    client.initialize(admin, &7, &"unit".into_val(e), &"test".into_val(e));
    (contract_address, client)
}

pub(crate) fn create_mock_pool<'a>(e: &Env) -> (Address, MockPoolClient<'a>) {
    let contract_address = e.register(MockPool, ());
    (
        contract_address.clone(),
        MockPoolClient::new(e, &contract_address),
    )
}

pub(crate) fn create_mock_treasury<'a>(e: &Env) -> (Address, MockTreasuryClient<'a>) {
    let contract_address = e.register(MockTreasury, ());
    (
        contract_address.clone(),
        MockTreasuryClient::new(e, &contract_address),
    )
}

pub(crate) fn create_mock_hook<'a>(e: &Env) -> (Address, MockBackstopHookClient<'a>) {
    let contract_address = e.register(MockBackstopHook, ());
    (
        contract_address.clone(),
        MockBackstopHookClient::new(e, &contract_address),
    )
}

pub(crate) fn create_backstop<'a>(
    e: &Env,
    pool: &Address,
    backstop_token: &Address,
    treasury: &Address,
    decimals_offset: u32,
    hook: Option<Address>,
) -> (Address, BackstopContractClient<'a>) {
    let contract_address = e.register(
        BackstopContract {},
        (
            pool,
            backstop_token,
            treasury,
            decimals_offset,
            String::from_str(e, "Pool Backstop Share"),
            String::from_str(e, "PBS"),
            hook,
        ),
    );
    (
        contract_address.clone(),
        BackstopContractClient::new(e, &contract_address),
    )
}

/********** Fixture **********/

/// A backstop with a mock pool, a mock treasury (rate 0) and a 7 decimal mock backstop token
pub(crate) struct Fixture<'a> {
    pub e: Env,
    pub pool: Address,
    pub pool_client: MockPoolClient<'a>,
    pub token: Address,
    pub token_client: MockTokenClient<'a>,
    pub treasury: Address,
    pub treasury_client: MockTreasuryClient<'a>,
    pub backstop: Address,
    pub backstop_client: BackstopContractClient<'a>,
}

pub(crate) fn fixture<'a>(decimals_offset: u32) -> Fixture<'a> {
    fixture_with_hook(decimals_offset, None)
}

/// A fixture whose backstop calls a `MockBackstopHook` after deposits
pub(crate) fn hooked_fixture<'a>() -> (Fixture<'a>, MockBackstopHookClient<'a>) {
    let e = create_env();
    let (hook, hook_client) = create_mock_hook(&e);
    (fixture_in(e, 0, Some(hook)), hook_client)
}

fn fixture_with_hook<'a>(decimals_offset: u32, hook: Option<Address>) -> Fixture<'a> {
    fixture_in(create_env(), decimals_offset, hook)
}

fn fixture_in<'a>(e: Env, decimals_offset: u32, hook: Option<Address>) -> Fixture<'a> {
    let admin = Address::generate(&e);
    let (pool, pool_client) = create_mock_pool(&e);
    let (token, token_client) = create_token(&e, &admin);
    let (treasury, treasury_client) = create_mock_treasury(&e);
    let (backstop, backstop_client) =
        create_backstop(&e, &pool, &token, &treasury, decimals_offset, hook);
    Fixture {
        e,
        pool,
        pool_client,
        token,
        token_client,
        treasury,
        treasury_client,
        backstop,
        backstop_client,
    }
}

impl Fixture<'_> {
    /// A new address holding `tokens` of the backstop token
    pub fn user_with(&self, tokens: i128) -> Address {
        let user = Address::generate(&self.e);
        self.token_client.mint(&user, &tokens);
        user
    }

    /// Deposit `amount` of backstop tokens for `user`, returning the shares minted
    pub fn deposit(&self, user: &Address, amount: i128) -> i128 {
        self.backstop_client.deposit(user, &self.pool, &amount)
    }
}
