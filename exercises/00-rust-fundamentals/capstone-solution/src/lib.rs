//! Capstone 00, patched. Compare with `../capstone/src/lib.rs`.
//! Each `// FIX` block replaces the `// WAS:` line under it.

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
        // FIX 1 (wrapping arithmetic): `wrapping_add` turns a huge amount
        // into a tiny debit. Overflow must be an error, never a wrap.
        let debit = amount.checked_add(self.fee).ok_or(LedgerError::Overflow)?;
        // END FIX
        // WAS: let debit = amount.wrapping_add(self.fee);
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
        // FIX 2 (truncating cast): `as u64` silently drops the high bits of
        // a u128, so a batch summing to 2^64 + 10 would debit only 10.
        let total = u64::try_from(total).map_err(|_| LedgerError::Overflow)?;
        // END FIX
        // WAS: let total = total as u64;
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
