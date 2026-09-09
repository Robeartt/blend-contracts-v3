use soroban_fixed_point_math::FixedPoint;
use soroban_sdk::{
    contract, contractimpl, contracttype, panic_with_error, token, unwrap::UnwrapOptimized,
    Address, Env, MuxedAddress, String, Vec,
};
use stellar_tokens::{
    fungible::{Base, ContractOverrides, FungibleToken},
    vault::Vault,
};

use crate::{
    constants::{MAX_PROTOCOL_RATE, SCALAR_7},
    dependencies::{PoolClient, TreasuryClient},
    errors::BackstopError,
    events::BackstopEvents,
    hooks::BackstopHookClient,
    queue::{self, Q4W},
    storage,
};

/// The pool's backstop data
#[derive(Clone)]
#[contracttype]
pub struct PoolBackstopData {
    pub tokens: i128,  // the number of backstop tokens held in the pool's backstop
    pub shares: i128,  // the number of shares the pool's backstop has issued
    pub q4w_pct: i128, // the percentage of shares/tokens queued for withdrawal
}

/// A user's backstop balance
#[derive(Clone)]
#[contracttype]
pub struct UserBalance {
    pub shares: i128,  // the balance of shares the user owns, excludes Q4W
    pub q4w: Vec<Q4W>, // a list of queued withdrawals
}

/// ### Backstop
///
/// A backstop module for one Blend Isolated Lending Pool.
///
/// Keeps the Blend v2 backstop's interface, reduced to a single pool: every `pool_address`
/// argument must be the pool this backstop was deployed for. Shares are an ERC-4626 share
/// token over the backstop token (OpenZeppelin `stellar-tokens`), so they are transferable
/// and priced with a virtual offset. Exiting takes v2's two steps: `queue_withdrawal` moves
/// shares into escrow held by the backstop for the lock time, after which `withdraw` burns the
/// escrowed shares for backstop tokens. Only the pool can `draw`.
#[contract]
pub struct BackstopContract;

#[contractimpl]
impl BackstopContract {
    /// Construct the backstop contract
    ///
    /// ### Arguments
    /// * `pool` - The pool this backstop serves, the only address allowed to `draw`
    /// * `backstop_token` - The backstop token
    /// * `treasury` - The treasury that takes the protocol's share of donations
    /// * `decimals_offset` - Extra decimals on shares over the backstop token (ERC-4626 virtual offset)
    /// * `name` - The share token name
    /// * `symbol` - The share token symbol
    /// * `hook` - The hook called after every deposit, if any
    pub fn __constructor(
        e: &Env,
        pool: Address,
        backstop_token: Address,
        treasury: Address,
        decimals_offset: u32,
        name: String,
        symbol: String,
        hook: Option<Address>,
    ) {
        storage::set_pool(e, &pool);
        storage::set_treasury(e, &treasury);
        storage::set_hook(e, &hook);
        Vault::set_asset(e, backstop_token);
        Vault::set_decimals_offset(e, decimals_offset);
        Base::set_metadata(e, Vault::decimals(e), name, symbol);
    }
}

pub trait Backstop {
    /********** Core **********/

    /// Deposit backstop tokens from `from` into the backstop of a pool
    ///
    /// Returns the number of backstop pool shares minted
    ///
    /// ### Arguments
    /// * `from` - The address depositing into the backstop
    /// * `pool_address` - The address of the pool
    /// * `amount` - The amount of tokens to deposit
    ///
    /// ### Errors
    /// If `pool_address` is not the backstop's pool, or if the deposit mints no shares
    fn deposit(e: Env, from: Address, pool_address: Address, amount: i128) -> i128;

    /// Queue deposited pool shares from `from` for withdraw from a backstop of a pool
    ///
    /// Returns the created queue for withdrawal
    ///
    /// ### Arguments
    /// * `from` - The address whose deposits are being queued for withdrawal
    /// * `pool_address` - The address of the pool
    /// * `amount` - The amount of shares to queue for withdraw
    fn queue_withdrawal(e: Env, from: Address, pool_address: Address, amount: i128) -> Q4W;

    /// Dequeue a currently queued pool share withdraw for `from` from the backstop of a pool
    ///
    /// ### Arguments
    /// * `from` - The address whose deposits are being queued for withdrawal
    /// * `pool_address` - The address of the pool
    /// * `amount` - The amount of shares to dequeue
    fn dequeue_withdrawal(e: Env, from: Address, pool_address: Address, amount: i128);

    /// Withdraw shares from `from`s withdraw queue for a backstop of a pool
    ///
    /// Returns the amount of tokens returned
    ///
    /// ### Arguments
    /// * `from` - The address whose shares are being withdrawn
    /// * `pool_address` - The address of the pool
    /// * `amount` - The amount of shares to withdraw
    ///
    /// ### Errors
    /// If the pool has assigned bad debt to the backstop, if the shares are not unlocked, or
    /// if the shares are worth no tokens
    fn withdraw(e: Env, from: Address, pool_address: Address, amount: i128) -> i128;

    /// Fetch the balance of backstop shares of a pool for the user
    ///
    /// ### Arguments
    /// * `pool` - The address of the pool
    /// * `user` - The user to fetch the balance for
    fn user_balance(e: Env, pool: Address, user: Address) -> UserBalance;

    /// Fetch the backstop data for the pool
    ///
    /// Return a summary of the pool's backstop data
    ///
    /// ### Arguments
    /// * `pool` - The address of the pool
    fn pool_data(e: Env, pool: Address) -> PoolBackstopData;

    /// Fetch the backstop token for the backstop
    fn backstop_token(e: Env) -> Address;

    /// Fetch the pool this backstop serves
    fn pool(e: Env) -> Address;

    /// Fetch the hook called after deposits, if any
    fn hook(e: Env) -> Option<Address>;

    /// Fetch the treasury that takes the protocol's share of donations
    fn treasury(e: Env) -> Address;

    /********** Fund Management *********/

    /// (Only Pool) Take backstop token from a pools backstop
    ///
    /// ### Arguments
    /// * `pool_address` - The address of the pool
    /// * `amount` - The amount of backstop tokens to draw
    /// * `to` - The address to send the backstop tokens to
    ///
    /// ### Errors
    /// If the pool does not have enough backstop tokens, or if the pool does
    /// not authorize the call
    fn draw(e: Env, pool_address: Address, amount: i128, to: Address);

    /// (Only Pool) Sends backstop tokens from `from` to a pools backstop
    ///
    /// NOTE: This is not a deposit, and `from` will permanently lose access to the funds
    ///
    /// The treasury takes its share first: `amount` times the rate the treasury reports, capped
    /// at `MAX_PROTOCOL_RATE`, goes from `from` to the treasury and the rest from `from` to the
    /// backstop. The tokens are moved with `transfer` from `from`, so `from`'s authorization of
    /// the call must cover the nested token transfers; no allowance to the backstop is needed.
    ///
    /// ### Arguments
    /// * `from` - The address donating tokens to the backstop
    /// * `pool_address` - The address of the pool
    /// * `amount` - The amount of backstop tokens to add
    ///
    /// ### Errors
    /// If the `pool_address` is not valid, or if the pool does not authorize the call
    fn donate(e: Env, from: Address, pool_address: Address, amount: i128);
}

/// @dev
/// The contract implementation only manages the authorization / authentication required from the
/// caller(s) and the withdrawal queue, and utilizes the OpenZeppelin vault module to carry out the
/// share accounting.
#[contractimpl]
impl Backstop for BackstopContract {
    /********** Core **********/

    fn deposit(e: Env, from: Address, pool_address: Address, amount: i128) -> i128 {
        storage::extend_instance(&e);
        from.require_auth();
        require_is_pool(&e, &pool_address);
        require_nonnegative(&e, amount);
        if from == pool_address || from == e.current_contract_address() {
            panic_with_error!(&e, BackstopError::BadRequest);
        }

        let to_mint = Vault::preview_deposit(&e, amount);
        if to_mint <= 0 {
            panic_with_error!(&e, BackstopError::InvalidShareMintAmount);
        }
        Vault::deposit_internal(&e, &from, amount, to_mint, &from, &from);
        call_hook(&e, &from, amount, to_mint);

        BackstopEvents::deposit(&e, pool_address, from, amount, to_mint);
        to_mint
    }

    fn queue_withdrawal(e: Env, from: Address, pool_address: Address, amount: i128) -> Q4W {
        storage::extend_instance(&e);
        from.require_auth();
        require_is_pool(&e, &pool_address);
        require_nonnegative(&e, amount);

        if Base::balance(&e, &from) < amount {
            panic_with_error!(&e, BackstopError::BalanceError);
        }
        let mut q4w = storage::get_user_q4w(&e, &from);
        let to_queue = queue::queue(&e, &mut q4w, amount);
        storage::set_user_q4w(&e, &from, &q4w);

        Base::update(&e, Some(&from), Some(&e.current_contract_address()), amount);
        storage::set_total_q4w(&e, storage::get_total_q4w(&e) + amount);

        BackstopEvents::queue_withdrawal(&e, pool_address, from, amount, to_queue.exp);
        to_queue
    }

    fn dequeue_withdrawal(e: Env, from: Address, pool_address: Address, amount: i128) {
        storage::extend_instance(&e);
        from.require_auth();
        require_is_pool(&e, &pool_address);
        require_nonnegative(&e, amount);

        let mut q4w = storage::get_user_q4w(&e, &from);
        queue::dequeue(&e, &mut q4w, amount);
        storage::set_user_q4w(&e, &from, &q4w);

        Base::update(&e, Some(&e.current_contract_address()), Some(&from), amount);
        storage::set_total_q4w(&e, storage::get_total_q4w(&e) - amount);

        BackstopEvents::dequeue_withdrawal(&e, pool_address, from, amount);
    }

    fn withdraw(e: Env, from: Address, pool_address: Address, amount: i128) -> i128 {
        storage::extend_instance(&e);
        from.require_auth();
        require_is_pool(&e, &pool_address);
        require_nonnegative(&e, amount);

        let pool_client = PoolClient::new(&e, &pool_address);
        let backstop_positions = pool_client.get_positions(&e.current_contract_address());
        if !backstop_positions.liabilities.is_empty() {
            panic_with_error!(&e, BackstopError::BadDebtExists);
        }

        let mut q4w = storage::get_user_q4w(&e, &from);
        queue::consume_expired(&e, &mut q4w, amount);
        storage::set_user_q4w(&e, &from, &q4w);

        let to_return = Vault::preview_redeem(&e, amount);
        if to_return <= 0 {
            panic_with_error!(&e, BackstopError::InvalidTokenWithdrawAmount);
        }
        Base::update(&e, Some(&e.current_contract_address()), None, amount);
        storage::set_total_q4w(&e, storage::get_total_q4w(&e) - amount);
        token::Client::new(&e, &Vault::query_asset(&e)).transfer(
            &e.current_contract_address(),
            &from,
            &to_return,
        );

        BackstopEvents::withdraw(&e, pool_address, from, amount, to_return);
        to_return
    }

    fn user_balance(e: Env, pool: Address, user: Address) -> UserBalance {
        require_is_pool(&e, &pool);
        UserBalance {
            shares: Base::balance(&e, &user),
            q4w: storage::get_user_q4w(&e, &user),
        }
    }

    fn pool_data(e: Env, pool: Address) -> PoolBackstopData {
        require_is_pool(&e, &pool);
        let shares = Base::total_supply(&e);
        let q4w_pct = if shares > 0 {
            storage::get_total_q4w(&e)
                .fixed_div_ceil(shares, SCALAR_7)
                .unwrap_optimized()
        } else {
            0
        };
        PoolBackstopData {
            tokens: Vault::total_assets(&e),
            shares,
            q4w_pct,
        }
    }

    fn backstop_token(e: Env) -> Address {
        Vault::query_asset(&e)
    }

    fn pool(e: Env) -> Address {
        storage::get_pool(&e)
    }

    fn hook(e: Env) -> Option<Address> {
        storage::get_hook(&e)
    }

    fn treasury(e: Env) -> Address {
        storage::get_treasury(&e)
    }

    /********** Fund Management *********/

    fn draw(e: Env, pool_address: Address, amount: i128, to: Address) {
        storage::extend_instance(&e);
        pool_address.require_auth();
        require_is_pool(&e, &pool_address);
        require_nonnegative(&e, amount);

        if Vault::total_assets(&e) < amount {
            panic_with_error!(&e, BackstopError::InsufficientFunds);
        }
        token::Client::new(&e, &Vault::query_asset(&e)).transfer(
            &e.current_contract_address(),
            &to,
            &amount,
        );

        BackstopEvents::draw(&e, pool_address, to, amount);
    }

    fn donate(e: Env, from: Address, pool_address: Address, amount: i128) {
        storage::extend_instance(&e);
        from.require_auth();
        pool_address.require_auth();
        require_is_pool(&e, &pool_address);
        require_nonnegative(&e, amount);
        if from == pool_address || from == e.current_contract_address() {
            panic_with_error!(&e, BackstopError::BadRequest);
        }

        let treasury = storage::get_treasury(&e);
        let rate = TreasuryClient::new(&e, &treasury)
            .get_rate()
            .min(MAX_PROTOCOL_RATE);
        let to_treasury = amount
            .fixed_mul_floor(i128::from(rate), SCALAR_7)
            .unwrap_optimized();
        let to_backstop = amount - to_treasury;
        let backstop_token = token::Client::new(&e, &Vault::query_asset(&e));
        if to_treasury > 0 {
            backstop_token.transfer(&from, &treasury, &to_treasury);
        }
        if to_backstop > 0 {
            backstop_token.transfer(&from, &e.current_contract_address(), &to_backstop);
        }

        BackstopEvents::donate(&e, pool_address, from, to_backstop);
    }
}

/// The share token. Shares cannot be sent to the backstop itself: escrow is only entered
/// through `queue_withdrawal`, so the queued share count cannot be inflated by a transfer.
#[contractimpl(contracttrait)]
impl FungibleToken for BackstopContract {
    type ContractType = Vault;

    fn decimals(e: &Env) -> u32 {
        Vault::decimals(e)
    }

    fn transfer(e: &Env, from: Address, to: MuxedAddress, amount: i128) {
        require_not_self(e, &to.address());
        Vault::transfer(e, &from, &to, amount);
    }

    fn transfer_from(e: &Env, spender: Address, from: Address, to: Address, amount: i128) {
        require_not_self(e, &to);
        Vault::transfer_from(e, &spender, &from, &to, amount);
    }
}

/// Call the backstop's hook after a deposit, if one is set
fn call_hook(e: &Env, from: &Address, amount: i128, shares: i128) {
    if let Some(hook) = storage::get_hook(e) {
        BackstopHookClient::new(e, &hook).on_backstop_deposit(from, &amount, &shares);
    }
}

/// Require that an incoming amount is not negative
///
/// ### Arguments
/// * `amount` - The amount
///
/// ### Errors
/// If the number is negative
fn require_nonnegative(e: &Env, amount: i128) {
    if amount.is_negative() {
        panic_with_error!(e, BackstopError::NegativeAmountError);
    }
}

/// Require that `pool_address` is the pool this backstop serves
///
/// ### Errors
/// If the address is any other pool
fn require_is_pool(e: &Env, pool_address: &Address) {
    if pool_address != &storage::get_pool(e) {
        panic_with_error!(e, BackstopError::NotPool);
    }
}

/// Require that an address is not the backstop
fn require_not_self(e: &Env, address: &Address) {
    if address == &e.current_contract_address() {
        panic_with_error!(e, BackstopError::BadRequest);
    }
}
