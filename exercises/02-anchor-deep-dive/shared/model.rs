// A pure-Rust model of what an Anchor program sees, shared by every crate in
// this workspace through `include!`. Nothing here talks to a validator or a
// network: it is the smallest set of types that lets the exercises express
// the same checks a real program must write.

pub use borsh::{BorshDeserialize, BorshSerialize};
use thiserror::Error;

/// A 32-byte address, like `solana_program::pubkey::Pubkey`.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    BorshSerialize,
    BorshDeserialize,
)]
pub struct Pubkey(pub [u8; 32]);

impl Pubkey {
    pub const fn new_from_byte(b: u8) -> Self {
        Pubkey([b; 32])
    }
}

/// The System Program owns every plain wallet (all-zero id, as on Solana).
pub const SYSTEM_PROGRAM_ID: Pubkey = Pubkey([0; 32]);

/// What a program sees for each account in an instruction.
/// The runtime fills `is_signer` / `is_writable` from the transaction;
/// everything else comes from the account itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountInfo {
    pub key: Pubkey,
    pub owner: Pubkey,
    pub lamports: u64,
    pub data: Vec<u8>,
    pub is_signer: bool,
    pub is_writable: bool,
    pub executable: bool,
}

impl AccountInfo {
    /// A read-only, non-signer account with `data_len` zero bytes.
    pub fn new(key: Pubkey, owner: Pubkey, lamports: u64, data_len: usize) -> Self {
        AccountInfo {
            key,
            owner,
            lamports,
            data: vec![0; data_len],
            is_signer: false,
            is_writable: false,
            executable: false,
        }
    }

    /// Mark the account as having signed the transaction.
    pub fn signer(mut self) -> Self {
        self.is_signer = true;
        self
    }

    /// Mark the account as writable in the transaction.
    pub fn writable(mut self) -> Self {
        self.is_writable = true;
        self
    }
}

/// The Anchor errors these exercises need (`anchor_lang::error::ErrorCode`
/// numbers in brackets), plus this program's own `#[error_code]` errors
/// (Anchor numbers those from 6000).
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum AnchorError {
    #[error("a has_one constraint was violated [2001]")]
    ConstraintHasOne,
    #[error("a signer constraint was violated [2002]")]
    ConstraintSigner,
    #[error("a seeds constraint was violated [2006]")]
    ConstraintSeeds,
    #[error("no 8 byte discriminator was found on the account [3001]")]
    AccountDiscriminatorNotFound,
    #[error("8 byte discriminator did not match what was expected [3002]")]
    AccountDiscriminatorMismatch,
    #[error("failed to deserialize the account [3003]")]
    AccountDidNotDeserialize,
    #[error("the given account is owned by a different program [3007]")]
    AccountOwnedByWrongProgram,
    #[error("the account is already in use (init on a live account)")]
    AccountAlreadyInUse,
    #[error("seeds are invalid for a program address (off-curve search failed)")]
    InvalidSeeds,
    #[error("a seed is longer than 32 bytes, or there are more than 16 seeds")]
    MaxSeedLengthExceeded,
    #[error("unauthorized [6000]")]
    Unauthorized,
    #[error("fee above the maximum [6001]")]
    FeeTooHigh,
    #[error("math overflow [6002]")]
    MathOverflow,
    #[error("not enough staked [6003]")]
    InsufficientStake,
}
