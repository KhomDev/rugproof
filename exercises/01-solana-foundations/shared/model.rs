// A pure-Rust model of the Solana account model, shared by every crate in
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

/// A subset of `solana_program::program_error::ProgramError`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ProgramError {
    #[error("missing required signature")]
    MissingRequiredSignature,
    #[error("account is not writable")]
    AccountNotWritable,
    #[error("account is owned by the wrong program")]
    IncorrectProgramId,
    #[error("not enough account keys")]
    NotEnoughAccountKeys,
    #[error("invalid instruction data")]
    InvalidInstructionData,
    #[error("invalid account data")]
    InvalidAccountData,
    #[error("account already initialized")]
    AccountAlreadyInitialized,
    #[error("account is not rent exempt")]
    AccountNotRentExempt,
    #[error("arithmetic overflow")]
    ArithmeticOverflow,
    #[error("signer is not the stored authority")]
    InvalidAuthority,
    #[error("runtime rule violated: {0}")]
    RuntimeViolation(&'static str),
}
