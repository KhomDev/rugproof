#![allow(unused_variables)]
//! CAPSTONE 03: audit the share vault (guide 03: CPIs and token accounts).
//!
//! It compiles, the honest tests pass, and depositors can be robbed.
//! `cargo test -p capstone` runs `exploit_*` tests that play the attacker
//! locally; each fails with "EXPLOIT SUCCEEDED" until you fix the bug.
//!
//! TODO(you):
//! 1. Fill in guide 03's audit table for `deposit`: for each account, its
//!    role, who controls it, and the line that proves it (use exercise 3.3).
//! 2. Write down the missing sentence for each bug (there are two).
//! 3. Patch this file until `cargo test -p capstone` is all green.
//! 4. Compare with `../capstone-solution`.
//!
//! Local toy only: no real addresses, no network, nothing to deploy.

mod model;
pub use model::*;

use std::collections::BTreeMap;

/// A program that can be the target of a token-transfer CPI.
pub trait TokenProgram {
    fn transfer(
        &self,
        from: &mut TokenAccount,
        to: &mut TokenAccount,
        authority: &Pubkey,
        amount: u64,
    ) -> Result<(), SecurityError>;
}

/// The real Token program's transfer rules (owner-signed, same mint, funds).
pub struct SplToken;
impl TokenProgram for SplToken {
    fn transfer(
        &self,
        from: &mut TokenAccount,
        to: &mut TokenAccount,
        authority: &Pubkey,
        amount: u64,
    ) -> Result<(), SecurityError> {
        if from.mint != to.mint {
            return Err(SecurityError::MintMismatch);
        }
        if from.owner != *authority {
            return Err(SecurityError::OwnerOrDelegateRequired);
        }
        let f = from
            .amount
            .checked_sub(amount)
            .ok_or(SecurityError::InsufficientFunds)?;
        let t = to
            .amount
            .checked_add(amount)
            .ok_or(SecurityError::Overflow)?;
        from.amount = f;
        to.amount = t;
        Ok(())
    }
}

/// The deployed programs a CPI can reach, by program id. An attacker can
/// deploy anything, so tests register their own programs here too.
#[derive(Default)]
pub struct Programs {
    map: BTreeMap<Pubkey, Box<dyn TokenProgram>>,
}
impl Programs {
    pub fn with_spl_token() -> Self {
        let mut p = Programs::default();
        p.deploy(TOKEN_PROGRAM_ID, Box::new(SplToken));
        p
    }
    pub fn deploy(&mut self, id: Pubkey, program: Box<dyn TokenProgram>) {
        self.map.insert(id, program);
    }
    /// `invoke(&transfer_ix, ...)`: run whatever program lives at `id`.
    pub fn invoke_transfer(
        &self,
        id: &Pubkey,
        from: &mut TokenAccount,
        to: &mut TokenAccount,
        authority: &Pubkey,
        amount: u64,
    ) -> Result<(), SecurityError> {
        self.map
            .get(id)
            .ok_or(SecurityError::ArbitraryCpi)?
            .transfer(from, to, authority, amount)
    }
}

/// A token account as passed to the instruction: its address plus its state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenAccountInfo {
    pub key: Pubkey,
    pub state: TokenAccount,
}

/// The PDA that owns the vault's token account (signs withdrawals).
pub const VAULT_AUTHORITY: Pubkey = Pubkey::new_from_byte(0xA1);

/// A 1:1 share vault. Loaded and owner-checked already: trusted.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Vault {
    pub mint: Pubkey,
    /// The one token account that holds deposits (owned by VAULT_AUTHORITY).
    pub token_account: Pubkey,
    pub total_shares: u64,
    pub shares: BTreeMap<Pubkey, u64>,
}

/// `deposit(amount)`: move `amount` tokens from the user's token account
/// into the vault's token account via a Token-program CPI, then credit
/// `amount` shares.
/// Accounts: `user` (signer), `user_token`, `vault_token`, `token_program`.
pub fn deposit(
    programs: &Programs,
    vault: &mut Vault,
    user: &AccountInfo,
    user_token: &mut TokenAccountInfo,
    vault_token: &mut TokenAccountInfo,
    token_program: &Pubkey,
    amount: u64,
) -> Result<(), SecurityError> {
    if !user.is_signer {
        return Err(SecurityError::MissingRequiredSignature);
    }
    if vault_token.state.mint != vault.mint {
        return Err(SecurityError::MintMismatch);
    }
    let new_total = vault
        .total_shares
        .checked_add(amount)
        .ok_or(SecurityError::Overflow)?;
    programs.invoke_transfer(
        token_program,
        &mut user_token.state,
        &mut vault_token.state,
        &user.key,
        amount,
    )?;
    *vault.shares.entry(user.key).or_default() += amount;
    vault.total_shares = new_total;
    Ok(())
}

/// `withdraw(amount)`: burn `amount` shares and send `amount` tokens from
/// the vault's token account (VAULT_AUTHORITY signs as PDA) to the user's.
pub fn withdraw(
    programs: &Programs,
    vault: &mut Vault,
    user: &AccountInfo,
    vault_token: &mut TokenAccountInfo,
    user_token: &mut TokenAccountInfo,
    token_program: &Pubkey,
    amount: u64,
) -> Result<(), SecurityError> {
    if !user.is_signer {
        return Err(SecurityError::MissingRequiredSignature);
    }
    if *token_program != TOKEN_PROGRAM_ID {
        return Err(SecurityError::ArbitraryCpi);
    }
    if vault_token.key != vault.token_account {
        return Err(SecurityError::OwnerMismatch);
    }
    let have = vault.shares.get(&user.key).copied().unwrap_or(0);
    let left = have
        .checked_sub(amount)
        .ok_or(SecurityError::InsufficientFunds)?;
    programs.invoke_transfer(
        token_program,
        &mut vault_token.state,
        &mut user_token.state,
        &VAULT_AUTHORITY,
        amount,
    )?;
    vault.shares.insert(user.key, left);
    vault.total_shares -= amount;
    Ok(())
}
