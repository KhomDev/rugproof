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

const ALICE: UserId = 1;
const BOB: UserId = 2;
const MALLORY: UserId = 66;
const ACCOMPLICE: UserId = 67;
const FEE: u64 = 5;

fn ledger() -> Ledger {
    let mut l = Ledger {
        fee: FEE,
        ..Default::default()
    };
    l.mint(ALICE, 1_000).unwrap();
    l.mint(MALLORY, 10).unwrap();
    l
}

fn assert_supply_intact(l: &Ledger) {
    assert!(
        l.sum_of_balances() == l.total_supply as u128
            && l.balance(MALLORY) as u128 + l.balance(ACCOMPLICE) as u128 <= 10,
        "EXPLOIT SUCCEEDED: Mallory+accomplice hold {}, balances sum to {} but supply is {}",
        l.balance(MALLORY) as u128 + l.balance(ACCOMPLICE) as u128,
        l.sum_of_balances(),
        l.total_supply
    );
}

// ---- honest use ----------------------------------------------------------

#[test]
fn honest_transfer_pays_fee() {
    let mut l = ledger();
    l.transfer(ALICE, BOB, 100).unwrap();
    assert_eq!(
        (l.balance(ALICE), l.balance(BOB), l.balance(TREASURY)),
        (895, 100, FEE)
    );
    assert_eq!(l.sum_of_balances(), l.total_supply as u128);
}

#[test]
fn honest_transfer_rejections() {
    let mut l = ledger();
    assert_eq!(
        l.transfer(ALICE, BOB, 996),
        Err(LedgerError::InsufficientBalance),
        "fee counts"
    );
    assert_eq!(
        l.transfer(BOB, ALICE, 0),
        Err(LedgerError::InsufficientBalance),
        "Bob can't pay the fee"
    );
    assert_eq!(l.balance(ALICE), 1_000);
    l.transfer(ALICE, ALICE, 10).unwrap();
    assert_eq!(l.balance(ALICE), 995, "self-transfer only costs the fee");
}

#[test]
fn honest_mint_overflow() {
    let mut l = ledger();
    assert_eq!(l.mint(BOB, u64::MAX), Err(LedgerError::Overflow));
    assert_eq!(l.balance(BOB), 0);
}

#[test]
fn honest_batch_payout() {
    let mut l = ledger();
    l.batch_payout(ALICE, &[(BOB, 300), (MALLORY, 200)])
        .unwrap();
    assert_eq!(
        (l.balance(ALICE), l.balance(BOB), l.balance(MALLORY)),
        (500, 300, 210)
    );
    assert_eq!(
        l.batch_payout(ALICE, &[(BOB, 501)]),
        Err(LedgerError::InsufficientBalance)
    );
    assert_eq!(l.batch_payout(ALICE, &[]), Err(LedgerError::EmptyBatch));
    assert_eq!(l.sum_of_balances(), l.total_supply as u128);
}

proptest! {
    #[test]
    fn honest_prop_small_transfers_conserve_supply(ops in proptest::collection::vec((0u32..4, 0u32..4, 0u64..2_000), 1..40)) {
        let mut l = ledger();
        for (f, t, a) in ops {
            let _ = l.transfer(f, t, a);
            prop_assert_eq!(l.sum_of_balances(), l.total_supply as u128);
        }
    }
}

// ---- the attacker's moves --------------------------------------------------

/// Mallory (balance 10) sends an amount so large that amount + fee wraps.
#[test]
fn exploit_wrapping_fee_cannot_print_points() {
    let mut l = ledger();
    let _ = l.transfer(MALLORY, ACCOMPLICE, u64::MAX - FEE + 1);
    assert_supply_intact(&l);
}

/// Mallory pays out a batch whose total is 2^64 + 10, split across two
/// accounts she controls.
#[test]
fn exploit_truncated_batch_total_is_rejected() {
    let mut l = ledger();
    let _ = l.batch_payout(MALLORY, &[(ACCOMPLICE, u64::MAX), (ACCOMPLICE + 1, 11)]);
    assert_supply_intact(&l);
}

proptest! {
    /// Any amount, any batch: supply is conserved and Mallory can't gain.
    #[test]
    fn exploit_prop_supply_conserved(
        amount in prop_oneof![any::<u64>(), (u64::MAX - 100)..=u64::MAX],
        batch in proptest::collection::vec(prop_oneof![any::<u64>(), (u64::MAX - 100)..=u64::MAX], 1..4),
    ) {
        let mut l = ledger();
        let _ = l.transfer(MALLORY, ACCOMPLICE, amount);
        let payouts: Vec<(UserId, u64)> = batch.into_iter().map(|a| (ACCOMPLICE, a)).collect();
        let _ = l.batch_payout(MALLORY, &payouts);
        prop_assert!(
            l.sum_of_balances() == l.total_supply as u128,
            "EXPLOIT SUCCEEDED: balances sum to {} but supply is {}", l.sum_of_balances(), l.total_supply
        );
    }
}
