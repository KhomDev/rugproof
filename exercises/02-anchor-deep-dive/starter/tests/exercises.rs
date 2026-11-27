//! Full exercise test suite (unit, property-based, adversarial, and
//! integration). The `#[test]` functions live in this file. An identical
//! copy is in the other crate of the pair (`starter` / `solutions`).
//!
//! From the workspace root: `cargo test -p starter` or `cargo test -p solutions`.
//! From this directory: `cargo test`.
extern crate starter as ex;
// `ex` is the crate under test.
use ex::*;
use proptest::prelude::*;

const PROGRAM: Pubkey = Pubkey::new_from_byte(0xAA);
const OTHER_PROGRAM: Pubkey = Pubkey::new_from_byte(0xEE);
const ADMIN: Pubkey = Pubkey::new_from_byte(1);
const MALLORY: Pubkey = Pubkey::new_from_byte(66);

fn signer(key: Pubkey) -> AccountInfo {
    AccountInfo::new(key, SYSTEM_PROGRAM_ID, 0, 0).signer()
}

fn config_pda() -> (Pubkey, u8) {
    find_program_address(&[CONFIG_SEED], &PROGRAM).unwrap()
}

/// An uninitialized config account at the canonical PDA.
fn empty_config() -> AccountInfo {
    AccountInfo::new(config_pda().0, SYSTEM_PROGRAM_ID, 0, 0).writable()
}

fn live_config(fee: u16) -> AccountInfo {
    let mut c = empty_config();
    initialize_config(&PROGRAM, &mut c, &signer(ADMIN), fee).unwrap();
    c
}

/// All (bump, address) pairs that are valid PDAs for `seeds`, highest bump first.
fn all_valid_bumps(seeds: &[&[u8]]) -> Vec<(u8, Pubkey)> {
    (0..=255u8)
        .rev()
        .filter_map(|b| {
            create_program_address(seeds, b, &PROGRAM)
                .ok()
                .map(|k| (b, k))
        })
        .collect()
}

// ---- Level 1 -------------------------------------------------------------

#[test]
fn l1_1_sighash_matches_anchor() {
    // Every Anchor program's `initialize` instruction starts with these bytes.
    assert_eq!(
        sighash("global", "initialize"),
        [175, 175, 109, 31, 13, 152, 155, 237]
    );
    assert_ne!(sighash("account", "Config"), sighash("account", "Vault"));
    assert_ne!(
        sighash("global", "Config"),
        sighash("account", "Config"),
        "namespaces differ"
    );
    assert_eq!(Config::discriminator(), sighash("account", "Config"));
}

#[test]
fn l1_2_space_examples() {
    use Field::*;
    assert_eq!(account_space(&[]), 8);
    assert_eq!(account_space(&[Pubkey, U16, U8]), CONFIG_SPACE);
    assert_eq!(field_space(&String { max_len: 50 }), 54);
    assert_eq!(
        field_space(&Vec {
            elem: Box::new(Pubkey),
            max_len: 10
        }),
        324
    );
    assert_eq!(field_space(&Option(Box::new(U64))), 9);
    assert_eq!(
        field_space(&Array {
            elem: Box::new(U32),
            len: 4
        }),
        16,
        "arrays have no length prefix"
    );
    assert_eq!(
        field_space(&Bool) + field_space(&I64) + field_space(&U128),
        25
    );
}

#[test]
fn l1_2_space_fits_real_serialization() {
    // The computed space must hold the largest value Borsh can produce.
    #[derive(BorshSerialize)]
    struct Profile {
        owner: ex::Pubkey,
        name: std::string::String,
        tags: std::vec::Vec<u64>,
        nick: std::option::Option<u32>,
    }
    let max = Profile {
        owner: ADMIN,
        name: "x".repeat(20),
        tags: vec![u64::MAX; 5],
        nick: Some(1),
    };
    let fields = [
        Field::Pubkey,
        Field::String { max_len: 20 },
        Field::Vec {
            elem: Box::new(Field::U64),
            max_len: 5,
        },
        Field::Option(Box::new(Field::U32)),
    ];
    assert_eq!(
        8 + borsh::to_vec(&max).unwrap().len(),
        account_space(&fields)
    );
}

// ---- Level 2 -------------------------------------------------------------

#[test]
fn l2_1_load_account_happy_and_trailing_bytes() {
    let mut info = AccountInfo::new(config_pda().0, PROGRAM, 1, 0);
    let cfg = Config {
        admin: ADMIN,
        fee_bps: 30,
        bump: 7,
    };
    info.data = account_bytes(&cfg, CONFIG_SPACE + 100);
    assert_eq!(load_account::<Config>(&PROGRAM, &info), Ok(cfg));
}

#[test]
fn l2_1_load_account_rejects_in_anchor_order() {
    #[derive(BorshSerialize, BorshDeserialize)]
    struct Vault {
        authority: ex::Pubkey,
        amount: u16,
        bump: u8,
    }
    impl AnchorAccount for Vault {
        const NAME: &'static str = "Vault";
    }
    let good = account_bytes(
        &Config {
            admin: ADMIN,
            fee_bps: 1,
            bump: 1,
        },
        CONFIG_SPACE,
    );
    let mut info = AccountInfo::new(ADMIN, OTHER_PROGRAM, 1, 0);
    info.data = good.clone();
    assert_eq!(
        load_account::<Config>(&PROGRAM, &info),
        Err(AnchorError::AccountOwnedByWrongProgram)
    );
    info.owner = PROGRAM;
    info.data = vec![1, 2, 3];
    assert_eq!(
        load_account::<Config>(&PROGRAM, &info),
        Err(AnchorError::AccountDiscriminatorNotFound)
    );
    // Same layout, different type: the discriminator is what stops cosplay.
    info.data = account_bytes(
        &Vault {
            authority: MALLORY,
            amount: 0,
            bump: 0,
        },
        CONFIG_SPACE,
    );
    assert_eq!(
        load_account::<Config>(&PROGRAM, &info),
        Err(AnchorError::AccountDiscriminatorMismatch)
    );
    info.data = good[..12].to_vec();
    assert_eq!(
        load_account::<Config>(&PROGRAM, &info),
        Err(AnchorError::AccountDidNotDeserialize)
    );
}

#[test]
fn l2_2_pda_is_deterministic_and_off_curve() {
    let (k, bump) = config_pda();
    assert_eq!(
        create_program_address(&[CONFIG_SEED], bump, &PROGRAM),
        Ok(k)
    );
    assert!(!is_on_curve(&k.0));
    for b in (bump as u16 + 1)..=255 {
        assert_eq!(
            create_program_address(&[CONFIG_SEED], b as u8, &PROGRAM),
            Err(AnchorError::InvalidSeeds),
            "every bump above the canonical one is on-curve"
        );
    }
    assert_ne!(
        find_program_address(&[CONFIG_SEED], &OTHER_PROGRAM)
            .unwrap()
            .0,
        k,
        "program id is part of the hash"
    );
    assert_ne!(
        find_program_address(&[b"config", b"x"], &PROGRAM)
            .unwrap()
            .0,
        k
    );
}

#[test]
fn l2_2_seed_limits() {
    let long = [0u8; 33];
    assert_eq!(
        create_program_address(&[&long], 0, &PROGRAM),
        Err(AnchorError::MaxSeedLengthExceeded)
    );
    assert_eq!(
        find_program_address(&[&long], &PROGRAM),
        Err(AnchorError::MaxSeedLengthExceeded)
    );
    let s: &[u8] = b"s";
    assert_eq!(
        find_program_address(&[s; 16], &PROGRAM),
        Err(AnchorError::MaxSeedLengthExceeded)
    );
    assert!(
        find_program_address(&[s; 15], &PROGRAM).is_ok(),
        "15 seeds + bump is the maximum"
    );
}

#[test]
fn l2_3_check_seeds_requires_canonical_bump() {
    let seeds: &[&[u8]] = &[b"receipt", &MALLORY.0];
    let valid = all_valid_bumps(seeds);
    assert!(valid.len() >= 2, "need a non-canonical bump for this test");
    let (canon_bump, canon_key) = valid[0];
    assert_eq!(check_seeds(seeds, &canon_key, &PROGRAM), Ok(canon_bump));
    let (_, other_key) = valid[1];
    assert_eq!(
        check_seeds(seeds, &other_key, &PROGRAM),
        Err(AnchorError::ConstraintSeeds)
    );
    assert_eq!(
        check_seeds(seeds, &MALLORY, &PROGRAM),
        Err(AnchorError::ConstraintSeeds)
    );
}

// ---- Level 3 -------------------------------------------------------------

#[test]
fn l3_1_initialize_config() {
    let c = live_config(30);
    assert_eq!(c.owner, PROGRAM);
    assert_eq!(c.data.len(), CONFIG_SPACE);
    let cfg: Config = load_account(&PROGRAM, &c).unwrap();
    assert_eq!(
        cfg,
        Config {
            admin: ADMIN,
            fee_bps: 30,
            bump: config_pda().1
        }
    );
}

#[test]
fn l3_1_initialize_rejections_leave_state_unchanged() {
    let check = |mut c: AccountInfo, admin: AccountInfo, fee: u16, want: AnchorError| {
        let before = c.clone();
        assert_eq!(
            initialize_config(&PROGRAM, &mut c, &admin, fee),
            Err(want.clone()),
            "{want:?}"
        );
        assert_eq!(c, before);
    };
    check(
        empty_config(),
        signer(ADMIN),
        MAX_FEE_BPS + 1,
        AnchorError::FeeTooHigh,
    );
    let mut unsigned = signer(ADMIN);
    unsigned.is_signer = false;
    check(empty_config(), unsigned, 1, AnchorError::ConstraintSigner);
    let mut wrong_key = empty_config();
    wrong_key.key = MALLORY;
    check(wrong_key, signer(ADMIN), 1, AnchorError::ConstraintSeeds);
    // Re-initialising a live config would let anyone replace the admin.
    check(
        live_config(1),
        signer(MALLORY),
        1,
        AnchorError::AccountAlreadyInUse,
    );
    assert!(initialize_config(&PROGRAM, &mut empty_config(), &signer(ADMIN), MAX_FEE_BPS).is_ok());
}

#[test]
fn l3_2_set_fee_admin_only() {
    let mut c = live_config(30);
    set_fee(&PROGRAM, &mut c, &signer(ADMIN), 50).unwrap();
    assert_eq!(load_account::<Config>(&PROGRAM, &c).unwrap().fee_bps, 50);

    let before = c.clone();
    assert_eq!(
        set_fee(&PROGRAM, &mut c, &signer(MALLORY), 0),
        Err(AnchorError::ConstraintHasOne)
    );
    let mut unsigned = signer(ADMIN);
    unsigned.is_signer = false;
    assert_eq!(
        set_fee(&PROGRAM, &mut c, &unsigned, 0),
        Err(AnchorError::ConstraintSigner)
    );
    assert_eq!(
        set_fee(&PROGRAM, &mut c, &signer(ADMIN), 1_001),
        Err(AnchorError::FeeTooHigh)
    );
    assert_eq!(c, before);
}

#[test]
fn l3_2_set_fee_rejects_lookalike_configs() {
    // Mallory's program writes a Config naming her admin.
    let mut fake = live_config(30);
    fake.owner = OTHER_PROGRAM;
    fake.data = account_bytes(
        &Config {
            admin: MALLORY,
            fee_bps: 0,
            bump: config_pda().1,
        },
        CONFIG_SPACE,
    );
    assert_eq!(
        set_fee(&PROGRAM, &mut fake, &signer(MALLORY), 0),
        Err(AnchorError::AccountOwnedByWrongProgram)
    );
    // A genuine Config living at a non-PDA address (wrong seeds).
    let mut moved = live_config(30);
    moved.key = MALLORY;
    assert_eq!(
        set_fee(&PROGRAM, &mut moved, &signer(ADMIN), 0),
        Err(AnchorError::ConstraintSeeds)
    );
}

fn staker(key: Pubkey) -> UserStake {
    UserStake {
        owner: key,
        amount: 0,
        reward_debt: 0,
    }
}

#[test]
fn l3_3_rewards_split_pro_rata() {
    let mut pool = Pool::default();
    let (mut a, mut b) = (staker(ADMIN), staker(MALLORY));
    stake(&mut pool, &mut a, &signer(ADMIN), 100).unwrap();
    stake(&mut pool, &mut b, &signer(MALLORY), 300).unwrap();
    distribute(&mut pool, 400).unwrap();
    assert_eq!(pending(&pool, &a), Ok(100));
    assert_eq!(pending(&pool, &b), Ok(300));
    // Late joiner earns nothing from earlier rewards.
    let mut c = staker(Pubkey::new_from_byte(3));
    stake(&mut pool, &mut c, &signer(Pubkey::new_from_byte(3)), 400).unwrap();
    assert_eq!(pending(&pool, &c), Ok(0));
    let (paid, out) = unstake(&mut pool, &mut a, &signer(ADMIN), 100).unwrap();
    assert_eq!((paid, out, pool.total_staked), (100, 100, 700));
    assert_eq!(pending(&pool, &a), Ok(0), "rewards are paid once");
}

#[test]
fn l3_3_staking_rejections() {
    let mut pool = Pool::default();
    let mut a = staker(ADMIN);
    stake(&mut pool, &mut a, &signer(ADMIN), 10).unwrap();
    let (p0, a0) = (pool.clone(), a.clone());
    assert_eq!(
        unstake(&mut pool, &mut a, &signer(MALLORY), 10),
        Err(AnchorError::ConstraintHasOne)
    );
    assert_eq!(
        stake(&mut pool, &mut a, &signer(MALLORY), 10),
        Err(AnchorError::ConstraintHasOne)
    );
    let mut unsigned = signer(ADMIN);
    unsigned.is_signer = false;
    assert_eq!(
        unstake(&mut pool, &mut a, &unsigned, 1),
        Err(AnchorError::ConstraintSigner)
    );
    assert_eq!(
        unstake(&mut pool, &mut a, &signer(ADMIN), 11),
        Err(AnchorError::InsufficientStake)
    );
    assert_eq!(
        stake(&mut pool, &mut a, &signer(ADMIN), u64::MAX),
        Err(AnchorError::MathOverflow)
    );
    assert_eq!((pool, a), (p0, a0), "failed calls change nothing");
}

#[test]
fn l3_3_rewards_with_nobody_staked_are_parked() {
    let mut pool = Pool::default();
    distribute(&mut pool, 50).unwrap();
    assert_eq!(pool.undistributed, 50);
    let mut a = staker(ADMIN);
    stake(&mut pool, &mut a, &signer(ADMIN), 10).unwrap();
    assert_eq!(
        pending(&pool, &a),
        Ok(0),
        "first staker does not scoop parked rewards"
    );
}

// ---- Property tests ------------------------------------------------------

proptest! {
    #[test]
    fn prop_canonical_bump_is_highest_valid(seed in proptest::collection::vec(any::<u8>(), 0..32)) {
        let seeds: &[&[u8]] = &[&seed];
        let (k, bump) = find_program_address(seeds, &PROGRAM).unwrap();
        prop_assert_eq!(all_valid_bumps(seeds)[0], (bump, k));
        prop_assert_eq!(check_seeds(seeds, &k, &PROGRAM), Ok(bump));
    }

    #[test]
    fn prop_load_rejects_random_bytes(data in proptest::collection::vec(any::<u8>(), 0..64)) {
        prop_assume!(data.len() < 8 || data[..8] != Config::discriminator());
        let mut info = AccountInfo::new(ADMIN, PROGRAM, 1, 0);
        info.data = data;
        prop_assert!(load_account::<Config>(&PROGRAM, &info).is_err());
    }

    #[test]
    fn prop_fee_cap_holds(fees in proptest::collection::vec(any::<u16>(), 1..10)) {
        let mut c = live_config(0);
        for f in fees {
            let r = set_fee(&PROGRAM, &mut c, &signer(ADMIN), f);
            prop_assert_eq!(r.is_ok(), f <= MAX_FEE_BPS);
            prop_assert!(load_account::<Config>(&PROGRAM, &c).unwrap().fee_bps <= MAX_FEE_BPS);
        }
    }

    // Integration: random stake/unstake/distribute sequences by two users.
    // The pool never pays out more rewards than it received, and stake is
    // always fully returned.
    #[test]
    fn prop_pool_is_solvent(
        ops in proptest::collection::vec((0u8..3, any::<bool>(), 0u64..1_000_000), 1..40),
    ) {
        let mut pool = Pool::default();
        let mut users = [staker(ADMIN), staker(MALLORY)];
        let (mut paid, mut received, mut staked_in, mut staked_out) = (0u128, 0u128, 0u128, 0u128);
        for (op, who, amt) in ops {
            let u = &mut users[who as usize];
            let s = signer(u.owner);
            match op {
                0 => if let Ok(p) = stake(&mut pool, u, &s, amt) { paid += p as u128; staked_in += amt as u128; },
                1 => if let Ok((p, a)) = unstake(&mut pool, u, &s, amt) { paid += p as u128; staked_out += a as u128; },
                _ => if distribute(&mut pool, amt).is_ok() { received += amt as u128; },
            }
        }
        for u in &mut users {
            let s = signer(u.owner);
            let amt = u.amount;
            let (p, a) = unstake(&mut pool, u, &s, amt).unwrap();
            paid += p as u128;
            staked_out += a as u128;
        }
        prop_assert!(paid <= received, "paid {} > received {}", paid, received);
        prop_assert_eq!(staked_in, staked_out);
        prop_assert_eq!(pool.total_staked, 0);
    }
}
