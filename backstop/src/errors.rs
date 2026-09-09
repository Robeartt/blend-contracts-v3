use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
/// Error codes for the backstop contract. Common errors are codes that match up with the built-in
/// contracts error reporting. Backstop specific errors start at 1000 and keep the Blend v2
/// backstop's codes for the same conditions. Share-token errors come from the OpenZeppelin
/// `stellar-tokens` crate (codes 100-112 and 400-410).
pub enum BackstopError {
    // Common Errors
    InternalError = 1,
    NegativeAmountError = 8,
    BalanceError = 10,

    // Backstop
    BadRequest = 1000,
    NotExpired = 1001,
    InsufficientFunds = 1003,
    NotPool = 1004, // `pool_address` is not the pool this backstop serves
    InvalidShareMintAmount = 1005,
    InvalidTokenWithdrawAmount = 1006,
    TooManyQ4WEntries = 1007,
    BadDebtExists = 1011,
}
