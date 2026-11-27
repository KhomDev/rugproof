// Shared test suite: included by both `starter` and `solutions`.
// `ex` is the crate under test.
use ex::*;
use proptest::prelude::*;
use std::cell::RefCell;

fn acct(b: u8, lamports: u64) -> Account {
    Account { owner: Pubkey::new_from_byte(b), lamports }
}

// ---- Level 1 -------------------------------------------------------------

#[test]
fn l1_1_longest() {
    assert_eq!(longest("vault", "pool"), "vault");
    assert_eq!(longest("ab", "abc"), "abc");
    assert_eq!(longest("ab", "cd"), "ab", "ties return the first");
}

#[test]
fn l1_2_total_lamports() {
    assert_eq!(total_lamports(&[]), Some(0));
    assert_eq!(total_lamports(&[acct(1, 5), acct(2, 7)]), Some(12));
    assert_eq!(total_lamports(&[acct(1, u64::MAX), acct(2, 1)]), None, "overflow must be caught");
}

#[test]
fn l1_3_transfer_ok_and_errors() {
    let (mut a, mut b) = (acct(1, 10), acct(2, 0));
    transfer(&mut a, &mut b, 4).unwrap();
    assert_eq!((a.lamports, b.lamports), (6, 4));

    let err = transfer(&mut a, &mut b, 7).unwrap_err();
    assert_eq!(err, VaultError::InsufficientFunds { have: 6, need: 7 });
    assert_eq!((a.lamports, b.lamports), (6, 4), "failed transfer must not change state");

    let mut full = acct(3, u64::MAX);
    assert_eq!(transfer(&mut a, &mut full, 1), Err(VaultError::Overflow));
    assert_eq!(a.lamports, 6, "overflow on credit must not debit");
}

#[test]
fn l1_3_error_messages() {
    let e = VaultError::InsufficientFunds { have: 1, need: 2 };
    assert_eq!(e.to_string(), "insufficient funds: have 1, need 2");
}

// ---- Level 2 -------------------------------------------------------------

#[test]
fn l2_1_decode_valid() {
    assert_eq!(decode_instruction(&[0]), Ok(VaultInstruction::Initialize));
    let mut d = vec![1];
    d.extend_from_slice(&500u64.to_le_bytes());
    assert_eq!(decode_instruction(&d), Ok(VaultInstruction::Deposit { amount: 500 }));
    d[0] = 2;
    assert_eq!(decode_instruction(&d), Ok(VaultInstruction::Withdraw { amount: 500 }));
}

#[test]
fn l2_1_decode_adversarial() {
    for bad in [&[][..], &[0, 0], &[1, 1, 2, 3], &[2; 10], &[3], &[255; 9]] {
        assert_eq!(decode_instruction(bad), Err(VaultError::InvalidInstruction), "input {bad:?}");
    }
}

#[test]
fn l2_2_top_holders() {
    let v = [acct(1, 5), acct(2, 0), acct(3, 9), acct(4, 5), acct(5, 1)];
    let p = |b| Pubkey::new_from_byte(b);
    assert_eq!(top_holders(&v, 3), vec![p(3), p(1), p(4)]);
    assert_eq!(top_holders(&v, 0), vec![]);
    assert_eq!(top_holders(&v, 99).len(), 4, "zero balances are skipped");
}

#[test]
fn l2_3_borsh_len() {
    // 32 bytes for the owner + 8 for the u64. Borsh adds no padding or type tags.
    assert_eq!(borsh_len(&acct(1, 1)), 40);
}

// ---- Level 3 -------------------------------------------------------------

#[test]
fn l3_1_store_load_roundtrip() {
    let v = Vault { authority: Pubkey::new_from_byte(9), balance: 42 };
    let bytes = store(&v);
    assert_eq!(&bytes[..8], b"vault___");
    assert_eq!(bytes.len(), 8 + 32 + 8);
    assert_eq!(load::<Vault>(&bytes), Ok(v));
}

#[test]
fn l3_1_type_cosplay_is_rejected() {
    // Vault and Config have the same Borsh shape (Pubkey + u64).
    // Without the discriminator a Config could be loaded as a Vault.
    let c = Config { admin: Pubkey::new_from_byte(7), fee_bps: 10_000 };
    let bytes = store(&c);
    assert_eq!(load::<Vault>(&bytes), Err(VaultError::WrongAccountType));
    assert_eq!(load::<Vault>(&[]), Err(VaultError::WrongAccountType));
    let mut long = store(&Vault { authority: Pubkey::new_from_byte(1), balance: 1 });
    long.push(0);
    assert!(load::<Vault>(&long).is_err(), "trailing bytes must be rejected");
}

#[test]
fn l3_2_share_math_examples() {
    assert_eq!(deposit_shares(100, 0, 0), Some(100));
    assert_eq!(deposit_shares(10, 30, 10), Some(3)); // 3.33 rounds down
    assert_eq!(withdraw_assets(3, 31, 10), Some(9)); // 9.3 rounds down
    assert_eq!(withdraw_assets(11, 100, 10), None);
    assert_eq!(deposit_shares(1, 0, 5), None);
    assert_eq!(deposit_shares(u64::MAX, 1, u64::MAX), None, "result above u64::MAX");
}

#[test]
fn l3_3_move_lamports() {
    let (a, b) = (RefCell::new(10), RefCell::new(0));
    move_lamports(&a, &b, 3).unwrap();
    assert_eq!((*a.borrow(), *b.borrow()), (7, 3));
    assert_eq!(move_lamports(&a, &a, 1), Err(VaultError::DuplicateAccount));
    assert_eq!(*a.borrow(), 7);
    let _held = b.borrow(); // someone else holds a borrow: must error, not panic
    assert!(move_lamports(&a, &b, 1).is_err());
    assert_eq!(*a.borrow(), 7);
}

// ---- Property tests ------------------------------------------------------

proptest! {
    #[test]
    fn prop_decode_never_panics(data in proptest::collection::vec(any::<u8>(), 0..64)) {
        let _ = decode_instruction(&data);
    }

    #[test]
    fn prop_transfer_conserves(a in any::<u64>(), b in any::<u64>(), amt in any::<u64>()) {
        let (mut x, mut y) = (acct(1, a), acct(2, b));
        let before = a as u128 + b as u128;
        let _ = transfer(&mut x, &mut y, amt);
        prop_assert_eq!(x.lamports as u128 + y.lamports as u128, before);
    }

    #[test]
    fn prop_load_rejects_random_bytes(data in proptest::collection::vec(any::<u8>(), 0..64)) {
        prop_assume!(data.len() < 8 || data[..8] != *b"vault___");
        prop_assert_eq!(load::<Vault>(&data), Err(VaultError::WrongAccountType));
    }

    // The vault never pays out more than was deposited (rounding favours the vault).
    #[test]
    fn prop_no_free_money(
        seed_assets in 1u64..1_000_000_000_000,
        seed_shares in 1u64..1_000_000_000_000,
        dep in 1u64..1_000_000_000_000,
    ) {
        if let Some(s) = deposit_shares(dep, seed_assets, seed_shares) {
            let ta = seed_assets + dep;
            let ts = seed_shares + s;
            let out = withdraw_assets(s, ta, ts).unwrap();
            prop_assert!(out <= dep, "deposited {dep}, withdrew {out}");
        }
    }
}
