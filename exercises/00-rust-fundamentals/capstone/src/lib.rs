//! CAPSTONE 00: audit the points ledger (guide 00: the Rust security lens).
//!
//! No Solana here, just Rust. It compiles, the honest tests pass, and a user
//! with a tiny balance can print points out of thin air.
//! `cargo test -p capstone` runs `exploit_*` tests that play the attacker
//! locally; each fails with "EXPLOIT SUCCEEDED" until you fix the bug.
//!
//! TODO(you):
//! 1. Go through guide 00 section 13 (overflow, truncating casts, rounding,
//!    panics) line by line against this file.
//! 2. Write down the missing sentence for each bug (there are two).
//! 3. Patch this file until `cargo test -p capstone` is all green.
//! 4. Compare with `../capstone-solution`.
//!
//! Local toy only: nothing to deploy.

use std::collections::BTreeMap;
use thiserror::Error;

pub type UserId = u32;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LedgerError {
    #[error("insufficient balance")]
    InsufficientBalance,
    #[error("arithmetic overflow")]
    Overflow,
    #[error("empty batch")]
    EmptyBatch,
}

/// A points ledger with a flat transfer fee paid to the treasury.
/// Invariant: the sum of all balances always equals `total_supply`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ledger {
    pub balances: BTreeMap<UserId, u64>,
    pub total_supply: u64,
    pub fee: u64,
}

pub const TREASURY: UserId = 0;

impl Ledger {
    pub fn balance(&self, user: UserId) -> u64 {
        self.balances.get(&user).copied().unwrap_or(0)
    }

    pub fn mint(&mut self, user: UserId, amount: u64) -> Result<(), LedgerError> {
        let supply = self
            .total_supply
            .checked_add(amount)
            .ok_or(LedgerError::Overflow)?;
        let bal = self
            .balance(user)
            .checked_add(amount)
            .ok_or(LedgerError::Overflow)?;
        self.total_supply = supply;
        self.balances.insert(user, bal);
        Ok(())
    }

    /// Move `amount` from `from` to `to`; `from` also pays `self.fee` to the
    /// treasury. Self-transfers are allowed (they just pay the fee).
    pub fn transfer(&mut self, from: UserId, to: UserId, amount: u64) -> Result<(), LedgerError> {
        let debit = amount.wrapping_add(self.fee);
        let from_bal = self.balance(from);
        if debit > from_bal {
            return Err(LedgerError::InsufficientBalance);
        }
        self.balances.insert(from, from_bal - debit);
        let to_bal = self
            .balance(to)
            .checked_add(amount)
            .ok_or(LedgerError::Overflow)?;
        self.balances.insert(to, to_bal);
        let t = self
            .balance(TREASURY)
            .checked_add(self.fee)
            .ok_or(LedgerError::Overflow)?;
        self.balances.insert(TREASURY, t);
        Ok(())
    }

    /// Pay many recipients from `from` in one go (no fee).
    pub fn batch_payout(
        &mut self,
        from: UserId,
        payouts: &[(UserId, u64)],
    ) -> Result<(), LedgerError> {
        if payouts.is_empty() {
            return Err(LedgerError::EmptyBatch);
        }
        let total: u128 = payouts.iter().map(|&(_, a)| a as u128).sum();
        let total = total as u64;
        let from_bal = self.balance(from);
        if total > from_bal {
            return Err(LedgerError::InsufficientBalance);
        }
        // Validate every credit before writing anything.
        let mut next = self.balances.clone();
        next.insert(from, from_bal - total);
        for &(to, a) in payouts {
            let b = next
                .get(&to)
                .copied()
                .unwrap_or(0)
                .checked_add(a)
                .ok_or(LedgerError::Overflow)?;
            next.insert(to, b);
        }
        self.balances = next;
        Ok(())
    }

    /// Sum of all balances (u128 so the check itself cannot overflow).
    pub fn sum_of_balances(&self) -> u128 {
        self.balances.values().map(|&b| b as u128).sum()
    }
}
