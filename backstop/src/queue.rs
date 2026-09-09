use soroban_sdk::{contracttype, panic_with_error, Env, Vec};

use crate::{
    constants::{MAX_Q4W_SIZE, Q4W_LOCK_TIME},
    errors::BackstopError,
};

/// Shares queued for withdrawal, unlocked at `exp`
#[derive(Clone, Debug, PartialEq)]
#[contracttype]
pub struct Q4W {
    pub amount: i128, // the amount of shares queued for withdrawal
    pub exp: u64,     // the expiration of the withdrawal
}

/// Append a withdrawal queue entry for `shares` unlocking after the lock time
///
/// Returns the new Q4W entry
///
/// ### Errors
/// If the queue already holds the maximum number of entries
pub fn queue(e: &Env, q4w: &mut Vec<Q4W>, shares: i128) -> Q4W {
    if q4w.len() >= MAX_Q4W_SIZE {
        panic_with_error!(e, BackstopError::TooManyQ4WEntries);
    }
    let entry = Q4W {
        amount: shares,
        exp: e.ledger().timestamp() + Q4W_LOCK_TIME,
    };
    q4w.push_back(entry.clone());
    entry
}

/// Remove `shares` from the queue, consuming the most recently queued entries first
///
/// ### Errors
/// If the queue holds fewer than `shares`
#[allow(clippy::comparison_chain)]
pub fn dequeue(e: &Env, q4w: &mut Vec<Q4W>, shares: i128) {
    let mut left_to_dequeue: i128 = shares;
    for _index in 0..q4w.len() {
        let mut cur_q4w = q4w.pop_back_unchecked();
        if cur_q4w.amount > left_to_dequeue {
            // last record we need to update, but the q4w should remain
            cur_q4w.amount -= left_to_dequeue;
            left_to_dequeue = 0;
            q4w.push_back(cur_q4w);
            break;
        } else if cur_q4w.amount == left_to_dequeue {
            // last record we need to update, q4w fully consumed
            left_to_dequeue = 0;
            break;
        } else {
            // allow the pop to consume the record
            left_to_dequeue -= cur_q4w.amount;
        }
    }

    if left_to_dequeue > 0 {
        panic_with_error!(e, BackstopError::BalanceError);
    }
}

/// Remove `shares` from the queue, consuming the oldest entries first. Every entry touched
/// must be unlocked.
///
/// ### Errors
/// If an entry that would be consumed is still locked, or if the queue holds fewer than `shares`
#[allow(clippy::comparison_chain)]
pub fn consume_expired(e: &Env, q4w: &mut Vec<Q4W>, shares: i128) {
    let mut left_to_withdraw: i128 = shares;
    for _index in 0..q4w.len() {
        let mut cur_q4w = q4w.pop_front_unchecked();
        if cur_q4w.exp <= e.ledger().timestamp() {
            if cur_q4w.amount > left_to_withdraw {
                // last record we need to update, but the q4w should remain
                cur_q4w.amount -= left_to_withdraw;
                left_to_withdraw = 0;
                q4w.push_front(cur_q4w);
                break;
            } else if cur_q4w.amount == left_to_withdraw {
                // last record we need to update, q4w fully consumed
                left_to_withdraw = 0;
                break;
            } else {
                // allow the pop to consume the record
                left_to_withdraw -= cur_q4w.amount;
            }
        } else {
            panic_with_error!(e, BackstopError::NotExpired);
        }
    }

    if left_to_withdraw > 0 {
        panic_with_error!(e, BackstopError::BalanceError);
    }
}
