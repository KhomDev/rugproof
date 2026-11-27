//! Capstone test suite. The `#[test]` functions live in this file. An
//! identical copy is in the other crate of the pair
//! (`capstone` / `capstone-solution`).
//!
//! From the workspace root: `cargo test -p capstone` or `cargo test -p capstone-solution`.
//! From this directory: `cargo test`.
extern crate capstone_solution as cap;
// Capstone test suite: included by both `capstone` (vulnerable, your work)
// and `capstone-solution` (patched). `cap` is the crate under test.
//
// honest_*  : normal use. Passes on both crates.
// exploit_* : the attacker's moves, run locally. Fails on the vulnerable
//             crate ("EXPLOIT SUCCEEDED") and passes once the bug is fixed.
use cap::*;
use proptest::prelude::*;

const USDC: Pubkey = Pubkey::new_from_byte(0x0C);
const ALICE: Pubkey = Pubkey::new_from_byte(1);
const MALLORY: Pubkey = Pubkey::new_from_byte(66);
const ATTACKER_PROGRAM: Pubkey = Pubkey::new_from_byte(0xEE);
const VAULT_TA: Pubkey = Pubkey::new_from_byte(0x20);
const ALICE_TA: Pubkey = Pubkey::new_from_byte(0x21);
const MALLORY_TA: Pubkey = Pubkey::new_from_byte(0x22);
const MALLORY_TA2: Pubkey = Pubkey::new_from_byte(0x23);
const ALICE_DEPOSIT: u64 = 1_000;
const MALLORY_FUNDS: u64 = 50;

/// Mallory's "token program": reports success, moves nothing.
struct FakeToken;
impl TokenProgram for FakeToken {
    fn transfer(
        &self,
        _: &mut TokenAccount,
        _: &mut TokenAccount,
        _: &Pubkey,
        _: u64,
    ) -> Result<(), SecurityError> {
        Ok(())
    }
}

fn tai(key: Pubkey, owner: Pubkey, amount: u64) -> TokenAccountInfo {
    TokenAccountInfo {
        key,
        state: TokenAccount {
            mint: USDC,
            owner,
            amount,
            delegate: None,
            delegated_amount: 0,
            is_frozen: false,
        },
    }
}

fn signer(k: Pubkey) -> AccountInfo {
    AccountInfo::new(k, SYSTEM_PROGRAM_ID, 0, 0).signer()
}

struct World {
    programs: Programs,
    vault: Vault,
    vault_ta: TokenAccountInfo,
    alice_ta: TokenAccountInfo,
    mallory_ta: TokenAccountInfo,
}

/// Alice has deposited ALICE_DEPOSIT. Mallory has MALLORY_FUNDS of her own
/// and has deployed FakeToken at ATTACKER_PROGRAM.
fn world() -> World {
    let mut programs = Programs::with_spl_token();
    programs.deploy(ATTACKER_PROGRAM, Box::new(FakeToken));
    let mut w = World {
        programs,
        vault: Vault {
            mint: USDC,
            token_account: VAULT_TA,
            ..Default::default()
        },
        vault_ta: tai(VAULT_TA, VAULT_AUTHORITY, 0),
        alice_ta: tai(ALICE_TA, ALICE, ALICE_DEPOSIT),
        mallory_ta: tai(MALLORY_TA, MALLORY, MALLORY_FUNDS),
    };
    deposit(
        &w.programs,
        &mut w.vault,
        &signer(ALICE),
        &mut w.alice_ta,
        &mut w.vault_ta,
        &TOKEN_PROGRAM_ID,
        ALICE_DEPOSIT,
    )
    .expect("alice deposits");
    w
}

/// After the attack Mallory tries to cash out as much as the vault holds.
fn mallory_cashes_out(w: &mut World) {
    let shares = w.vault.shares.get(&MALLORY).copied().unwrap_or(0);
    let amt = shares.min(w.vault_ta.state.amount);
    let _ = withdraw(
        &w.programs,
        &mut w.vault,
        &signer(MALLORY),
        &mut w.vault_ta,
        &mut w.mallory_ta,
        &TOKEN_PROGRAM_ID,
        amt,
    );
}

fn assert_mallory_not_richer(w: &World, extra_held: u64) {
    let total = w.mallory_ta.state.amount + extra_held;
    assert!(
        total <= MALLORY_FUNDS && w.vault_ta.state.amount >= w.vault.total_shares,
        "EXPLOIT SUCCEEDED: Mallory holds {total} (started with {MALLORY_FUNDS}); vault holds {} for {} shares",
        w.vault_ta.state.amount,
        w.vault.total_shares
    );
}

// ---- honest use ----------------------------------------------------------

#[test]
fn honest_deposit_then_withdraw() {
    let mut w = world();
    assert_eq!(
        (w.vault_ta.state.amount, w.vault.total_shares),
        (ALICE_DEPOSIT, ALICE_DEPOSIT)
    );
    withdraw(
        &w.programs,
        &mut w.vault,
        &signer(ALICE),
        &mut w.vault_ta,
        &mut w.alice_ta,
        &TOKEN_PROGRAM_ID,
        400,
    )
    .unwrap();
    assert_eq!(
        (
            w.alice_ta.state.amount,
            w.vault_ta.state.amount,
            w.vault.total_shares
        ),
        (400, 600, 600)
    );
}

#[test]
fn honest_cannot_withdraw_more_than_shares() {
    let mut w = world();
    let r = withdraw(
        &w.programs,
        &mut w.vault,
        &signer(MALLORY),
        &mut w.vault_ta,
        &mut w.mallory_ta,
        &TOKEN_PROGRAM_ID,
        1,
    );
    assert_eq!(r, Err(SecurityError::InsufficientFunds));
    let r = withdraw(
        &w.programs,
        &mut w.vault,
        &signer(ALICE),
        &mut w.vault_ta,
        &mut w.alice_ta,
        &TOKEN_PROGRAM_ID,
        ALICE_DEPOSIT + 1,
    );
    assert_eq!(r, Err(SecurityError::InsufficientFunds));
}

#[test]
fn honest_deposit_needs_signature_and_funds() {
    let mut w = world();
    let mut unsigned = signer(MALLORY);
    unsigned.is_signer = false;
    let r = deposit(
        &w.programs,
        &mut w.vault,
        &unsigned,
        &mut w.mallory_ta,
        &mut w.vault_ta,
        &TOKEN_PROGRAM_ID,
        1,
    );
    assert_eq!(r, Err(SecurityError::MissingRequiredSignature));
    let r = deposit(
        &w.programs,
        &mut w.vault,
        &signer(MALLORY),
        &mut w.mallory_ta,
        &mut w.vault_ta,
        &TOKEN_PROGRAM_ID,
        MALLORY_FUNDS + 1,
    );
    assert_eq!(r, Err(SecurityError::InsufficientFunds));
    // Depositing from Alice's token account with Mallory's signature.
    let r = deposit(
        &w.programs,
        &mut w.vault,
        &signer(MALLORY),
        &mut w.alice_ta,
        &mut w.vault_ta,
        &TOKEN_PROGRAM_ID,
        1,
    );
    assert_eq!(r, Err(SecurityError::OwnerOrDelegateRequired));
    assert_eq!(w.vault.total_shares, ALICE_DEPOSIT);
}

proptest! {
    /// Honest deposits and withdrawals by two users: tokens and shares stay 1:1.
    #[test]
    fn honest_prop_vault_stays_backed(ops in proptest::collection::vec((any::<bool>(), any::<bool>(), 0u64..300), 1..30)) {
        let mut w = world();
        for (alice, dep, amt) in ops {
            let (who, ta) = if alice { (ALICE, &mut w.alice_ta) } else { (MALLORY, &mut w.mallory_ta) };
            let _ = if dep {
                deposit(&w.programs, &mut w.vault, &signer(who), ta, &mut w.vault_ta, &TOKEN_PROGRAM_ID, amt)
            } else {
                withdraw(&w.programs, &mut w.vault, &signer(who), &mut w.vault_ta, ta, &TOKEN_PROGRAM_ID, amt)
            };
            prop_assert_eq!(w.vault_ta.state.amount, w.vault.total_shares);
            prop_assert_eq!(w.alice_ta.state.amount + w.mallory_ta.state.amount + w.vault_ta.state.amount, ALICE_DEPOSIT + MALLORY_FUNDS);
        }
    }
}

// ---- the attacker's moves --------------------------------------------------

/// Mallory passes her own program as `token_program`.
#[test]
fn exploit_fake_token_program_mints_no_shares() {
    let mut w = world();
    let _ = deposit(
        &w.programs,
        &mut w.vault,
        &signer(MALLORY),
        &mut w.mallory_ta,
        &mut w.vault_ta,
        &ATTACKER_PROGRAM,
        ALICE_DEPOSIT,
    );
    mallory_cashes_out(&mut w);
    assert_mallory_not_richer(&w, 0);
}

/// Mallory passes her second USDC account as `vault_token`: her tokens
/// never leave her control, but shares are credited.
#[test]
fn exploit_substituted_vault_token_account_is_rejected() {
    let mut w = world();
    let mut decoy = tai(MALLORY_TA2, MALLORY, 0);
    let _ = deposit(
        &w.programs,
        &mut w.vault,
        &signer(MALLORY),
        &mut w.mallory_ta,
        &mut decoy,
        &TOKEN_PROGRAM_ID,
        MALLORY_FUNDS,
    );
    // move it back so we can count it, then repeat the trick
    w.mallory_ta.state.amount += std::mem::take(&mut decoy.state.amount);
    for _ in 0..19 {
        let _ = deposit(
            &w.programs,
            &mut w.vault,
            &signer(MALLORY),
            &mut w.mallory_ta,
            &mut decoy,
            &TOKEN_PROGRAM_ID,
            MALLORY_FUNDS,
        );
        w.mallory_ta.state.amount += std::mem::take(&mut decoy.state.amount);
    }
    mallory_cashes_out(&mut w);
    assert_mallory_not_richer(&w, decoy.state.amount);
}

proptest! {
    /// Any mix of fake program / decoy account / honest calls.
    #[test]
    fn exploit_prop_vault_always_backed(ops in proptest::collection::vec((0u8..3, 1u64..=MALLORY_FUNDS), 1..20)) {
        let mut w = world();
        let mut decoy = tai(MALLORY_TA2, MALLORY, 0);
        for (kind, amt) in ops {
            let _ = match kind {
                0 => deposit(&w.programs, &mut w.vault, &signer(MALLORY), &mut w.mallory_ta, &mut w.vault_ta, &ATTACKER_PROGRAM, amt),
                1 => deposit(&w.programs, &mut w.vault, &signer(MALLORY), &mut w.mallory_ta, &mut decoy, &TOKEN_PROGRAM_ID, amt),
                _ => deposit(&w.programs, &mut w.vault, &signer(MALLORY), &mut w.mallory_ta, &mut w.vault_ta, &TOKEN_PROGRAM_ID, amt),
            };
            w.mallory_ta.state.amount += std::mem::take(&mut decoy.state.amount);
        }
        prop_assert!(
            w.vault_ta.state.amount >= w.vault.total_shares,
            "EXPLOIT SUCCEEDED: vault holds {} tokens for {} shares", w.vault_ta.state.amount, w.vault.total_shares
        );
    }
}
