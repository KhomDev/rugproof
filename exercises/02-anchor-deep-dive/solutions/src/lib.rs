//! Reference solutions for guide 02 (Anchor Deep-Dive).
//! Each exercise re-implements, in plain Rust, a check that an Anchor macro
//! generates for you, so you can see exactly what `Account<'info, T>`,
//! `has_one`, `seeds`/`bump`, `Signer` and `InitSpace` really do.

mod model;
pub use model::*;
mod pda_base;
pub use pda_base::*;

// ---------------------------------------------------------------- Level 1

/// 1.1 Discriminators: the first 8 bytes of `sha256("<namespace>:<name>")`.
/// Anchor uses namespace `global` for instructions and `account` for
/// account types, e.g. `sighash("global", "initialize")`.
pub fn sighash(namespace: &str, name: &str) -> [u8; 8] {
    let h = sha256(&[namespace.as_bytes(), b":", name.as_bytes()]);
    let mut out = [0u8; 8];
    out.copy_from_slice(&h[..8]);
    out
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
    match field {
        Field::Bool | Field::U8 => 1,
        Field::U16 => 2,
        Field::U32 => 4,
        Field::U64 | Field::I64 => 8,
        Field::U128 => 16,
        Field::Pubkey => 32,
        Field::String { max_len } => 4 + max_len,
        Field::Vec { elem, max_len } => 4 + field_space(elem) * max_len,
        Field::Option(inner) => 1 + field_space(inner),
        Field::Array { elem, len } => field_space(elem) * len,
    }
}

/// 1.2 (cont.) The `space = ...` to pass to `init`: 8 discriminator bytes
/// plus every field.
pub fn account_space(fields: &[Field]) -> usize {
    8 + fields.iter().map(field_space).sum::<usize>()
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
    if info.owner != *program_id {
        return Err(AnchorError::AccountOwnedByWrongProgram);
    }
    if info.data.len() < 8 {
        return Err(AnchorError::AccountDiscriminatorNotFound);
    }
    if info.data[..8] != T::discriminator() {
        return Err(AnchorError::AccountDiscriminatorMismatch);
    }
    T::deserialize(&mut &info.data[8..]).map_err(|_| AnchorError::AccountDidNotDeserialize)
}

/// 2.2 `Pubkey::create_program_address`: sha256(seeds.. || [bump] ||
/// program_id || "ProgramDerivedAddress"); reject on-curve results
/// (`InvalidSeeds`) and seeds over the limits (`MaxSeedLengthExceeded`).
pub fn create_program_address(
    seeds: &[&[u8]],
    bump: u8,
    program_id: &Pubkey,
) -> Result<Pubkey, AnchorError> {
    if seeds.len() >= MAX_SEEDS || seeds.iter().any(|s| s.len() > MAX_SEED_LEN) {
        return Err(AnchorError::MaxSeedLengthExceeded);
    }
    let bump = [bump];
    let mut parts: Vec<&[u8]> = seeds.to_vec();
    parts.push(&bump);
    parts.push(&program_id.0);
    parts.push(PDA_MARKER);
    let h = sha256(&parts);
    if is_on_curve(&h) {
        return Err(AnchorError::InvalidSeeds);
    }
    Ok(Pubkey(h))
}

/// 2.2 (cont.) `Pubkey::find_program_address`: try bumps 255, 254, ... 0 and
/// return the first off-curve address with its (canonical) bump.
pub fn find_program_address(
    seeds: &[&[u8]],
    program_id: &Pubkey,
) -> Result<(Pubkey, u8), AnchorError> {
    for bump in (0..=u8::MAX).rev() {
        match create_program_address(seeds, bump, program_id) {
            Ok(k) => return Ok((k, bump)),
            Err(AnchorError::InvalidSeeds) => continue,
            Err(e) => return Err(e),
        }
    }
    Err(AnchorError::InvalidSeeds)
}

/// 2.3 The `seeds = [...], bump` constraint: `key` must be the canonical
/// PDA for `seeds`. Returns the bump to store. Any other key, including a
/// valid PDA with a non-canonical bump, is `ConstraintSeeds`.
pub fn check_seeds(seeds: &[&[u8]], key: &Pubkey, program_id: &Pubkey) -> Result<u8, AnchorError> {
    let (expected, bump) =
        find_program_address(seeds, program_id).map_err(|_| AnchorError::ConstraintSeeds)?;
    if expected != *key {
        return Err(AnchorError::ConstraintSeeds);
    }
    Ok(bump)
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
    if !admin.is_signer {
        return Err(AnchorError::ConstraintSigner);
    }
    let bump = check_seeds(&[CONFIG_SEED], &config.key, program_id)?;
    if config.owner != SYSTEM_PROGRAM_ID || config.data.iter().any(|&b| b != 0) {
        return Err(AnchorError::AccountAlreadyInUse);
    }
    if fee_bps > MAX_FEE_BPS {
        return Err(AnchorError::FeeTooHigh);
    }
    config.owner = *program_id;
    config.data = account_bytes(
        &Config {
            admin: admin.key,
            fee_bps,
            bump,
        },
        CONFIG_SPACE,
    );
    Ok(())
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
    let mut c: Config = load_account(program_id, config)?;
    if !admin.is_signer {
        return Err(AnchorError::ConstraintSigner);
    }
    if c.admin != admin.key {
        return Err(AnchorError::ConstraintHasOne);
    }
    if create_program_address(&[CONFIG_SEED], c.bump, program_id) != Ok(config.key) {
        return Err(AnchorError::ConstraintSeeds);
    }
    if new_fee_bps > MAX_FEE_BPS {
        return Err(AnchorError::FeeTooHigh);
    }
    c.fee_bps = new_fee_bps;
    config.data = account_bytes(&c, CONFIG_SPACE);
    Ok(())
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
    if pool.total_staked == 0 {
        pool.undistributed = pool
            .undistributed
            .checked_add(rewards)
            .ok_or(AnchorError::MathOverflow)?;
        return Ok(());
    }
    let inc = (rewards as u128)
        .checked_mul(PRECISION)
        .ok_or(AnchorError::MathOverflow)?
        / pool.total_staked as u128;
    pool.acc_reward_per_share = pool
        .acc_reward_per_share
        .checked_add(inc)
        .ok_or(AnchorError::MathOverflow)?;
    Ok(())
}

/// 3.3 (cont.) Rewards `user` can claim right now: accrued rounded DOWN
/// minus `reward_debt` (which was rounded UP), floored at zero.
///
/// Why both roundings? The textbook MasterChef rounds both down, and the
/// property test `prop_pool_is_solvent` finds a sequence where the pool pays
/// out 1 unit more than it ever received. Rounding always goes against the
/// user (guide 00, section on rounding).
pub fn pending(pool: &Pool, user: &UserStake) -> Result<u64, AnchorError> {
    let accrued = (user.amount as u128)
        .checked_mul(pool.acc_reward_per_share)
        .ok_or(AnchorError::MathOverflow)?
        / PRECISION;
    let p = accrued.saturating_sub(user.reward_debt);
    u64::try_from(p).map_err(|_| AnchorError::MathOverflow)
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
    if !signer.is_signer {
        return Err(AnchorError::ConstraintSigner);
    }
    if signer.key != user.owner {
        return Err(AnchorError::ConstraintHasOne);
    }
    let paid = pending(pool, user)?;
    let new_amount = user
        .amount
        .checked_add(amount)
        .ok_or(AnchorError::MathOverflow)?;
    let new_total = pool
        .total_staked
        .checked_add(amount)
        .ok_or(AnchorError::MathOverflow)?;
    let new_debt = reward_debt_for(new_amount, pool.acc_reward_per_share)?;
    user.amount = new_amount;
    user.reward_debt = new_debt;
    pool.total_staked = new_total;
    Ok(paid)
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
    if !signer.is_signer {
        return Err(AnchorError::ConstraintSigner);
    }
    if signer.key != user.owner {
        return Err(AnchorError::ConstraintHasOne);
    }
    if amount > user.amount {
        return Err(AnchorError::InsufficientStake);
    }
    let paid = pending(pool, user)?;
    let new_amount = user.amount - amount;
    let new_total = pool
        .total_staked
        .checked_sub(amount)
        .ok_or(AnchorError::MathOverflow)?;
    let new_debt = reward_debt_for(new_amount, pool.acc_reward_per_share)?;
    user.amount = new_amount;
    user.reward_debt = new_debt;
    pool.total_staked = new_total;
    Ok((paid, amount))
}
