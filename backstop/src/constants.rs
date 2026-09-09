/// Fixed-point scalar for 7 decimal numbers
pub const SCALAR_7: i128 = 1_0000000;

/// The largest share of a donation the treasury can take, with 7 decimals. The backstop clamps
/// the rate the treasury reports to this value.
pub const MAX_PROTOCOL_RATE: u32 = 0_5000000;

/// The maximum amount of active Q4W entries that a user can have against the backstop.
pub const MAX_Q4W_SIZE: u32 = 20;

/// The time in seconds that a Q4W entry is locked for (17 days).
pub const Q4W_LOCK_TIME: u64 = 17 * 24 * 60 * 60;
