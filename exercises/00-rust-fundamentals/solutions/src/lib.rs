//! Reference solutions for guide 00 (Rust Fundamentals).
//! Try `starter/` first: `cargo test -p starter`.

use borsh::{BorshDeserialize, BorshSerialize};
use std::cell::RefCell;
use thiserror::Error;

/// A 32-byte address, like Solana's `Pubkey`.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, BorshSerialize, BorshDeserialize,
)]
pub struct Pubkey(pub [u8; 32]);

impl Pubkey {
    pub const fn new_from_byte(b: u8) -> Self {
        Pubkey([b; 32])
    }
}

/// A toy account: who controls it and how many lamports it holds.
#[derive(Clone, Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Account {
    pub owner: Pubkey,
    pub lamports: u64,
}

/// Errors for every exercise (exercise 1.3).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VaultError {
    #[error("insufficient funds: have {have}, need {need}")]
    InsufficientFunds { have: u64, need: u64 },
    #[error("arithmetic overflow")]
    Overflow,
    #[error("invalid instruction data")]
    InvalidInstruction,
    #[error("account discriminator mismatch")]
    WrongAccountType,
    #[error("the same account was passed twice")]
    DuplicateAccount,
}

// ---------------------------------------------------------------- Level 1

/// 1.1 Lifetimes: return the longer of two names (the first one on a tie).
pub fn longest<'a>(a: &'a str, b: &'a str) -> &'a str {
    if b.len() > a.len() {
        b
    } else {
        a
    }
}

/// 1.2 Overflow: total lamports, or `None` if the sum overflows `u64`.
pub fn total_lamports(accounts: &[Account]) -> Option<u64> {
    accounts
        .iter()
        .try_fold(0u64, |acc, a| acc.checked_add(a.lamports))
}

/// 1.3 Borrowing + errors: move `amount` lamports between two accounts.
/// Must not change either account if it fails.
pub fn transfer(from: &mut Account, to: &mut Account, amount: u64) -> Result<(), VaultError> {
    let new_from = from
        .lamports
        .checked_sub(amount)
        .ok_or(VaultError::InsufficientFunds {
            have: from.lamports,
            need: amount,
        })?;
    let new_to = to
        .lamports
        .checked_add(amount)
        .ok_or(VaultError::Overflow)?;
    from.lamports = new_from;
    to.lamports = new_to;
    Ok(())
}

// ---------------------------------------------------------------- Level 2

/// Instructions for a tiny vault program.
#[derive(Debug, PartialEq, Eq)]
pub enum VaultInstruction {
    Initialize,
    Deposit { amount: u64 },
    Withdraw { amount: u64 },
}

/// 2.1 Enums and matching: decode `[tag, ...payload]`.
/// tag 0 = Initialize (no payload), 1 = Deposit, 2 = Withdraw (8-byte little-endian u64).
/// Any trailing or missing bytes are an error. Must never panic.
pub fn decode_instruction(data: &[u8]) -> Result<VaultInstruction, VaultError> {
    let (&tag, rest) = data.split_first().ok_or(VaultError::InvalidInstruction)?;
    let amount = || -> Result<u64, VaultError> {
        let bytes: [u8; 8] = rest
            .try_into()
            .map_err(|_| VaultError::InvalidInstruction)?;
        Ok(u64::from_le_bytes(bytes))
    };
    match tag {
        0 if rest.is_empty() => Ok(VaultInstruction::Initialize),
        1 => Ok(VaultInstruction::Deposit { amount: amount()? }),
        2 => Ok(VaultInstruction::Withdraw { amount: amount()? }),
        _ => Err(VaultError::InvalidInstruction),
    }
}

/// 2.2 Iterators: the owners of the `n` richest accounts, richest first.
/// Ties keep input order. Accounts with zero lamports are skipped.
pub fn top_holders(accounts: &[Account], n: usize) -> Vec<Pubkey> {
    let mut v: Vec<&Account> = accounts.iter().filter(|a| a.lamports > 0).collect();
    v.sort_by_key(|a| std::cmp::Reverse(a.lamports)); // stable sort keeps ties in order
    v.into_iter().take(n).map(|a| a.owner).collect()
}

/// 2.3 Borsh: the exact serialized length of `account` (predict it, then check).
pub fn borsh_len(account: &Account) -> usize {
    borsh::to_vec(account)
        .expect("Vec writer cannot fail")
        .len()
}

// ---------------------------------------------------------------- Level 3

/// Types stored in "accounts" carry an 8-byte tag so one type can't be
/// passed where another is expected (the "type cosplay" bug, guide 04).
pub trait Discriminator {
    const DISCRIMINATOR: [u8; 8];
}

#[derive(Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Vault {
    pub authority: Pubkey,
    pub balance: u64,
}
impl Discriminator for Vault {
    const DISCRIMINATOR: [u8; 8] = *b"vault___";
}

#[derive(Debug, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Config {
    pub admin: Pubkey,
    pub fee_bps: u64,
}
impl Discriminator for Config {
    const DISCRIMINATOR: [u8; 8] = *b"config__";
}

/// 3.1 Traits and generics: write `T::DISCRIMINATOR` followed by Borsh bytes.
pub fn store<T: Discriminator + BorshSerialize>(value: &T) -> Vec<u8> {
    let mut out = T::DISCRIMINATOR.to_vec();
    value.serialize(&mut out).expect("Vec writer cannot fail");
    out
}

/// 3.1 (cont.) Load a `T`, rejecting data with the wrong discriminator
/// and data with trailing bytes.
pub fn load<T: Discriminator + BorshDeserialize>(data: &[u8]) -> Result<T, VaultError> {
    if data.len() < 8 || data[..8] != T::DISCRIMINATOR {
        return Err(VaultError::WrongAccountType);
    }
    T::try_from_slice(&data[8..]).map_err(|_| VaultError::InvalidInstruction)
}

/// 3.2 Share math: shares minted for a deposit, rounded DOWN (against the user).
/// First depositor gets 1:1. `None` on overflow or division by zero.
pub fn deposit_shares(amount: u64, total_assets: u64, total_shares: u64) -> Option<u64> {
    if total_shares == 0 {
        return Some(amount);
    }
    if total_assets == 0 {
        return None;
    }
    let s = (amount as u128).checked_mul(total_shares as u128)? / total_assets as u128;
    u64::try_from(s).ok()
}

/// 3.2 (cont.) Assets paid out for burning `shares`, rounded DOWN.
pub fn withdraw_assets(shares: u64, total_assets: u64, total_shares: u64) -> Option<u64> {
    if total_shares == 0 || shares > total_shares {
        return None;
    }
    let a = (shares as u128).checked_mul(total_assets as u128)? / total_shares as u128;
    u64::try_from(a).ok()
}

/// 3.3 RefCell and duplicate accounts: move `amount` between two shared cells.
/// If both arguments are the same cell, return `DuplicateAccount`
/// instead of panicking. Leave both unchanged on any error.
pub fn move_lamports(
    from: &RefCell<u64>,
    to: &RefCell<u64>,
    amount: u64,
) -> Result<(), VaultError> {
    if std::ptr::eq(from, to) {
        return Err(VaultError::DuplicateAccount);
    }
    let mut f = from
        .try_borrow_mut()
        .map_err(|_| VaultError::DuplicateAccount)?;
    let mut t = to
        .try_borrow_mut()
        .map_err(|_| VaultError::DuplicateAccount)?;
    let nf = f.checked_sub(amount).ok_or(VaultError::InsufficientFunds {
        have: *f,
        need: amount,
    })?;
    let nt = t.checked_add(amount).ok_or(VaultError::Overflow)?;
    *f = nf;
    *t = nt;
    Ok(())
}
