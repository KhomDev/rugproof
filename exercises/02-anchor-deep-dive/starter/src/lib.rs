#![allow(unused_variables)]
//! Starter code for guide 02 (Anchor Deep-Dive).
//! Replace every `todo!()` in this file.
//! The full test suite is in `tests/exercises.rs` (it includes `../shared/exercises.rs`).
//! Run `cargo test -p starter` from the workspace root, or `cargo test` from `starter/`.
//! Each exercise is a check an Anchor macro writes for you; here you write it
//! by hand. Model types: `../shared/model.rs`, hashing: `../shared/pda_base.rs`.
//! Stuck? Reference answers are in `../solutions`.

include!("../../shared/model.rs");
include!("../../shared/pda_base.rs");

// ---------------------------------------------------------------- Level 1

/// 1.1 Discriminators: the first 8 bytes of `sha256("<namespace>:<name>")`.
/// Anchor uses namespace `global` for instructions and `account` for
/// account types, e.g. `sighash("global", "initialize")`.
pub fn sighash(namespace: &str, name: &str) -> [u8; 8] {
    todo!("exercise: sighash")
}

/// Field types for `#[derive(InitSpace)]`-style size calculation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Field {
    Bool,
    U8,
    U16,
    U32,
    U64,
    I64,
    U128,
    Pubkey,
    /// `#[max_len(n)] String`: n bytes of UTF-8.
    String {
        max_len: usize,
    },
    /// `#[max_len(n)] Vec<T>`.
    Vec {
        elem: Box<Field>,
        max_len: usize,
    },
    Option(Box<Field>),
    Array {
        elem: Box<Field>,
        len: usize,
    },
}

/// 1.2 Space: Borsh size of one field at its maximum. Strings and Vecs carry
/// a 4-byte length prefix, Options a 1-byte tag, arrays no prefix.
pub fn field_space(field: &Field) -> usize {
    todo!("exercise: field_space")
}

/// 1.2 (cont.) The `space = ...` to pass to `init`: 8 discriminator bytes
/// plus every field.
pub fn account_space(fields: &[Field]) -> usize {
    todo!("exercise: account_space")
}

// ---------------------------------------------------------------- Level 2

/// An `#[account]` type: Borsh data behind an 8-byte discriminator.
pub trait AnchorAccount: BorshSerialize + BorshDeserialize {
    const NAME: &'static str;
    fn discriminator() -> [u8; 8] {
        sighash("account", Self::NAME)
    }
}

/// Helper (given): the bytes Anchor writes for an `#[account]`, padded with
/// zeroes to `space`.
pub fn account_bytes<T: AnchorAccount>(value: &T, space: usize) -> Vec<u8> {
    let mut out = T::discriminator().to_vec();
    value.serialize(&mut out).expect("Vec writer cannot fail");
    assert!(out.len() <= space, "space too small for value");
    out.resize(space, 0);
    out
}

/// 2.1 What `Account<'info, T>` checks when it loads, in Anchor's order:
/// owner == `program_id`, at least 8 bytes, discriminator matches, Borsh
/// decodes. Trailing bytes are fine (accounts are often over-allocated).
pub fn load_account<T: AnchorAccount>(
    program_id: &Pubkey,
    info: &AccountInfo,
) -> Result<T, AnchorError> {
    todo!("exercise: load_account")
}

/// 2.2 `Pubkey::create_program_address`: sha256(seeds.. || [bump] ||
/// program_id || "ProgramDerivedAddress"); reject on-curve results
/// (`InvalidSeeds`) and seeds over the limits (`MaxSeedLengthExceeded`).
pub fn create_program_address(
    seeds: &[&[u8]],
    bump: u8,
    program_id: &Pubkey,
) -> Result<Pubkey, AnchorError> {
    todo!("exercise: create_program_address")
}

/// 2.2 (cont.) `Pubkey::find_program_address`: try bumps 255, 254, ... 0 and
/// return the first off-curve address with its (canonical) bump.
pub fn find_program_address(
    seeds: &[&[u8]],
    program_id: &Pubkey,
) -> Result<(Pubkey, u8), AnchorError> {
    todo!("exercise: find_program_address")
}

/// 2.3 The `seeds = [...], bump` constraint: `key` must be the canonical
/// PDA for `seeds`. Returns the bump to store. Any other key, including a
/// valid PDA with a non-canonical bump, is `ConstraintSeeds`.
pub fn check_seeds(seeds: &[&[u8]], key: &Pubkey, program_id: &Pubkey) -> Result<u8, AnchorError> {
    todo!("exercise: check_seeds")
}

// ---------------------------------------------------------------- Level 3

pub const MAX_FEE_BPS: u16 = 1_000;
pub const CONFIG_SEED: &[u8] = b"config";

/// `#[account] #[derive(InitSpace)] pub struct Config { admin, fee_bps, bump }`
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Config {
    pub admin: Pubkey,
    pub fee_bps: u16,
    pub bump: u8,
}
impl AnchorAccount for Config {
    const NAME: &'static str = "Config";
}
pub const CONFIG_SPACE: usize = 8 + 32 + 2 + 1;

/// 3.1 `initialize_config`, written out by hand. The Accounts struct would be:
///
/// ```text
/// #[account(init, payer = admin, space = CONFIG_SPACE, seeds = [b"config"], bump)]
/// pub config: Account<'info, Config>,
/// #[account(mut)] pub admin: Signer<'info>,
/// ```
///
/// Checks, in order: admin signed; `config` is the canonical PDA; `config`
/// is not already in use (non-empty data or not owned by the System
/// Program); `fee_bps <= MAX_FEE_BPS`. Then assign `config` to the program
/// and write the account. Nothing changes on error.
pub fn initialize_config(
    program_id: &Pubkey,
    config: &mut AccountInfo,
    admin: &AccountInfo,
    fee_bps: u16,
) -> Result<(), AnchorError> {
    todo!("exercise: initialize_config")
}

/// 3.2 `set_fee` with
/// `#[account(mut, has_one = admin, seeds = [b"config"], bump = config.bump)]`
/// and `admin: Signer`. Load the config (2.1), check the signer, `has_one`,
/// the seeds with the stored bump, and the fee cap. Then write.
pub fn set_fee(
    program_id: &Pubkey,
    config: &mut AccountInfo,
    admin: &AccountInfo,
    new_fee_bps: u16,
) -> Result<(), AnchorError> {
    todo!("exercise: set_fee")
}

/// Reward precision for `acc_reward_per_share` (MasterChef style).
pub const PRECISION: u128 = 1_000_000_000_000;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Pool {
    pub total_staked: u64,
    /// Rewards per staked token, scaled by `PRECISION`.
    pub acc_reward_per_share: u128,
    /// Rewards that arrived while nothing was staked; paid to nobody.
    pub undistributed: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserStake {
    pub owner: Pubkey,
    pub amount: u64,
    /// `amount * acc_reward_per_share / PRECISION` at the last update,
    /// rounded UP (against the user).
    pub reward_debt: u128,
}

/// 3.3 Pool accounting. `distribute`: add `rewards` to every staker
/// pro-rata (if nobody is staked, park them in `undistributed`).
pub fn distribute(pool: &mut Pool, rewards: u64) -> Result<(), AnchorError> {
    todo!("exercise: distribute")
}

/// 3.3 (cont.) Rewards `user` can claim right now: accrued rounded DOWN
/// minus `reward_debt` (which was rounded UP), floored at zero.
///
/// Why both roundings? The textbook MasterChef rounds both down, and the
/// property test `prop_pool_is_solvent` finds a sequence where the pool pays
/// out 1 unit more than it ever received. Rounding always goes against the
/// user (guide 00, section on rounding).
pub fn pending(pool: &Pool, user: &UserStake) -> Result<u64, AnchorError> {
    todo!("exercise: pending")
}

/// Helper (given): `amount * acc / PRECISION`, rounded UP.
pub fn reward_debt_for(amount: u64, acc: u128) -> Result<u128, AnchorError> {
    let x = (amount as u128)
        .checked_mul(acc)
        .ok_or(AnchorError::MathOverflow)?;
    Ok(x.div_ceil(PRECISION))
}

/// 3.3 (cont.) `stake`: `signer` must sign and be `user.owner` (`has_one =
/// owner`). Pays out pending rewards first, then adds `amount`. Returns
/// the rewards paid. Nothing changes on error.
pub fn stake(
    pool: &mut Pool,
    user: &mut UserStake,
    signer: &AccountInfo,
    amount: u64,
) -> Result<u64, AnchorError> {
    todo!("exercise: stake")
}

/// 3.3 (cont.) `unstake`: same signer rules; `InsufficientStake` if
/// `amount > user.amount`. Pays pending rewards, then removes `amount`.
/// Returns `(rewards_paid, amount)`.
pub fn unstake(
    pool: &mut Pool,
    user: &mut UserStake,
    signer: &AccountInfo,
    amount: u64,
) -> Result<(u64, u64), AnchorError> {
    todo!("exercise: unstake")
}
