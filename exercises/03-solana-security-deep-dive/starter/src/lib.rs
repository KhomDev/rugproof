#![allow(unused_variables)]
//! Starter code for guide 03 (Solana Security Deep-Dive).
//! Replace every `todo!()` in this file.
//! The full test suite is in `tests/exercises.rs`.
//! Run `cargo test -p starter` from the workspace root, or `cargo test` from `starter/`.
//! Model types (accounts, SPL Token state, errors): `src/model.rs`.
//! Stuck? Reference answers are in `../solutions`.

mod model;
pub use model::*;

// ---------------------------------------------------------------- Level 1

/// 1.1 Owner check: `IllegalOwner` unless `info.owner == expected`.
pub fn check_owner(info: &AccountInfo, expected: &Pubkey) -> Result<(), SecurityError> {
    todo!("exercise: check_owner")
}

/// 1.2 Duplicate mutable accounts: `DuplicateAccount` if any key appears
/// twice (e.g. `from == to` in a swap, which can double-count balances).
pub fn check_distinct(keys: &[Pubkey]) -> Result<(), SecurityError> {
    todo!("exercise: check_distinct")
}

/// 1.3 Fees round in the protocol's favour: `ceil(amount * bps / 10_000)`.
/// `bps > 10_000` is `InvalidArgument`. Never overflows (use u128).
pub fn fee_amount(amount: u64, bps: u16) -> Result<u64, SecurityError> {
    todo!("exercise: fee_amount")
}

// ---------------------------------------------------------------- Level 2

/// 2.1 Validate a token account the instruction is about to trust:
/// right mint (`MintMismatch`), right owner (`OwnerMismatch`), not frozen.
pub fn validate_token_account(
    acc: &TokenAccount,
    expected_mint: &Pubkey,
    expected_owner: &Pubkey,
) -> Result<(), SecurityError> {
    todo!("exercise: validate_token_account")
}

/// 2.2 `TransferChecked`, as the Token program does it:
/// both accounts on `mint_key` and `decimals == mint.decimals`; neither
/// frozen; `authority` signed and is the owner, or the delegate with enough
/// `delegated_amount` (which is then reduced); enough balance. Nothing
/// changes on error.
#[allow(clippy::too_many_arguments)]
pub fn transfer_checked(
    from: &mut TokenAccount,
    to: &mut TokenAccount,
    authority: &AccountInfo,
    mint_key: &Pubkey,
    mint: &Mint,
    amount: u64,
    decimals: u8,
) -> Result<(), SecurityError> {
    todo!("exercise: transfer_checked")
}

/// 2.3 Arbitrary CPI: before invoking `program`, prove it is one of
/// `allowed` and executable. Otherwise `ArbitraryCpi`.
pub fn check_cpi_program(program: &AccountInfo, allowed: &[Pubkey]) -> Result<(), SecurityError> {
    todo!("exercise: check_cpi_program")
}

// ---------------------------------------------------------------- Level 3

/// 3.1 Sysvars by key, never by contents (the fake-sysvar bug class):
/// `InvalidSysvar` unless `info.key == expected`.
pub fn check_sysvar(info: &AccountInfo, expected: &Pubkey) -> Result<(), SecurityError> {
    todo!("exercise: check_sysvar")
}

/// Discriminator written by old Anchor versions into closed accounts.
pub const CLOSED_ACCOUNT_DISCRIMINATOR: [u8; 8] = [255; 8];
pub const VAULT_TAG: [u8; 8] = *b"vault___";
pub const VAULT_LEN: usize = 8 + 32;

/// 3.2 One-time initialization. `data` must be exactly `VAULT_LEN` bytes.
/// All zero: write `VAULT_TAG` + `authority`. Starts with
/// `CLOSED_ACCOUNT_DISCRIMINATOR`: `AccountClosed`. Anything else:
/// `AlreadyInitialized` (re-init would let a caller replace the authority).
pub fn initialize_vault(data: &mut [u8], authority: &Pubkey) -> Result<(), SecurityError> {
    todo!("exercise: initialize_vault")
}

/// 3.2 (cont.) Close safely: move every lamport to `dest` (not the same
/// account), zero the data and give the account back to the System Program.
pub fn close_account(acc: &mut AccountInfo, dest: &mut AccountInfo) -> Result<(), SecurityError> {
    todo!("exercise: close_account")
}

/// What an instruction handler does with one of its accounts, and which
/// checks it performs on it. 3.3 audits a list of these.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountUse {
    pub name: &'static str,
    pub role: Role,
    pub mutated: bool,
    pub checks: Checks,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// A wallet whose permission the instruction relies on.
    Authority,
    /// Program state the instruction reads and trusts.
    State,
    /// An SPL token account.
    TokenAccount,
    /// A program the instruction CPIs into.
    CpiProgram,
    /// A sysvar the instruction reads.
    Sysvar,
    /// Anything else (e.g. a destination wallet).
    Other,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Checks {
    pub signer: bool,
    pub owner: bool,
    pub key: bool,
    pub mint: bool,
    pub token_owner: bool,
    pub writable: bool,
    pub distinct: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Finding {
    MissingSigner(&'static str),
    MissingOwnerCheck(&'static str),
    MissingMintCheck(&'static str),
    MissingTokenOwnerCheck(&'static str),
    ArbitraryCpi(&'static str),
    FakeSysvar(&'static str),
    NotWritable(&'static str),
    PossibleDuplicate(&'static str, &'static str),
}

/// 3.3 Audit checklist as code. Report, sorted, every finding:
///
/// * `Authority` without `signer` -> `MissingSigner`
/// * `State` without `owner` -> `MissingOwnerCheck`
/// * `TokenAccount` without `mint` -> `MissingMintCheck`, without
///   `token_owner` -> `MissingTokenOwnerCheck`
/// * `CpiProgram` without `key` -> `ArbitraryCpi`
/// * `Sysvar` without `key` -> `FakeSysvar`
/// * `mutated` without `writable` -> `NotWritable`
/// * two `mutated` accounts with the same role where neither has `distinct`
///   -> `PossibleDuplicate(first, second)` in list order.
pub fn audit(accounts: &[AccountUse]) -> Vec<Finding> {
    todo!("exercise: audit")
}
