//! Reference solutions for guide 01 (Solana Foundations).
//! The account model lives in `src/model.rs`; every exercise here is a
//! check a real Solana program (or the runtime itself) has to perform.

mod model;
pub use model::*;

// ---------------------------------------------------------------- Level 1

/// Rent constants from the Solana runtime (`solana_program::rent`).
pub const ACCOUNT_STORAGE_OVERHEAD: u64 = 128;
pub const LAMPORTS_PER_BYTE_YEAR: u64 = 3_480;
pub const EXEMPTION_THRESHOLD_YEARS: u64 = 2;

/// 1.1 Rent: the minimum balance that makes an account with `data_len` bytes
/// rent exempt: `(128 + data_len) * 3480 * 2`. `None` on overflow.
pub fn rent_exempt_minimum(data_len: usize) -> Option<u64> {
    let len = u64::try_from(data_len).ok()?;
    ACCOUNT_STORAGE_OVERHEAD
        .checked_add(len)?
        .checked_mul(LAMPORTS_PER_BYTE_YEAR)?
        .checked_mul(EXEMPTION_THRESHOLD_YEARS)
}

/// 1.2 Ownership: may `program_id` debit lamports from or write data to
/// `account` in this instruction? Only the owner may, only if the account is
/// writable, and never an executable account.
pub fn can_modify(program_id: &Pubkey, account: &AccountInfo) -> bool {
    account.owner == *program_id && account.is_writable && !account.executable
}

/// 1.3 Signers: return `MissingRequiredSignature` unless `account` signed.
pub fn require_signer(account: &AccountInfo) -> Result<(), ProgramError> {
    if account.is_signer {
        Ok(())
    } else {
        Err(ProgramError::MissingRequiredSignature)
    }
}

// ---------------------------------------------------------------- Level 2

/// 2.1 Like `solana_program::account_info::next_account_info`: the next
/// account, or `NotEnoughAccountKeys` when the list runs out.
pub fn next_account<'a, I>(iter: &mut I) -> Result<&'a AccountInfo, ProgramError>
where
    I: Iterator<Item = &'a AccountInfo>,
{
    iter.next().ok_or(ProgramError::NotEnoughAccountKeys)
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
    match data {
        [0] => Ok(CounterInstruction::Initialize),
        [1, rest @ ..] => {
            let bytes: [u8; 8] = rest
                .try_into()
                .map_err(|_| ProgramError::InvalidInstructionData)?;
            Ok(CounterInstruction::Increment {
                by: u64::from_le_bytes(bytes),
            })
        }
        [2] => Ok(CounterInstruction::Close),
        _ => Err(ProgramError::InvalidInstructionData),
    }
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
    use ProgramError::RuntimeViolation as V;
    if before.len() != after.len() || before.iter().zip(after).any(|(b, a)| b.key != a.key) {
        return Err(ProgramError::InvalidAccountData);
    }
    let sum = |v: &[AccountInfo]| v.iter().map(|a| a.lamports as u128).sum::<u128>();
    if sum(before) != sum(after) {
        return Err(V("lamports not conserved"));
    }
    for (b, a) in before.iter().zip(after) {
        let unchanged = b.lamports == a.lamports
            && b.data == a.data
            && b.owner == a.owner
            && b.executable == a.executable;
        if b.executable && !unchanged {
            return Err(V("executable account modified"));
        }
        if !b.is_writable && !unchanged {
            return Err(V("read-only account modified"));
        }
        if b.owner != *program_id {
            if a.data != b.data {
                return Err(V("data of foreign account modified"));
            }
            if a.lamports < b.lamports {
                return Err(V("foreign account debited"));
            }
            if a.owner != b.owner {
                return Err(V("owner changed by non-owner"));
            }
        }
        if a.owner != b.owner && a.data.iter().any(|&x| x != 0) {
            return Err(V("owner change requires zeroed data"));
        }
    }
    Ok(())
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
    if account.owner != *program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    if account.data.len() != COUNTER_LEN || account.data[..8] != COUNTER_DISCRIMINATOR {
        return Err(ProgramError::InvalidAccountData);
    }
    Counter::try_from_slice(&account.data[8..]).map_err(|_| ProgramError::InvalidAccountData)
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
    let ix = decode_counter_ix(data)?;
    let needed = if ix == CounterInstruction::Close {
        3
    } else {
        2
    };
    if accounts.len() < needed {
        return Err(ProgramError::NotEnoughAccountKeys);
    }
    let (counter_acc, rest) = accounts.split_first_mut().expect("len checked");
    if counter_acc.owner != *program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    if !counter_acc.is_writable {
        return Err(ProgramError::AccountNotWritable);
    }
    require_signer(&rest[0])?;
    let authority = rest[0].key;

    match ix {
        CounterInstruction::Initialize => {
            if counter_acc.data.len() != COUNTER_LEN {
                return Err(ProgramError::InvalidAccountData);
            }
            if counter_acc.data.iter().any(|&b| b != 0) {
                return Err(ProgramError::AccountAlreadyInitialized);
            }
            let min = rent_exempt_minimum(COUNTER_LEN).ok_or(ProgramError::ArithmeticOverflow)?;
            if counter_acc.lamports < min {
                return Err(ProgramError::AccountNotRentExempt);
            }
            store_counter(
                counter_acc,
                &Counter {
                    authority,
                    count: 0,
                },
            );
        }
        CounterInstruction::Increment { by } => {
            let mut c = load_counter(program_id, counter_acc)?;
            if c.authority != authority {
                return Err(ProgramError::InvalidAuthority);
            }
            c.count = c
                .count
                .checked_add(by)
                .ok_or(ProgramError::ArithmeticOverflow)?;
            store_counter(counter_acc, &c);
        }
        CounterInstruction::Close => {
            let c = load_counter(program_id, counter_acc)?;
            if c.authority != authority {
                return Err(ProgramError::InvalidAuthority);
            }
            let dest = &mut rest[1];
            if dest.key == counter_acc.key {
                return Err(ProgramError::InvalidAccountData);
            }
            if !dest.is_writable {
                return Err(ProgramError::AccountNotWritable);
            }
            dest.lamports = dest
                .lamports
                .checked_add(counter_acc.lamports)
                .ok_or(ProgramError::ArithmeticOverflow)?;
            counter_acc.lamports = 0;
            counter_acc.data.iter_mut().for_each(|b| *b = 0);
            counter_acc.owner = SYSTEM_PROGRAM_ID;
        }
    }
    Ok(())
}
