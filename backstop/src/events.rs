use soroban_sdk::{contractevent, Address, Env};

/// The backstop's events keep the Blend v2 backstop's topics and data. Share transfers emit the
/// SEP-41 events of the OpenZeppelin token implementation.

/// Emitted when tokens are deposited into a backstop
///
/// - topics - `["deposit", pool_address: Address, from: Address]`
/// - data - `[tokens_in: i128, backstop_shares_minted: i128]`
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deposit {
    #[topic]
    pub pool_address: Address,
    #[topic]
    pub from: Address,
    pub tokens_in: i128,
    pub backstop_shares_minted: i128,
}

/// Emitted when a withdrawal is queued
///
/// - topics - `["queue_withdrawal", pool_address: Address, from: Address]`
/// - data - `[amount: i128, expiration: u64]`
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QueueWithdrawal {
    #[topic]
    pub pool_address: Address,
    #[topic]
    pub from: Address,
    pub amount: i128,
    pub expiration: u64,
}

/// Emitted when a withdrawal is dequeued
///
/// - topics - `["dequeue_withdrawal", pool_address: Address, from: Address]`
/// - data - `amount: i128`
#[contractevent(data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DequeueWithdrawal {
    #[topic]
    pub pool_address: Address,
    #[topic]
    pub from: Address,
    pub amount: i128,
}

/// Emitted when tokens are withdrawn from the backstop
///
/// - topics - `["withdraw", pool_address: Address, from: Address]`
/// - data - `[amount: i128, tokens_out: i128]`
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Withdraw {
    #[topic]
    pub pool_address: Address,
    #[topic]
    pub from: Address,
    pub amount: i128,
    pub tokens_out: i128,
}

/// Emitted when tokens are drawn from the backstop
///
/// - topics - `["draw", pool_address: Address]`
/// - data - `[to: Address, amount: i128]`
#[contractevent(data_format = "vec")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Draw {
    #[topic]
    pub pool_address: Address,
    pub to: Address,
    pub amount: i128,
}

/// Emitted when tokens are donated to the backstop. `amount` is what the backstop received; the
/// treasury's share is visible as the backstop token's own transfer event.
///
/// - topics - `["donate", pool_address: Address, from: Address]`
/// - data - `amount: i128`
#[contractevent(data_format = "single-value")]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Donate {
    #[topic]
    pub pool_address: Address,
    #[topic]
    pub from: Address,
    pub amount: i128,
}

pub struct BackstopEvents {}

impl BackstopEvents {
    /// Emitted when tokens are deposited into a backstop
    ///
    /// ### Arguments
    /// * `pool_address` - The address of the pool
    /// * `from` - The address of the user depositing tokens
    /// * `tokens_in` - The amount of tokens sent to the backstop
    /// * `backstop_shares_minted` - The amount of backstop shares minted
    pub fn deposit(
        e: &Env,
        pool_address: Address,
        from: Address,
        tokens_in: i128,
        backstop_shares_minted: i128,
    ) {
        Deposit {
            pool_address,
            from,
            tokens_in,
            backstop_shares_minted,
        }
        .publish(e);
    }

    /// Emitted when a withdrawal is queued
    ///
    /// ### Arguments
    /// * `pool_address` - The address of the pool
    /// * `from` - The address of the user queuing the withdrawal
    /// * `amount` - The amount of shares being queued for withdrawal
    /// * `expiration` - The expiration timestamp of the withdrawal request
    pub fn queue_withdrawal(
        e: &Env,
        pool_address: Address,
        from: Address,
        amount: i128,
        expiration: u64,
    ) {
        QueueWithdrawal {
            pool_address,
            from,
            amount,
            expiration,
        }
        .publish(e);
    }

    /// Emitted when a withdrawal is dequeued
    ///
    /// ### Arguments
    /// * `pool_address` - The address of the pool
    /// * `from` - The address of the user dequeuing the withdrawal
    /// * `amount` - The amount of shares being dequeued
    pub fn dequeue_withdrawal(e: &Env, pool_address: Address, from: Address, amount: i128) {
        DequeueWithdrawal {
            pool_address,
            from,
            amount,
        }
        .publish(e);
    }

    /// Emitted when tokens are withdrawn from the backstop
    ///
    /// ### Arguments
    /// * `pool_address` - The address of the pool
    /// * `from` - The address of the user withdrawing tokens
    /// * `amount` - The amount of backstop shares being burned
    /// * `tokens_out` - The amount of tokens being withdrawn
    pub fn withdraw(e: &Env, pool_address: Address, from: Address, amount: i128, tokens_out: i128) {
        Withdraw {
            pool_address,
            from,
            amount,
            tokens_out,
        }
        .publish(e);
    }

    /// Emitted when tokens are drawn from the backstop
    ///
    /// ### Arguments
    /// * `pool_address` - The address of the pool
    /// * `to` - The address receiving the drawn tokens
    /// * `amount` - The amount of tokens drawn
    pub fn draw(e: &Env, pool_address: Address, to: Address, amount: i128) {
        Draw {
            pool_address,
            to,
            amount,
        }
        .publish(e);
    }

    /// Emitted when tokens are donated to the backstop
    ///
    /// ### Arguments
    /// * `pool_address` - The address of the pool
    /// * `from` - The address of the donor
    /// * `amount` - The amount of tokens the backstop received
    pub fn donate(e: &Env, pool_address: Address, from: Address, amount: i128) {
        Donate {
            pool_address,
            from,
            amount,
        }
        .publish(e);
    }
}
