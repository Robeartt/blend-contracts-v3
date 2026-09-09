/**
 * Partial client for the treasury contract
 */
use soroban_sdk::{contractclient, Env};

#[allow(dead_code)]
#[contractclient(name = "TreasuryClient")]
pub trait Treasury {
    /// Fetch the share of each donation the treasury takes, with 7 decimals
    fn get_rate(e: Env) -> u32;
}
