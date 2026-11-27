//! Capstone test suite. The `#[test]` functions live in this file. An
//! identical copy is in the other crate of the pair
//! (`capstone` / `capstone-solution`).
//!
//! From the workspace root: `cargo test -p capstone` or `cargo test -p capstone-solution`.
//! From this directory: `cargo test`.
extern crate capstone as cap;
// Capstone test suite: included by both `capstone` (vulnerable, your work)
// and `capstone-solution` (patched). `cap` is the crate under test.
//
// honest_*  : normal use. Passes on both crates.
// exploit_* : the attacker's moves, run locally. Fails on the vulnerable
//             crate ("EXPLOIT SUCCEEDED") and passes once the bug is fixed.
use cap::*;
use proptest::prelude::*;

const PROGRAM: Pubkey = Pubkey::new_from_byte(0xAA);
const ATTACKER_PROGRAM: Pubkey = Pubkey::new_from_byte(0xEE);
const ALICE: Pubkey = Pubkey::new_from_byte(1);
const MALLORY: Pubkey = Pubkey::new_from_byte(66);
const STATE: Pubkey = Pubkey::new_from_byte(10);
const TREASURY: Pubkey = Pubkey::new_from_byte(11);
const FAKE_STATE: Pubkey = Pubkey::new_from_byte(12);
const POT: u64 = 1_000_000;

fn wallet(key: Pubkey, lamports: u64) -> AccountInfo {
    AccountInfo::new(key, SYSTEM_PROGRAM_ID, lamports, 0)
}

fn withdraw(amount: u64) -> Vec<u8> {
    let mut d = vec![1];
    d.extend_from_slice(&amount.to_le_bytes());
    d
}

/// Alice's treasury holding POT lamports, plus her state account.
fn setup() -> (AccountInfo, AccountInfo) {
    let mut accs = vec![
        AccountInfo::new(STATE, PROGRAM, 1, STATE_LEN).writable(),
        AccountInfo::new(TREASURY, PROGRAM, POT, 0),
        wallet(ALICE, 0).signer(),
    ];
    process_instruction(&PROGRAM, &mut accs, &[0]).expect("initialize");
    (accs[0].clone(), accs[1].clone().writable())
}

fn assert_not_drained(
    res: Result<(), ProgramError>,
    treasury: &AccountInfo,
    mallory: &AccountInfo,
) {
    assert!(
        res.is_err() && treasury.lamports == POT && mallory.lamports == 0,
        "EXPLOIT SUCCEEDED: Mallory now holds {} lamports, treasury {} (result {:?})",
        mallory.lamports,
        treasury.lamports,
        res
    );
}

// ---- honest use ----------------------------------------------------------

#[test]
fn honest_alice_withdraws_to_herself() {
    let (state, treasury) = setup();
    let mut accs = vec![
        state,
        treasury,
        wallet(ALICE, 0).signer(),
        wallet(ALICE, 0).writable(),
    ];
    process_instruction(&PROGRAM, &mut accs, &withdraw(400)).unwrap();
    assert_eq!(accs[1].lamports, POT - 400);
    assert_eq!(accs[3].lamports, 400);
}

#[test]
fn honest_cannot_overdraw_or_double_init() {
    let (state, treasury) = setup();
    let mut accs = vec![
        state.clone(),
        treasury.clone(),
        wallet(ALICE, 0).signer(),
        wallet(ALICE, 0).writable(),
    ];
    assert_eq!(
        process_instruction(&PROGRAM, &mut accs, &withdraw(POT + 1)),
        Err(ProgramError::ArithmeticOverflow)
    );
    assert_eq!(accs[1].lamports, POT);
    let mut again = vec![state, treasury, wallet(ALICE, 0).signer()];
    assert_eq!(
        process_instruction(&PROGRAM, &mut again, &[0]),
        Err(ProgramError::AccountAlreadyInitialized)
    );
}

#[test]
fn honest_mallory_signing_as_herself_is_refused() {
    let (state, treasury) = setup();
    let mut accs = vec![
        state,
        treasury,
        wallet(MALLORY, 0).signer(),
        wallet(MALLORY, 0).writable(),
    ];
    let res = process_instruction(&PROGRAM, &mut accs, &withdraw(POT));
    assert_eq!(res, Err(ProgramError::InvalidAuthority));
}

#[test]
fn honest_bad_instruction_data() {
    let (state, treasury) = setup();
    let mut accs = vec![
        state,
        treasury,
        wallet(ALICE, 0).signer(),
        wallet(ALICE, 0).writable(),
    ];
    for bad in [&[][..], &[1, 2, 3], &[9]] {
        assert_eq!(
            process_instruction(&PROGRAM, &mut accs, bad),
            Err(ProgramError::InvalidInstructionData)
        );
    }
}

proptest! {
    #[test]
    fn honest_prop_withdrawals_conserve_lamports(amounts in proptest::collection::vec(0u64..400_000, 1..10)) {
        let (state, treasury) = setup();
        let mut accs = vec![state, treasury, wallet(ALICE, 0).signer(), wallet(ALICE, 0).writable()];
        for a in amounts {
            let _ = process_instruction(&PROGRAM, &mut accs, &withdraw(a));
            prop_assert_eq!(accs[1].lamports + accs[3].lamports, POT);
        }
    }
}

// ---- the attacker's moves --------------------------------------------------

/// Mallory names Alice as the authority but cannot sign for her.
#[test]
fn exploit_unsigned_authority_cannot_withdraw() {
    let (state, treasury) = setup();
    let mut accs = vec![
        state,
        treasury,
        wallet(ALICE, 0),
        wallet(MALLORY, 0).writable(),
    ];
    let res = process_instruction(&PROGRAM, &mut accs, &withdraw(POT));
    assert_not_drained(res, &accs[1], &accs[3]);
}

/// Mallory writes her own "state" under her own program: same tag, same
/// layout, authority = Mallory, treasury = Alice's treasury.
#[test]
fn exploit_fake_state_from_attacker_program_is_rejected() {
    let (_, treasury) = setup();
    let mut fake = AccountInfo::new(FAKE_STATE, ATTACKER_PROGRAM, 1, 0);
    fake.data = STATE_TAG.to_vec();
    VaultState {
        authority: MALLORY,
        treasury: TREASURY,
    }
    .serialize(&mut fake.data)
    .unwrap();
    let mut accs = vec![
        fake,
        treasury,
        wallet(MALLORY, 0).signer(),
        wallet(MALLORY, 0).writable(),
    ];
    let res = process_instruction(&PROGRAM, &mut accs, &withdraw(POT));
    assert_not_drained(res, &accs[1], &accs[3]);
}

proptest! {
    /// Any mix of the attacker's options: real or fake state, Alice's or
    /// Mallory's key, signed or not. Mallory must never end up richer.
    #[test]
    fn exploit_prop_mallory_never_profits(
        use_fake_state in any::<bool>(),
        name_alice in any::<bool>(),
        signed in any::<bool>(),
        amount in 1u64..=POT,
    ) {
        let (state, treasury) = setup();
        let state = if use_fake_state {
            let mut fake = AccountInfo::new(FAKE_STATE, ATTACKER_PROGRAM, 1, 0);
            fake.data = STATE_TAG.to_vec();
            VaultState { authority: MALLORY, treasury: TREASURY }.serialize(&mut fake.data).unwrap();
            fake
        } else {
            state
        };
        let mut auth = wallet(if name_alice { ALICE } else { MALLORY }, 0);
        auth.is_signer = signed && !name_alice; // Mallory can only sign as herself
        let mut accs = vec![state, treasury, auth, wallet(MALLORY, 0).writable()];
        let res = process_instruction(&PROGRAM, &mut accs, &withdraw(amount));
        prop_assert!(
            accs[3].lamports == 0,
            "EXPLOIT SUCCEEDED: Mallory took {} lamports ({:?})", accs[3].lamports, res
        );
    }
}
