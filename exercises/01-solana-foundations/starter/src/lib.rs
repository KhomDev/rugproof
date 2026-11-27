#![allow(unused_variables)]
//! Starter code for guide 01 (Solana Foundations).
//! Replace every `todo!()` and run `cargo test -p starter`.
//! The account model is in `../shared/model.rs`. Stuck? Reference answers are in `../solutions`.

include!("../../shared/model.rs");

// ---------------------------------------------------------------- Level 1

/// Rent constants from the Solana runtime (`solana_program::rent`).
pub const ACCOUNT_STORAGE_OVERHEAD: u64 = 128;
pub const LAMPORTS_PER_BYTE_YEAR: u64 = 3_480;
pub const EXEMPTION_THRESHOLD_YEARS: u64 = 2;

/// 1.1 Rent: the minimum balance that makes an account with `data_len` bytes
/// rent exempt: `(128 + data_len) * 3480 * 2`. `None` on overflow.
pub fn rent_exempt_minimum(data_len: usize) -> Option<u64> {
    todo!("exercise: rent_exempt_minimum")
}

/// 1.2 Ownership: may `program_id` debit lamports from or write data to
/// `account` in this instruction? Only the owner may, only if the account is
/// writable, and never an executable account.
pub fn can_modify(program_id: &Pubkey, account: &AccountInfo) -> bool {
    todo!("exercise: can_modify")
}

/// 1.3 Signers: return `MissingRequiredSignature` unless `account` signed.
pub fn require_signer(account: &AccountInfo) -> Result<(), ProgramError> {
    todo!("exercise: require_signer")
}

// ---------------------------------------------------------------- Level 2

/// 2.1 Like `solana_program::account_info::next_account_info`: the next
/// account, or `NotEnoughAccountKeys` when the list runs out.
pub fn next_account<'a, I>(iter: &mut I) -> Result<&'a AccountInfo, ProgramError>
where
    I: Iterator<Item = &'a AccountInfo>,
{
    todo!("exercise: next_account")
}

/// Instructions of the counter program built in Level 3.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CounterInstruction {
    Initialize,
    Increment { by: u64 },
    Close,
}

/// 2.2 Decode `[tag, ...payload]`: 0 = Initialize, 1 = Increment (8-byte
/// little-endian u64), 2 = Close. Wrong lengths are errors. Never panics.
pub fn decode_counter_ix(data: &[u8]) -> Result<CounterInstruction, ProgramError> {
    todo!("exercise: decode_counter_ix")
}

/// 2.3 The runtime's account rules, checked after a program ran.
/// `before` and `after` are the same accounts (same keys, same order).
///
/// * the total lamports must not change (no minting, no burning);
/// * an executable account must not change at all;
/// * a read-only account must not change at all;
/// * an account the program does not own: data and owner unchanged, and
///   lamports may only go up (anyone can credit, only the owner can debit);
/// * an owner change needs the data to be all zeroes.
pub fn check_transition(
    program_id: &Pubkey,
    before: &[AccountInfo],
    after: &[AccountInfo],
) -> Result<(), ProgramError> {
    todo!("exercise: check_transition")
}

// ---------------------------------------------------------------- Level 3

/// State of the counter program. Stored as `COUNTER_DISCRIMINATOR` + Borsh.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct Counter {
    pub authority: Pubkey,
    pub count: u64,
}

pub const COUNTER_DISCRIMINATOR: [u8; 8] = *b"counter_";
pub const COUNTER_LEN: usize = 8 + 32 + 8;

/// 3.1 Load a `Counter` safely: the account must be owned by `program_id`,
/// exactly `COUNTER_LEN` bytes, and start with `COUNTER_DISCRIMINATOR`.
pub fn load_counter(program_id: &Pubkey, account: &AccountInfo) -> Result<Counter, ProgramError> {
    todo!("exercise: load_counter")
}

/// Helper (given): write `COUNTER_DISCRIMINATOR` + Borsh bytes into the account.
pub fn store_counter(account: &mut AccountInfo, counter: &Counter) {
    let mut out = COUNTER_DISCRIMINATOR.to_vec();
    counter.serialize(&mut out).expect("Vec writer cannot fail");
    account.data = out;
}

/// 3.2 The whole counter program. Validate everything first, then write, so
/// a failed instruction leaves every account unchanged.
///
/// * `Initialize`: `[counter (writable, owned by us, COUNTER_LEN zero bytes,
///   rent exempt), authority (signer)]`.
/// * `Increment { by }`: `[counter (writable), authority (signer, stored)]`.
/// * `Close`: `[counter (writable), authority (signer, stored),
///   destination (writable, not the counter)]`. All lamports go to the
///   destination, data is zeroed and the account is handed back to the
///   System Program so it cannot be "revived" by a later refund.
pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &mut [AccountInfo],
    data: &[u8],
) -> Result<(), ProgramError> {
    todo!("exercise: process_instruction")
}
