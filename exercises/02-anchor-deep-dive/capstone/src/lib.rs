#![allow(unused_variables)]
//! CAPSTONE 02: audit the airdrop distributor (guide 02: Anchor constraints).
//!
//! The doc comments show the Accounts structs the author *meant* to write;
//! the code is what actually runs. It compiles, the honest tests pass, and
//! one wallet can take far more than its share.
//! `cargo test -p capstone` runs `exploit_*` tests that play the attacker
//! locally; each fails with "EXPLOIT SUCCEEDED" until you fix the bug.
//!
//! TODO(you):
//! 1. For every account, list the constraints the intended Accounts struct
//!    implies (signer, has_one, seeds, bump, init) and find the code for each.
//! 2. Write down the missing sentence for each bug (there are two).
//! 3. Patch this file until `cargo test -p capstone` is all green.
//! 4. Compare with `../capstone-solution`.
//!
//! Local toy only: no real addresses, no network, nothing to deploy.

include!("../../shared/model.rs");
include!("../../shared/pda_base.rs");
include!("../../shared/pda.rs");

pub const RECEIPT_SEED: &[u8] = b"receipt";

/// An airdrop distributor. Already loaded and owner-checked (think
/// `Account<'info, Distributor>`), so these fields can be trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Distributor {
    pub admin: Pubkey,
    pub per_claim: u64,
    pub remaining: u64,
}

/// `claim(bump)`: each wallet may claim `per_claim` once. A receipt PDA at
/// `[b"receipt", claimant]` is created to remember the claim.
///
/// Intended Accounts struct:
/// ```text
/// #[account(mut)] pub distributor: Account<'info, Distributor>,
/// #[account(init, payer = claimant, space = 8 + 2,
///           seeds = [b"receipt", claimant.key().as_ref()], bump)]
/// pub receipt: Account<'info, Receipt>,
/// #[account(mut)] pub claimant: Signer<'info>,
/// ```
/// `payout` is the claimant's token balance (credited on success).
pub fn claim(
    program_id: &Pubkey,
    dist: &mut Distributor,
    receipt: &mut AccountInfo,
    claimant: &AccountInfo,
    bump: u8,
    payout: &mut u64,
) -> Result<(), AnchorError> {
    if !claimant.is_signer {
        return Err(AnchorError::ConstraintSigner);
    }
    let seeds: &[&[u8]] = &[RECEIPT_SEED, &claimant.key.0];
    let expected = create_program_address(seeds, bump, program_id)
        .map_err(|_| AnchorError::ConstraintSeeds)?;
    if expected != receipt.key {
        return Err(AnchorError::ConstraintSeeds);
    }
    if receipt.owner != SYSTEM_PROGRAM_ID || !receipt.data.is_empty() {
        return Err(AnchorError::AccountAlreadyInUse);
    }
    let remaining = dist
        .remaining
        .checked_sub(dist.per_claim)
        .ok_or(AnchorError::MathOverflow)?;
    let new_payout = payout
        .checked_add(dist.per_claim)
        .ok_or(AnchorError::MathOverflow)?;
    dist.remaining = remaining;
    *payout = new_payout;
    receipt.owner = *program_id;
    receipt.data = vec![1, bump];
    Ok(())
}

/// `withdraw_unclaimed()`: the admin sweeps what is left after the airdrop.
///
/// Intended Accounts struct:
/// ```text
/// #[account(mut, has_one = admin)] pub distributor: Account<'info, Distributor>,
/// pub admin: Signer<'info>,
/// ```
pub fn withdraw_unclaimed(
    dist: &mut Distributor,
    admin: &AccountInfo,
    payout: &mut u64,
) -> Result<u64, AnchorError> {
    if !admin.is_signer {
        return Err(AnchorError::ConstraintSigner);
    }
    let amount = dist.remaining;
    *payout = payout
        .checked_add(amount)
        .ok_or(AnchorError::MathOverflow)?;
    dist.remaining = 0;
    Ok(amount)
}
