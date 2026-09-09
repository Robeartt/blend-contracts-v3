#![cfg(test)]

use crate::{
    constants::{SCALAR_12, SCALAR_7},
    pool::Reserve,
    storage::{self, ReserveConfig, ReserveData},
    PoolContract, PoolError, Positions, Request,
};
use sep_40_oracle::testutils::{MockPriceOracleClient, MockPriceOracleWASM};
use sep_41_token::testutils::{MockTokenClient, MockTokenWASM};
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::{
    contract, contractimpl, contracttype, panic_with_error, testutils::Address as _, Address, Env,
    IntoVal, String, Symbol, Vec,
};

use backstop::{BackstopContract, BackstopContractClient};
use moderc3156_example::{
    FlashLoanReceiverModifiedERC3156, FlashLoanReceiverModifiedERC3156Client,
};

/// The protocol version every test ledger runs at
pub(crate) const PROTOCOL_VERSION: u32 = 27;

/// Create a pool contract.
///
/// This sets random data in the constructor, so unit tests that
/// rely on any constructor data need to reset it.
pub(crate) fn create_pool(e: &Env) -> Address {
    e.register(
        PoolContract {},
        (
            Address::generate(e),
            String::from_str(e, "teapot"),
            Address::generate(e),
            0_1000000u32,
            4u32,
            1_0000000i128,
            0i128,
            Address::generate(e),
            None::<Address>,
        ),
    )
}

//************************************************
//           External Contract Helpers
//************************************************

// ***** Token *****

pub(crate) fn create_token_contract<'a>(
    e: &Env,
    admin: &Address,
) -> (Address, MockTokenClient<'a>) {
    let contract_address = Address::generate(e);
    e.register_at(&contract_address, MockTokenWASM, ());
    let client = MockTokenClient::new(e, &contract_address);
    client.initialize(admin, &7, &"unit".into_val(e), &"test".into_val(e));
    (contract_address, client)
}

pub(crate) fn create_blnd_token<'a>(
    e: &Env,
    _pool_address: &Address,
    admin: &Address,
) -> (Address, MockTokenClient<'a>) {
    create_token_contract(e, admin)
}

//***** Oracle ******

pub(crate) fn create_mock_oracle(e: &Env) -> (Address, MockPriceOracleClient<'_>) {
    let contract_address = e.register(MockPriceOracleWASM, ());
    (
        contract_address.clone(),
        MockPriceOracleClient::new(e, &contract_address),
    )
}

//***** Pool Backstop ******

/// Deploy a backstop for `pool_address` holding `backstop_token` with a treasury taking no
/// share of donations, set it as the pool's backstop and set the pool's `min_backstop`
/// activation threshold.
pub(crate) fn create_backstop<'a>(
    e: &Env,
    pool_address: &Address,
    backstop_token: &Address,
    min_backstop: i128,
) -> (Address, BackstopContractClient<'a>) {
    let (treasury_id, _) = create_mock_treasury(e, 0);
    let backstop_id = e.register(
        BackstopContract {},
        (
            pool_address,
            backstop_token,
            &treasury_id,
            0u32,
            String::from_str(e, "Backstop Share"),
            String::from_str(e, "BSS"),
            None::<Address>,
        ),
    );
    e.as_contract(pool_address, || {
        storage::set_backstop(e, &backstop_id);
        storage::set_min_backstop(e, min_backstop);
    });
    (
        backstop_id.clone(),
        BackstopContractClient::new(e, &backstop_id),
    )
}

//***** Treasury ******

/// A treasury that reports whatever rate it is given, with no cap
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

/// Deploy a mock treasury reporting `rate`
pub(crate) fn create_mock_treasury<'a>(e: &Env, rate: u32) -> (Address, MockTreasuryClient<'a>) {
    let treasury_id = e.register(MockTreasury {}, ());
    let client = MockTreasuryClient::new(e, &treasury_id);
    client.set_rate(&rate);
    (treasury_id, client)
}

//***** Pool Hook ******

/// The last submit a mock hook saw
#[derive(Clone)]
#[contracttype]
pub struct SubmitCall {
    pub from: Address,
    pub spender: Address,
    pub to: Address,
    pub requests: Vec<Request>,
    pub reserves: Vec<Reserve>,
    pub positions: Positions,
}

/// A hook that counts and records submits and rejects when `rejecting` is set
#[contract]
pub struct MockPoolHook;

#[contractimpl]
impl MockPoolHook {
    pub fn set_rejecting(e: Env, rejecting: bool) {
        e.storage()
            .instance()
            .set(&Symbol::new(&e, "Reject"), &rejecting);
    }

    /// Return immediately from `on_submit`, to measure the pool's cost of calling a hook
    pub fn set_silent(e: Env, silent: bool) {
        e.storage()
            .instance()
            .set(&Symbol::new(&e, "Silent"), &silent);
    }

    pub fn calls(e: Env) -> u32 {
        e.storage()
            .instance()
            .get(&Symbol::new(&e, "Calls"))
            .unwrap_or(0)
    }

    pub fn last_submit(e: Env) -> Option<SubmitCall> {
        e.storage().instance().get(&Symbol::new(&e, "Last"))
    }

    pub fn on_submit(
        e: Env,
        from: Address,
        spender: Address,
        to: Address,
        requests: Vec<Request>,
        reserves: Vec<Reserve>,
        positions: Positions,
    ) {
        if e.storage()
            .instance()
            .get::<Symbol, bool>(&Symbol::new(&e, "Reject"))
            .unwrap_or(false)
        {
            panic_with_error!(&e, PoolError::BadRequest);
        }
        if e.storage()
            .instance()
            .get::<Symbol, bool>(&Symbol::new(&e, "Silent"))
            .unwrap_or(false)
        {
            return;
        }
        let calls: u32 = e
            .storage()
            .instance()
            .get(&Symbol::new(&e, "Calls"))
            .unwrap_or(0);
        e.storage()
            .instance()
            .set(&Symbol::new(&e, "Calls"), &(calls + 1));
        e.storage().instance().set(
            &Symbol::new(&e, "Last"),
            &SubmitCall {
                from,
                spender,
                to,
                requests,
                reserves,
                positions,
            },
        );
    }
}

/// Deploy a mock hook and set it as the pool's hook
pub(crate) fn create_mock_hook<'a>(
    e: &Env,
    pool_address: &Address,
) -> (Address, MockPoolHookClient<'a>) {
    let hook_id = e.register(MockPoolHook {}, ());
    e.as_contract(pool_address, || {
        storage::set_hook(e, &Some(hook_id.clone()));
    });
    (hook_id.clone(), MockPoolHookClient::new(e, &hook_id))
}

//***** Flash Loan *****

/// Create a flash loan receiver contract.
///
/// This returns the tokens received from the flash loan to the "caller" for
/// test purposes.
pub fn create_flashloan_receiver<'a>(
    e: &Env,
) -> (Address, FlashLoanReceiverModifiedERC3156Client<'a>) {
    let contract_id = Address::generate(e);
    e.register_at(&contract_id, FlashLoanReceiverModifiedERC3156 {}, ());

    (
        contract_id.clone(),
        FlashLoanReceiverModifiedERC3156Client::new(e, &contract_id),
    )
}

//************************************************
//            Object Creation Helpers
//************************************************

//***** Reserve *****

pub(crate) fn default_reserve(e: &Env) -> Reserve {
    Reserve {
        asset: Address::generate(e),
        config: ReserveConfig {
            decimals: 7,
            c_factor: 0_7500000,
            l_factor: 0_7500000,
            util: 0_7500000,
            max_util: 0_9500000,
            r_base: 0_0100000,
            r_one: 0_0500000,
            r_two: 0_5000000,
            r_three: 1_5000000,
            reactivity: 0_0000020, // 2e-6
            index: 0,
            supply_cap: 1000000000000000000,
            enabled: true,
        },
        data: ReserveData {
            b_rate: SCALAR_12,
            d_rate: SCALAR_12,
            ir_mod: SCALAR_7,
            b_supply: 100_0000000,
            d_supply: 75_0000000,
            last_time: 0,
            backstop_credit: 0,
        },
        scalar: SCALAR_7,
    }
}

pub(crate) fn default_reserve_meta() -> (ReserveConfig, ReserveData) {
    (
        ReserveConfig {
            decimals: 7,
            c_factor: 0_7500000,
            l_factor: 0_7500000,
            util: 0_7500000,
            max_util: 0_9500000,
            r_base: 0_0100000,
            r_one: 0_0500000,
            r_two: 0_5000000,
            r_three: 1_5000000,
            reactivity: 0_0000020, // 2e-6
            index: 0,
            supply_cap: 1000000000000000000,
            enabled: true,
        },
        ReserveData {
            b_rate: SCALAR_12,
            d_rate: SCALAR_12,
            ir_mod: SCALAR_7,
            b_supply: 100_0000000,
            d_supply: 75_0000000,
            last_time: 0,
            backstop_credit: 0,
        },
    )
}

/// Create a reserve based on the supplied config and data.
///
/// Mints the appropriate amount of underlying tokens to the pool based on the
/// b and d token supply and rates.
///
/// Returns the underlying asset address.
pub(crate) fn create_reserve(
    e: &Env,
    pool_address: &Address,
    token_address: &Address,
    reserve_config: &ReserveConfig,
    reserve_data: &ReserveData,
) {
    let mut new_reserve_config = reserve_config.clone();
    e.as_contract(pool_address, || {
        let index = storage::push_res_list(e, &token_address);
        new_reserve_config.index = index;
        storage::set_res_config(e, &token_address, &new_reserve_config);
        storage::set_res_data(e, &token_address, &reserve_data);
    });
    let underlying_client = MockTokenClient::new(e, token_address);

    // mint pool assets to set expected b_rate
    let total_supply = reserve_data
        .b_supply
        .fixed_mul_floor(e, &reserve_data.b_rate, &SCALAR_12);
    let total_liabilities =
        reserve_data
            .d_supply
            .fixed_mul_floor(e, &reserve_data.d_rate, &SCALAR_12);
    let to_mint_pool = total_supply - total_liabilities + reserve_data.backstop_credit;
    underlying_client
        .mock_all_auths()
        .mint(&pool_address, &to_mint_pool);
}
