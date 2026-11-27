//! Full exercise test suite (unit, property-based, adversarial, and
//! integration). The `#[test]` functions live in this file. An identical
//! copy is in the other crate of the pair (`starter` / `solutions`).
//!
//! From the workspace root: `cargo test -p starter` or `cargo test -p solutions`.
//! From this directory: `cargo test`.
extern crate solutions as ex;
// `ex` is the crate under test.
use ex::*;
use proptest::prelude::*;

const PROGRAM: Pubkey = Pubkey::new_from_byte(0xAA);
const ATTACKER_PROGRAM: Pubkey = Pubkey::new_from_byte(0xEE);
const USDC: Pubkey = Pubkey::new_from_byte(0x0C);
const JUNK: Pubkey = Pubkey::new_from_byte(0x0D);
const ALICE: Pubkey = Pubkey::new_from_byte(1);
const BOB: Pubkey = Pubkey::new_from_byte(2);
const MALLORY: Pubkey = Pubkey::new_from_byte(66);

fn mint() -> Mint {
    Mint {
        decimals: 6,
        supply: 1_000_000,
        mint_authority: None,
        freeze_authority: None,
    }
}

fn ta(mint: Pubkey, owner: Pubkey, amount: u64) -> TokenAccount {
    TokenAccount {
        mint,
        owner,
        amount,
        delegate: None,
        delegated_amount: 0,
        is_frozen: false,
    }
}

fn signer(k: Pubkey) -> AccountInfo {
    AccountInfo::new(k, SYSTEM_PROGRAM_ID, 0, 0).signer()
}

fn program(k: Pubkey) -> AccountInfo {
    let mut p = AccountInfo::new(k, Pubkey::new_from_byte(0xBB), 1, 0);
    p.executable = true;
    p
}

// ---- Level 1 -------------------------------------------------------------

#[test]
fn l1_1_check_owner() {
    let a = AccountInfo::new(ALICE, PROGRAM, 1, 0);
    assert_eq!(check_owner(&a, &PROGRAM), Ok(()));
    assert_eq!(
        check_owner(&a, &ATTACKER_PROGRAM),
        Err(SecurityError::IllegalOwner)
    );
}

#[test]
fn l1_2_check_distinct() {
    assert_eq!(check_distinct(&[]), Ok(()));
    assert_eq!(check_distinct(&[ALICE, BOB, MALLORY]), Ok(()));
    assert_eq!(
        check_distinct(&[ALICE, BOB, ALICE]),
        Err(SecurityError::DuplicateAccount)
    );
}

#[test]
fn l1_3_fee_rounds_up() {
    assert_eq!(fee_amount(10_000, 30), Ok(30));
    assert_eq!(fee_amount(1, 30), Ok(1), "dust still pays a fee");
    assert_eq!(fee_amount(0, 30), Ok(0));
    assert_eq!(fee_amount(333, 10_000), Ok(333));
    assert_eq!(fee_amount(u64::MAX, 10_000), Ok(u64::MAX), "no overflow");
    assert_eq!(fee_amount(1, 10_001), Err(SecurityError::InvalidArgument));
}

// ---- Level 2 -------------------------------------------------------------

#[test]
fn l2_1_validate_token_account() {
    let good = ta(USDC, ALICE, 5);
    assert_eq!(validate_token_account(&good, &USDC, &ALICE), Ok(()));
    assert_eq!(
        validate_token_account(&ta(JUNK, ALICE, 5), &USDC, &ALICE),
        Err(SecurityError::MintMismatch)
    );
    assert_eq!(
        validate_token_account(&good, &USDC, &MALLORY),
        Err(SecurityError::OwnerMismatch)
    );
    let mut frozen = good;
    frozen.is_frozen = true;
    assert_eq!(
        validate_token_account(&frozen, &USDC, &ALICE),
        Err(SecurityError::AccountFrozen)
    );
}

#[test]
fn l2_2_transfer_by_owner() {
    let (mut a, mut b) = (ta(USDC, ALICE, 100), ta(USDC, BOB, 0));
    transfer_checked(&mut a, &mut b, &signer(ALICE), &USDC, &mint(), 40, 6).unwrap();
    assert_eq!((a.amount, b.amount), (60, 40));
}

#[test]
fn l2_2_transfer_by_delegate_spends_allowance() {
    let (mut a, mut b) = (ta(USDC, ALICE, 100), ta(USDC, BOB, 0));
    a.delegate = Some(BOB);
    a.delegated_amount = 30;
    transfer_checked(&mut a, &mut b, &signer(BOB), &USDC, &mint(), 20, 6).unwrap();
    assert_eq!(
        (a.amount, a.delegated_amount, a.delegate),
        (80, 10, Some(BOB))
    );
    assert_eq!(
        transfer_checked(&mut a, &mut b, &signer(BOB), &USDC, &mint(), 11, 6),
        Err(SecurityError::InsufficientFunds),
        "allowance is a hard cap"
    );
    transfer_checked(&mut a, &mut b, &signer(BOB), &USDC, &mint(), 10, 6).unwrap();
    assert_eq!(a.delegate, None, "a spent allowance revokes the delegate");
}

#[test]
fn l2_2_transfer_adversarial() {
    let check = |a: TokenAccount,
                 b: TokenAccount,
                 auth: AccountInfo,
                 amt: u64,
                 dec: u8,
                 want: SecurityError| {
        let (mut a2, mut b2) = (a.clone(), b.clone());
        assert_eq!(
            transfer_checked(&mut a2, &mut b2, &auth, &USDC, &mint(), amt, dec),
            Err(want.clone()),
            "{want:?}"
        );
        assert_eq!((a2, b2), (a, b), "failed transfer must not change state");
    };
    let (a, b) = (ta(USDC, ALICE, 100), ta(USDC, BOB, 0));
    check(
        a.clone(),
        b.clone(),
        signer(MALLORY),
        1,
        6,
        SecurityError::OwnerOrDelegateRequired,
    );
    let mut unsigned = signer(ALICE);
    unsigned.is_signer = false;
    check(
        a.clone(),
        b.clone(),
        unsigned,
        1,
        6,
        SecurityError::MissingRequiredSignature,
    );
    check(
        a.clone(),
        b.clone(),
        signer(ALICE),
        101,
        6,
        SecurityError::InsufficientFunds,
    );
    check(
        a.clone(),
        b.clone(),
        signer(ALICE),
        1,
        9,
        SecurityError::MintDecimalsMismatch,
    );
    check(
        a.clone(),
        ta(JUNK, BOB, 0),
        signer(ALICE),
        1,
        6,
        SecurityError::MintMismatch,
    );
    check(
        ta(JUNK, ALICE, 100),
        b.clone(),
        signer(ALICE),
        1,
        6,
        SecurityError::MintMismatch,
    );
    let mut frozen = b.clone();
    frozen.is_frozen = true;
    check(
        a.clone(),
        frozen,
        signer(ALICE),
        1,
        6,
        SecurityError::AccountFrozen,
    );
    check(
        a,
        ta(USDC, BOB, u64::MAX),
        signer(ALICE),
        1,
        6,
        SecurityError::Overflow,
    );
}

#[test]
fn l2_3_cpi_target_must_be_allowed_and_executable() {
    let allowed = [TOKEN_PROGRAM_ID, TOKEN_2022_PROGRAM_ID];
    assert_eq!(
        check_cpi_program(&program(TOKEN_PROGRAM_ID), &allowed),
        Ok(())
    );
    assert_eq!(
        check_cpi_program(&program(TOKEN_2022_PROGRAM_ID), &allowed),
        Ok(())
    );
    assert_eq!(
        check_cpi_program(&program(ATTACKER_PROGRAM), &allowed),
        Err(SecurityError::ArbitraryCpi)
    );
    let mut not_exe = program(TOKEN_PROGRAM_ID);
    not_exe.executable = false;
    assert_eq!(
        check_cpi_program(&not_exe, &allowed),
        Err(SecurityError::ArbitraryCpi)
    );
}

// ---- Level 3 -------------------------------------------------------------

#[test]
fn l3_1_sysvar_by_key_not_by_contents() {
    let real = AccountInfo::new(INSTRUCTIONS_SYSVAR_ID, Pubkey::new_from_byte(0x53), 1, 16);
    assert_eq!(check_sysvar(&real, &INSTRUCTIONS_SYSVAR_ID), Ok(()));
    // Same owner, same bytes, different address: the fake-sysvar bug class.
    let mut fake = real.clone();
    fake.key = MALLORY;
    assert_eq!(
        check_sysvar(&fake, &INSTRUCTIONS_SYSVAR_ID),
        Err(SecurityError::InvalidSysvar)
    );
    assert_eq!(
        check_sysvar(&real, &CLOCK_SYSVAR_ID),
        Err(SecurityError::InvalidSysvar)
    );
}

#[test]
fn l3_2_initialize_once() {
    let mut d = vec![0u8; VAULT_LEN];
    initialize_vault(&mut d, &ALICE).unwrap();
    assert_eq!(&d[..8], &VAULT_TAG);
    assert_eq!(&d[8..], &ALICE.0);
    assert_eq!(
        initialize_vault(&mut d, &MALLORY),
        Err(SecurityError::AlreadyInitialized)
    );
    assert_eq!(&d[8..], &ALICE.0, "re-init must not replace the authority");
    let mut closed = vec![0u8; VAULT_LEN];
    closed[..8].copy_from_slice(&CLOSED_ACCOUNT_DISCRIMINATOR);
    assert_eq!(
        initialize_vault(&mut closed, &MALLORY),
        Err(SecurityError::AccountClosed)
    );
    assert_eq!(
        initialize_vault(&mut [0u8; 3], &ALICE),
        Err(SecurityError::InvalidArgument)
    );
}

#[test]
fn l3_2_close_account() {
    let mut acc = AccountInfo::new(ALICE, PROGRAM, 500, VAULT_LEN).writable();
    initialize_vault(&mut acc.data, &ALICE).unwrap();
    let mut dest = AccountInfo::new(BOB, SYSTEM_PROGRAM_ID, 7, 0).writable();
    close_account(&mut acc, &mut dest).unwrap();
    assert_eq!(
        (acc.lamports, dest.lamports, acc.owner),
        (0, 507, SYSTEM_PROGRAM_ID)
    );
    assert!(acc.data.iter().all(|&b| b == 0));
    let mut me = AccountInfo::new(ALICE, PROGRAM, 500, 0);
    let mut me2 = me.clone();
    assert_eq!(
        close_account(&mut me, &mut me2),
        Err(SecurityError::DuplicateAccount)
    );
}

fn use_(name: &'static str, role: Role, mutated: bool, checks: Checks) -> AccountUse {
    AccountUse {
        name,
        role,
        mutated,
        checks,
    }
}

#[test]
fn l3_3_audit_clean_handler() {
    let all = Checks {
        signer: true,
        owner: true,
        key: true,
        mint: true,
        token_owner: true,
        writable: true,
        distinct: true,
    };
    let accounts = [
        use_("authority", Role::Authority, false, all),
        use_("vault", Role::State, true, all),
        use_("from", Role::TokenAccount, true, all),
        use_("to", Role::TokenAccount, true, all),
        use_("token_program", Role::CpiProgram, false, all),
        use_("instructions", Role::Sysvar, false, all),
    ];
    assert_eq!(audit(&accounts), vec![]);
}

#[test]
fn l3_3_audit_finds_every_missing_check() {
    let none = Checks::default();
    let accounts = [
        use_("authority", Role::Authority, false, none),
        use_(
            "vault",
            Role::State,
            true,
            Checks {
                writable: true,
                ..none
            },
        ),
        use_(
            "from",
            Role::TokenAccount,
            true,
            Checks {
                writable: true,
                mint: true,
                ..none
            },
        ),
        use_(
            "to",
            Role::TokenAccount,
            true,
            Checks {
                writable: true,
                token_owner: true,
                ..none
            },
        ),
        use_("token_program", Role::CpiProgram, false, none),
        use_("instructions", Role::Sysvar, false, none),
        use_("dest", Role::Other, true, none),
    ];
    let mut want = vec![
        Finding::MissingSigner("authority"),
        Finding::MissingOwnerCheck("vault"),
        Finding::MissingTokenOwnerCheck("from"),
        Finding::MissingMintCheck("to"),
        Finding::ArbitraryCpi("token_program"),
        Finding::FakeSysvar("instructions"),
        Finding::NotWritable("dest"),
        Finding::PossibleDuplicate("from", "to"),
    ];
    want.sort();
    assert_eq!(audit(&accounts), want);
}

// ---- Property tests ------------------------------------------------------

proptest! {
    #[test]
    fn prop_fee_bounds(amount in any::<u64>(), bps in 0u16..=10_000) {
        let fee = fee_amount(amount, bps).unwrap();
        prop_assert!(fee <= amount);
        // never less than the exact fee
        prop_assert!(fee as u128 * 10_000 >= amount as u128 * bps as u128);
        // and less than one unit above it
        prop_assert!((fee as u128) * 10_000 < amount as u128 * bps as u128 + 10_000);
    }

    #[test]
    fn prop_distinct_matches_set_size(keys in proptest::collection::vec(0u8..8, 0..8)) {
        let ks: Vec<Pubkey> = keys.iter().map(|&b| Pubkey::new_from_byte(b)).collect();
        let unique = ks.iter().collect::<std::collections::BTreeSet<_>>().len() == ks.len();
        prop_assert_eq!(check_distinct(&ks).is_ok(), unique);
    }

    // Integration: random transfers between three accounts by random signers.
    // Supply is conserved, no balance goes negative, and only owners or
    // delegates within their allowance ever move funds.
    #[test]
    fn prop_transfers_conserve_supply(
        ops in proptest::collection::vec((0usize..3, 0usize..3, 0u8..4, 0u64..60, any::<bool>()), 1..40),
    ) {
        let owners = [ALICE, BOB, MALLORY];
        let mut accs = [ta(USDC, ALICE, 100), ta(USDC, BOB, 100), ta(USDC, MALLORY, 100)];
        accs[0].delegate = Some(MALLORY);
        accs[0].delegated_amount = 25;
        let mut mallory_took_from_alice = 0u64;
        for (from, to, who, amt, signed) in ops {
            if from == to { continue; }
            let who_key = if who == 3 { Pubkey::new_from_byte(99) } else { owners[who as usize] };
            let mut auth = signer(who_key);
            auth.is_signer = signed;
            let (lo, hi) = (from.min(to), from.max(to));
            let (left, right) = accs.split_at_mut(hi);
            let (a, b) = if from < to { (&mut left[lo], &mut right[0]) } else { (&mut right[0], &mut left[lo]) };
            if transfer_checked(a, b, &auth, &USDC, &mint(), amt, 6).is_ok() {
                prop_assert!(signed);
                if from == 0 && who_key == MALLORY { mallory_took_from_alice += amt; }
                else { prop_assert_eq!(who_key, owners[from]); }
            }
        }
        prop_assert_eq!(accs.iter().map(|a| a.amount).sum::<u64>(), 300);
        prop_assert!(mallory_took_from_alice <= 25);
    }

    #[test]
    fn prop_initialize_only_on_zeroed(data in proptest::collection::vec(any::<u8>(), VAULT_LEN)) {
        let mut d = data.clone();
        let r = initialize_vault(&mut d, &ALICE);
        prop_assert_eq!(r.is_ok(), data.iter().all(|&b| b == 0));
        if r.is_err() { prop_assert_eq!(d, data); }
    }
}
