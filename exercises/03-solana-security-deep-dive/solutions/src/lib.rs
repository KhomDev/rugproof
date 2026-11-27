//! Reference solutions for guide 03 (Solana Security Deep-Dive).
//! Runtime and SPL Token invariants written out as code: every function is
//! one sentence from the guide's checklists that a program must make true.

mod model;
pub use model::*;

// ---------------------------------------------------------------- Level 1

/// 1.1 Owner check: `IllegalOwner` unless `info.owner == expected`.
pub fn check_owner(info: &AccountInfo, expected: &Pubkey) -> Result<(), SecurityError> {
    if info.owner == *expected {
        Ok(())
    } else {
        Err(SecurityError::IllegalOwner)
    }
}

/// 1.2 Duplicate mutable accounts: `DuplicateAccount` if any key appears
/// twice (e.g. `from == to` in a swap, which can double-count balances).
pub fn check_distinct(keys: &[Pubkey]) -> Result<(), SecurityError> {
    let mut seen = std::collections::BTreeSet::new();
    if keys.iter().all(|k| seen.insert(*k)) {
        Ok(())
    } else {
        Err(SecurityError::DuplicateAccount)
    }
}

/// 1.3 Fees round in the protocol's favour: `ceil(amount * bps / 10_000)`.
/// `bps > 10_000` is `InvalidArgument`. Never overflows (use u128).
pub fn fee_amount(amount: u64, bps: u16) -> Result<u64, SecurityError> {
    if bps > 10_000 {
        return Err(SecurityError::InvalidArgument);
    }
    let fee = (amount as u128 * bps as u128).div_ceil(10_000);
    Ok(fee as u64) // fee <= amount, so it fits
}

// ---------------------------------------------------------------- Level 2

/// 2.1 Validate a token account the instruction is about to trust:
/// right mint (`MintMismatch`), right owner (`OwnerMismatch`), not frozen.
pub fn validate_token_account(
    acc: &TokenAccount,
    expected_mint: &Pubkey,
    expected_owner: &Pubkey,
) -> Result<(), SecurityError> {
    if acc.mint != *expected_mint {
        return Err(SecurityError::MintMismatch);
    }
    if acc.owner != *expected_owner {
        return Err(SecurityError::OwnerMismatch);
    }
    if acc.is_frozen {
        return Err(SecurityError::AccountFrozen);
    }
    Ok(())
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
    if from.mint != *mint_key || to.mint != *mint_key {
        return Err(SecurityError::MintMismatch);
    }
    if decimals != mint.decimals {
        return Err(SecurityError::MintDecimalsMismatch);
    }
    if from.is_frozen || to.is_frozen {
        return Err(SecurityError::AccountFrozen);
    }
    if !authority.is_signer {
        return Err(SecurityError::MissingRequiredSignature);
    }
    let via_delegate = if authority.key == from.owner {
        false
    } else if from.delegate == Some(authority.key) {
        if from.delegated_amount < amount {
            return Err(SecurityError::InsufficientFunds);
        }
        true
    } else {
        return Err(SecurityError::OwnerOrDelegateRequired);
    };
    let new_from = from
        .amount
        .checked_sub(amount)
        .ok_or(SecurityError::InsufficientFunds)?;
    let new_to = to
        .amount
        .checked_add(amount)
        .ok_or(SecurityError::Overflow)?;
    from.amount = new_from;
    to.amount = new_to;
    if via_delegate {
        from.delegated_amount -= amount;
        if from.delegated_amount == 0 {
            from.delegate = None;
        }
    }
    Ok(())
}

/// 2.3 Arbitrary CPI: before invoking `program`, prove it is one of
/// `allowed` and executable. Otherwise `ArbitraryCpi`.
pub fn check_cpi_program(program: &AccountInfo, allowed: &[Pubkey]) -> Result<(), SecurityError> {
    if program.executable && allowed.contains(&program.key) {
        Ok(())
    } else {
        Err(SecurityError::ArbitraryCpi)
    }
}

// ---------------------------------------------------------------- Level 3

/// 3.1 Sysvars by key, never by contents (the fake-sysvar bug class):
/// `InvalidSysvar` unless `info.key == expected`.
pub fn check_sysvar(info: &AccountInfo, expected: &Pubkey) -> Result<(), SecurityError> {
    if info.key == *expected {
        Ok(())
    } else {
        Err(SecurityError::InvalidSysvar)
    }
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
    if data.len() != VAULT_LEN {
        return Err(SecurityError::InvalidArgument);
    }
    if data[..8] == CLOSED_ACCOUNT_DISCRIMINATOR {
        return Err(SecurityError::AccountClosed);
    }
    if data.iter().any(|&b| b != 0) {
        return Err(SecurityError::AlreadyInitialized);
    }
    data[..8].copy_from_slice(&VAULT_TAG);
    data[8..].copy_from_slice(&authority.0);
    Ok(())
}

/// 3.2 (cont.) Close safely: move every lamport to `dest` (not the same
/// account), zero the data and give the account back to the System Program.
pub fn close_account(acc: &mut AccountInfo, dest: &mut AccountInfo) -> Result<(), SecurityError> {
    if acc.key == dest.key {
        return Err(SecurityError::DuplicateAccount);
    }
    dest.lamports = dest
        .lamports
        .checked_add(acc.lamports)
        .ok_or(SecurityError::Overflow)?;
    acc.lamports = 0;
    acc.data.iter_mut().for_each(|b| *b = 0);
    acc.owner = SYSTEM_PROGRAM_ID;
    Ok(())
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
    let mut out = Vec::new();
    for a in accounts {
        let c = &a.checks;
        match a.role {
            Role::Authority if !c.signer => out.push(Finding::MissingSigner(a.name)),
            Role::State if !c.owner => out.push(Finding::MissingOwnerCheck(a.name)),
            Role::TokenAccount => {
                if !c.mint {
                    out.push(Finding::MissingMintCheck(a.name));
                }
                if !c.token_owner {
                    out.push(Finding::MissingTokenOwnerCheck(a.name));
                }
            }
            Role::CpiProgram if !c.key => out.push(Finding::ArbitraryCpi(a.name)),
            Role::Sysvar if !c.key => out.push(Finding::FakeSysvar(a.name)),
            _ => {}
        }
        if a.mutated && !c.writable {
            out.push(Finding::NotWritable(a.name));
        }
    }
    for (i, a) in accounts.iter().enumerate() {
        for b in &accounts[i + 1..] {
            if a.mutated
                && b.mutated
                && a.role == b.role
                && !a.checks.distinct
                && !b.checks.distinct
            {
                out.push(Finding::PossibleDuplicate(a.name, b.name));
            }
        }
    }
    out.sort();
    out
}
