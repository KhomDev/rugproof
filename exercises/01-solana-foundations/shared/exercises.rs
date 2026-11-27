// Shared test suite: included by both `starter` and `solutions`.
// `ex` is the crate under test.
use ex::*;
use proptest::prelude::*;

const PROGRAM: Pubkey = Pubkey::new_from_byte(0xAA);
const ATTACKER_PROGRAM: Pubkey = Pubkey::new_from_byte(0xEE);
const ALICE: Pubkey = Pubkey::new_from_byte(1);
const MALLORY: Pubkey = Pubkey::new_from_byte(66);
const COUNTER_KEY: Pubkey = Pubkey::new_from_byte(9);

fn rent() -> u64 {
    rent_exempt_minimum(COUNTER_LEN).unwrap()
}

fn wallet(key: Pubkey, lamports: u64) -> AccountInfo {
    AccountInfo::new(key, SYSTEM_PROGRAM_ID, lamports, 0)
}

fn ix_increment(by: u64) -> Vec<u8> {
    let mut d = vec![1];
    d.extend_from_slice(&by.to_le_bytes());
    d
}

/// A freshly initialized counter owned by `PROGRAM` with ALICE as authority.
fn initialized() -> Vec<AccountInfo> {
    let mut accs = vec![
        AccountInfo::new(COUNTER_KEY, PROGRAM, rent(), COUNTER_LEN).writable(),
        wallet(ALICE, 10).signer(),
    ];
    process_instruction(&PROGRAM, &mut accs, &[0]).expect("initialize");
    accs
}

/// Run an instruction and assert the runtime rules hold for whatever it did.
fn run(accs: &mut [AccountInfo], data: &[u8]) -> Result<(), ProgramError> {
    let before = accs.to_vec();
    let res = process_instruction(&PROGRAM, accs, data);
    check_transition(&PROGRAM, &before, accs).expect("program broke a runtime rule");
    if res.is_err() {
        assert_eq!(
            before,
            accs.to_vec(),
            "a failed instruction must not change state"
        );
    }
    res
}

// ---- Level 1 -------------------------------------------------------------

#[test]
fn l1_1_rent_matches_mainnet_values() {
    assert_eq!(rent_exempt_minimum(0), Some(890_880), "empty wallet");
    assert_eq!(
        rent_exempt_minimum(165),
        Some(2_039_280),
        "SPL token account"
    );
    assert_eq!(rent_exempt_minimum(82), Some(1_461_600), "SPL mint");
    assert_eq!(
        rent_exempt_minimum(usize::MAX),
        None,
        "overflow must be caught"
    );
}

#[test]
fn l1_2_can_modify_matrix() {
    let base = AccountInfo::new(COUNTER_KEY, PROGRAM, 1, 8);
    assert!(can_modify(&PROGRAM, &base.clone().writable()));
    assert!(!can_modify(&PROGRAM, &base), "read-only");
    assert!(
        !can_modify(&ATTACKER_PROGRAM, &base.clone().writable()),
        "not the owner"
    );
    let mut exe = base.writable();
    exe.executable = true;
    assert!(!can_modify(&PROGRAM, &exe), "executable");
}

#[test]
fn l1_3_require_signer() {
    assert_eq!(require_signer(&wallet(ALICE, 0).signer()), Ok(()));
    assert_eq!(
        require_signer(&wallet(ALICE, 0)),
        Err(ProgramError::MissingRequiredSignature)
    );
}

// ---- Level 2 -------------------------------------------------------------

#[test]
fn l2_1_next_account() {
    let v = [wallet(ALICE, 1), wallet(MALLORY, 2)];
    let mut it = v.iter();
    assert_eq!(next_account(&mut it).unwrap().key, ALICE);
    assert_eq!(next_account(&mut it).unwrap().key, MALLORY);
    assert_eq!(
        next_account(&mut it),
        Err(ProgramError::NotEnoughAccountKeys)
    );
}

#[test]
fn l2_2_decode_valid() {
    assert_eq!(decode_counter_ix(&[0]), Ok(CounterInstruction::Initialize));
    assert_eq!(
        decode_counter_ix(&ix_increment(7)),
        Ok(CounterInstruction::Increment { by: 7 })
    );
    assert_eq!(decode_counter_ix(&[2]), Ok(CounterInstruction::Close));
}

#[test]
fn l2_2_decode_adversarial() {
    for bad in [
        &[][..],
        &[0, 0],
        &[1, 1, 2],
        &[1; 10],
        &[2, 0],
        &[3],
        &[255; 9],
    ] {
        assert_eq!(
            decode_counter_ix(bad),
            Err(ProgramError::InvalidInstructionData),
            "input {bad:?}"
        );
    }
}

#[test]
fn l2_3_honest_transfer_is_allowed() {
    let before = vec![
        AccountInfo::new(COUNTER_KEY, PROGRAM, 100, 0).writable(),
        wallet(ALICE, 5).writable(),
    ];
    let mut after = before.clone();
    after[0].lamports = 60;
    after[1].lamports = 45;
    assert_eq!(check_transition(&PROGRAM, &before, &after), Ok(()));
}

#[test]
fn l2_3_runtime_rules_reject_each_violation() {
    let ours = AccountInfo::new(COUNTER_KEY, PROGRAM, 100, 4).writable();
    let theirs = wallet(ALICE, 100).writable();
    let before = vec![ours, theirs];
    let viol = |f: &dyn Fn(&mut Vec<AccountInfo>)| {
        let mut after = before.clone();
        f(&mut after);
        check_transition(&PROGRAM, &before, &after)
    };
    let is_violation =
        |r: Result<(), ProgramError>| matches!(r, Err(ProgramError::RuntimeViolation(_)));

    assert!(
        is_violation(viol(&|a| a[0].lamports += 1)),
        "minting lamports"
    );
    assert!(
        is_violation(viol(&|a| {
            a[1].lamports -= 10;
            a[0].lamports += 10;
        })),
        "debiting a foreign account"
    );
    assert!(
        is_violation(viol(&|a| a[1].data = vec![1])),
        "writing foreign data"
    );
    assert!(
        is_violation(viol(&|a| a[1].owner = PROGRAM)),
        "stealing ownership"
    );
    assert!(
        is_violation(viol(&|a| {
            a[0].owner = ATTACKER_PROGRAM;
            a[0].data = vec![1, 0, 0, 0];
        })),
        "owner change with live data"
    );
    // read-only accounts can't change even if we own them
    let ro_before = vec![AccountInfo::new(COUNTER_KEY, PROGRAM, 1, 1)];
    let mut ro_after = ro_before.clone();
    ro_after[0].data[0] = 7;
    assert!(is_violation(check_transition(
        &PROGRAM, &ro_before, &ro_after
    )));
    // a reordered or resized account list is not the same transaction
    let mut swapped = before.clone();
    swapped.swap(0, 1);
    assert_eq!(
        check_transition(&PROGRAM, &before, &swapped),
        Err(ProgramError::InvalidAccountData)
    );
    assert_eq!(
        check_transition(&PROGRAM, &before, &before[..1]),
        Err(ProgramError::InvalidAccountData)
    );
}

#[test]
fn l2_3_owner_can_close_by_zeroing_and_reassigning() {
    let before = vec![
        AccountInfo::new(COUNTER_KEY, PROGRAM, 100, 4).writable(),
        wallet(ALICE, 0).writable(),
    ];
    let mut after = before.clone();
    after[1].lamports = 100;
    after[0].lamports = 0;
    after[0].owner = SYSTEM_PROGRAM_ID;
    assert_eq!(check_transition(&PROGRAM, &before, &after), Ok(()));
}

// ---- Level 3 -------------------------------------------------------------

#[test]
fn l3_1_load_counter_checks_owner_length_and_tag() {
    let accs = initialized();
    assert_eq!(
        load_counter(&PROGRAM, &accs[0]),
        Ok(Counter {
            authority: ALICE,
            count: 0
        })
    );

    // An attacker-owned account with byte-identical contents is still fake.
    let mut fake = accs[0].clone();
    fake.owner = ATTACKER_PROGRAM;
    assert_eq!(
        load_counter(&PROGRAM, &fake),
        Err(ProgramError::IncorrectProgramId)
    );

    let mut wrong_tag = accs[0].clone();
    wrong_tag.data[..8].copy_from_slice(b"config__");
    assert_eq!(
        load_counter(&PROGRAM, &wrong_tag),
        Err(ProgramError::InvalidAccountData)
    );

    let mut short = accs[0].clone();
    short.data.pop();
    assert_eq!(
        load_counter(&PROGRAM, &short),
        Err(ProgramError::InvalidAccountData)
    );
}

#[test]
fn l3_2_initialize_and_increment() {
    let mut accs = initialized();
    run(&mut accs, &ix_increment(5)).unwrap();
    run(&mut accs, &ix_increment(2)).unwrap();
    assert_eq!(load_counter(&PROGRAM, &accs[0]).unwrap().count, 7);
}

#[test]
fn l3_2_initialize_rejections() {
    let fresh = || {
        vec![
            AccountInfo::new(COUNTER_KEY, PROGRAM, rent(), COUNTER_LEN).writable(),
            wallet(ALICE, 10).signer(),
        ]
    };
    let mut twice = initialized();
    assert_eq!(
        run(&mut twice, &[0]),
        Err(ProgramError::AccountAlreadyInitialized)
    );

    let mut poor = fresh();
    poor[0].lamports = rent() - 1;
    assert_eq!(
        run(&mut poor, &[0]),
        Err(ProgramError::AccountNotRentExempt)
    );

    let mut unsigned = fresh();
    unsigned[1].is_signer = false;
    assert_eq!(
        run(&mut unsigned, &[0]),
        Err(ProgramError::MissingRequiredSignature)
    );

    let mut foreign = fresh();
    foreign[0].owner = ATTACKER_PROGRAM;
    assert_eq!(
        run(&mut foreign, &[0]),
        Err(ProgramError::IncorrectProgramId)
    );

    let mut ro = fresh();
    ro[0].is_writable = false;
    assert_eq!(run(&mut ro, &[0]), Err(ProgramError::AccountNotWritable));

    let mut small = fresh();
    small[0].data.pop();
    assert_eq!(run(&mut small, &[0]), Err(ProgramError::InvalidAccountData));

    assert_eq!(
        run(&mut fresh()[..1], &[0]),
        Err(ProgramError::NotEnoughAccountKeys)
    );
}

#[test]
fn l3_2_adversarial_increment() {
    // Mallory signs, but she is not the stored authority.
    let mut accs = initialized();
    accs[1] = wallet(MALLORY, 10).signer();
    assert_eq!(
        run(&mut accs, &ix_increment(1)),
        Err(ProgramError::InvalidAuthority)
    );

    // Alice's key is passed, but she did not sign.
    let mut accs = initialized();
    accs[1].is_signer = false;
    assert_eq!(
        run(&mut accs, &ix_increment(1)),
        Err(ProgramError::MissingRequiredSignature)
    );

    // Mallory deploys her own program and creates a byte-identical counter
    // naming herself as authority. Owner check must stop it.
    let mut accs = initialized();
    accs[0].owner = ATTACKER_PROGRAM;
    accs[1] = wallet(MALLORY, 10).signer();
    assert_eq!(
        run(&mut accs, &ix_increment(1)),
        Err(ProgramError::IncorrectProgramId)
    );

    // Overflow is an error, not a wrap to zero.
    let mut accs = initialized();
    run(&mut accs, &ix_increment(u64::MAX)).unwrap();
    assert_eq!(
        run(&mut accs, &ix_increment(1)),
        Err(ProgramError::ArithmeticOverflow)
    );
}

#[test]
fn l3_2_close_pays_out_and_cannot_be_revived() {
    let mut accs = initialized();
    accs.push(wallet(ALICE, 10).writable());
    run(&mut accs, &[2]).unwrap();
    assert_eq!(accs[0].lamports, 0);
    assert_eq!(accs[2].lamports, 10 + rent());
    assert_eq!(accs[0].owner, SYSTEM_PROGRAM_ID);
    assert!(accs[0].data.iter().all(|&b| b == 0));
    // Someone refunds rent to the closed address: it is not a counter any more.
    accs[0].lamports = rent();
    assert_eq!(
        run(&mut accs, &ix_increment(1)),
        Err(ProgramError::IncorrectProgramId)
    );
}

#[test]
fn l3_2_close_rejections() {
    let mut to_self = initialized();
    to_self.push(to_self[0].clone());
    assert_eq!(
        run(&mut to_self, &[2]),
        Err(ProgramError::InvalidAccountData)
    );

    let mut ro_dest = initialized();
    ro_dest.push(wallet(MALLORY, 0));
    assert_eq!(
        run(&mut ro_dest, &[2]),
        Err(ProgramError::AccountNotWritable)
    );

    let mut thief = initialized();
    thief[1] = wallet(MALLORY, 0).signer();
    thief.push(wallet(MALLORY, 0).writable());
    assert_eq!(run(&mut thief, &[2]), Err(ProgramError::InvalidAuthority));

    assert_eq!(
        run(&mut initialized(), &[2]),
        Err(ProgramError::NotEnoughAccountKeys)
    );
}

// ---- Property tests ------------------------------------------------------

fn arb_account() -> impl Strategy<Value = AccountInfo> {
    (
        prop_oneof![
            Just(PROGRAM),
            Just(ATTACKER_PROGRAM),
            Just(SYSTEM_PROGRAM_ID)
        ],
        any::<u64>(),
        proptest::collection::vec(any::<u8>(), 0..64),
        any::<bool>(),
        any::<bool>(),
        any::<u8>(),
    )
        .prop_map(|(owner, lamports, data, signer, writable, k)| AccountInfo {
            key: Pubkey::new_from_byte(k),
            owner,
            lamports: lamports % 1_000_000_000_000,
            data,
            is_signer: signer,
            is_writable: writable,
            executable: false,
        })
}

proptest! {
    #[test]
    fn prop_decode_never_panics(data in proptest::collection::vec(any::<u8>(), 0..32)) {
        let _ = decode_counter_ix(&data);
    }

    #[test]
    fn prop_rent_is_monotonic(a in 0usize..10_000_000, b in 0usize..10_000_000) {
        let (lo, hi) = (a.min(b), a.max(b));
        prop_assert!(rent_exempt_minimum(lo).unwrap() <= rent_exempt_minimum(hi).unwrap());
    }

    // Integration: whatever garbage the attacker sends, the program never
    // panics, never breaks a runtime rule, and a failure changes nothing.
    #[test]
    fn prop_program_obeys_runtime_rules(
        mut accs in proptest::collection::vec(arb_account(), 0..4),
        data in proptest::collection::vec(any::<u8>(), 0..10),
    ) {
        let _ = run(&mut accs, &data);
    }

    // Same, starting from a real counter so the interesting paths run.
    #[test]
    fn prop_counter_only_grows_and_only_for_alice(
        steps in proptest::collection::vec((any::<u32>(), any::<bool>(), any::<bool>()), 1..20),
    ) {
        let mut accs = initialized();
        let mut expected = 0u64;
        for (by, alice, signed) in steps {
            accs[1] = wallet(if alice { ALICE } else { MALLORY }, 10);
            accs[1].is_signer = signed;
            let ok = run(&mut accs, &ix_increment(by as u64)).is_ok();
            prop_assert_eq!(ok, alice && signed);
            if ok { expected += by as u64; }
            prop_assert_eq!(load_counter(&PROGRAM, &accs[0]).unwrap().count, expected);
        }
    }
}
