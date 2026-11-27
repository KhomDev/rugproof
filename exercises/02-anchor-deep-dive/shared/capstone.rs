// Capstone test suite: included by both `capstone` (vulnerable, your work)
// and `capstone-solution` (patched). `cap` is the crate under test.
//
// honest_*  : normal use. Passes on both crates.
// exploit_* : the attacker's moves, run locally. Fails on the vulnerable
//             crate ("EXPLOIT SUCCEEDED") and passes once the bug is fixed.
use cap::*;
use proptest::prelude::*;

const PROGRAM: Pubkey = Pubkey::new_from_byte(0xAA);
const ADMIN: Pubkey = Pubkey::new_from_byte(1);
const ALICE: Pubkey = Pubkey::new_from_byte(2);
const MALLORY: Pubkey = Pubkey::new_from_byte(66);
const PER_CLAIM: u64 = 100;
const POOL: u64 = 10_000;

fn dist() -> Distributor {
    Distributor {
        admin: ADMIN,
        per_claim: PER_CLAIM,
        remaining: POOL,
    }
}

fn signer(key: Pubkey) -> AccountInfo {
    AccountInfo::new(key, SYSTEM_PROGRAM_ID, 0, 0).signer()
}

fn seeds_for(user: &Pubkey) -> [Vec<u8>; 2] {
    [RECEIPT_SEED.to_vec(), user.0.to_vec()]
}

/// Every bump that yields a valid receipt address for `user`, canonical first.
fn valid_bumps(user: &Pubkey) -> Vec<(u8, Pubkey)> {
    let s = seeds_for(user);
    let seeds: Vec<&[u8]> = s.iter().map(|v| v.as_slice()).collect();
    (0..=255u8)
        .rev()
        .filter_map(|b| {
            create_program_address(&seeds, b, &PROGRAM)
                .ok()
                .map(|k| (b, k))
        })
        .collect()
}

/// A not-yet-created account at `key` (what `init` expects).
fn blank(key: Pubkey) -> AccountInfo {
    AccountInfo::new(key, SYSTEM_PROGRAM_ID, 0, 0).writable()
}

// ---- honest use ----------------------------------------------------------

#[test]
fn honest_claim_once() {
    let mut d = dist();
    let (bump, key) = valid_bumps(&ALICE)[0];
    let mut receipt = blank(key);
    let mut bal = 0;
    claim(
        &PROGRAM,
        &mut d,
        &mut receipt,
        &signer(ALICE),
        bump,
        &mut bal,
    )
    .unwrap();
    assert_eq!(
        (bal, d.remaining, receipt.owner),
        (PER_CLAIM, POOL - PER_CLAIM, PROGRAM)
    );
    // Same receipt again: init on a live account fails.
    assert_eq!(
        claim(
            &PROGRAM,
            &mut d,
            &mut receipt,
            &signer(ALICE),
            bump,
            &mut bal
        ),
        Err(AnchorError::AccountAlreadyInUse)
    );
    assert_eq!(bal, PER_CLAIM);
}

#[test]
fn honest_bad_inputs_are_refused() {
    let mut d = dist();
    let (bump, key) = valid_bumps(&ALICE)[0];
    let mut bal = 0;
    let mut unsigned = signer(ALICE);
    unsigned.is_signer = false;
    assert_eq!(
        claim(&PROGRAM, &mut d, &mut blank(key), &unsigned, bump, &mut bal),
        Err(AnchorError::ConstraintSigner)
    );
    // Alice's receipt address used with Mallory's signature.
    assert_eq!(
        claim(
            &PROGRAM,
            &mut d,
            &mut blank(key),
            &signer(MALLORY),
            bump,
            &mut bal
        ),
        Err(AnchorError::ConstraintSeeds)
    );
    assert_eq!(
        claim(
            &PROGRAM,
            &mut d,
            &mut blank(MALLORY),
            &signer(ALICE),
            bump,
            &mut bal
        ),
        Err(AnchorError::ConstraintSeeds)
    );
    assert_eq!((bal, d.remaining), (0, POOL));
}

#[test]
fn honest_pool_runs_dry() {
    let mut d = Distributor {
        remaining: PER_CLAIM - 1,
        ..dist()
    };
    let (bump, key) = valid_bumps(&ALICE)[0];
    let mut bal = 0;
    assert_eq!(
        claim(
            &PROGRAM,
            &mut d,
            &mut blank(key),
            &signer(ALICE),
            bump,
            &mut bal
        ),
        Err(AnchorError::MathOverflow)
    );
}

#[test]
fn honest_admin_sweeps_the_rest() {
    let mut d = dist();
    let mut bal = 0;
    assert_eq!(
        withdraw_unclaimed(&mut d, &signer(ADMIN), &mut bal),
        Ok(POOL)
    );
    assert_eq!((bal, d.remaining), (POOL, 0));
    let mut unsigned = signer(ADMIN);
    unsigned.is_signer = false;
    assert_eq!(
        withdraw_unclaimed(&mut dist(), &unsigned, &mut 0),
        Err(AnchorError::ConstraintSigner)
    );
}

proptest! {
    /// Distinct wallets each claim once with the canonical bump: tokens are conserved.
    #[test]
    fn honest_prop_many_users_conserve_tokens(users in proptest::collection::btree_set(3u8..60, 1..20)) {
        let mut d = dist();
        let mut total = 0u64;
        for u in users {
            let user = Pubkey::new_from_byte(u);
            let (bump, key) = valid_bumps(&user)[0];
            let mut bal = 0;
            claim(&PROGRAM, &mut d, &mut blank(key), &signer(user), bump, &mut bal).unwrap();
            total += bal;
        }
        prop_assert_eq!(total + d.remaining, POOL);
    }
}

// ---- the attacker's moves --------------------------------------------------

/// Mallory claims once per valid bump, creating a new receipt each time.
#[test]
fn exploit_non_canonical_bumps_do_not_allow_double_claims() {
    let bumps = valid_bumps(&MALLORY);
    assert!(bumps.len() >= 2, "test needs at least two valid bumps");
    let mut d = dist();
    let mut bal = 0u64;
    for (bump, key) in bumps {
        let _ = claim(
            &PROGRAM,
            &mut d,
            &mut blank(key),
            &signer(MALLORY),
            bump,
            &mut bal,
        );
    }
    assert!(
        bal <= PER_CLAIM,
        "EXPLOIT SUCCEEDED: Mallory claimed {bal}, her share is {PER_CLAIM}"
    );
}

/// Mallory, any signer at all, calls the admin sweep.
#[test]
fn exploit_non_admin_cannot_sweep() {
    let mut d = dist();
    let mut bal = 0;
    let res = withdraw_unclaimed(&mut d, &signer(MALLORY), &mut bal);
    assert!(
        res.is_err() && bal == 0 && d.remaining == POOL,
        "EXPLOIT SUCCEEDED: Mallory swept {bal} ({res:?})"
    );
}

proptest! {
    /// Any order and subset of bump choices: one wallet, one share.
    #[test]
    fn exploit_prop_one_share_per_wallet(picks in proptest::collection::vec(any::<prop::sample::Index>(), 1..12)) {
        let bumps = valid_bumps(&MALLORY);
        let mut d = dist();
        let mut bal = 0u64;
        let mut receipts: std::collections::HashMap<Pubkey, AccountInfo> = Default::default();
        for p in picks {
            let (bump, key) = bumps[p.index(bumps.len())];
            let r = receipts.entry(key).or_insert_with(|| blank(key));
            let _ = claim(&PROGRAM, &mut d, r, &signer(MALLORY), bump, &mut bal);
        }
        prop_assert!(bal <= PER_CLAIM, "EXPLOIT SUCCEEDED: Mallory claimed {}", bal);
        prop_assert_eq!(bal + d.remaining, POOL);
    }
}
