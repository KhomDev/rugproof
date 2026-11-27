// A pure-Rust model of the Solana account model and SPL Token state, shared by every crate in
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

/// Stand-in program and sysvar ids (toy values, not the real base58 keys).
pub const TOKEN_PROGRAM_ID: Pubkey = Pubkey::new_from_byte(0x70);
pub const TOKEN_2022_PROGRAM_ID: Pubkey = Pubkey::new_from_byte(0x72);
pub const INSTRUCTIONS_SYSVAR_ID: Pubkey = Pubkey::new_from_byte(0x51);
pub const CLOCK_SYSVAR_ID: Pubkey = Pubkey::new_from_byte(0x52);

/// The fields of an SPL Token mint that matter for these exercises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mint {
    pub decimals: u8,
    pub supply: u64,
    pub mint_authority: Option<Pubkey>,
    pub freeze_authority: Option<Pubkey>,
}

/// The fields of an SPL Token account that matter for these exercises.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenAccount {
    pub mint: Pubkey,
    /// The wallet (or PDA) that controls this token account.
    pub owner: Pubkey,
    pub amount: u64,
    pub delegate: Option<Pubkey>,
    pub delegated_amount: u64,
    pub is_frozen: bool,
}

/// Errors for every exercise in this workspace.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecurityError {
    #[error("account is owned by the wrong program")]
    IllegalOwner,
    #[error("the same account was passed twice")]
    DuplicateAccount,
    #[error("token account is for a different mint")]
    MintMismatch,
    #[error("token account has a different owner")]
    OwnerMismatch,
    #[error("decimals do not match the mint")]
    MintDecimalsMismatch,
    #[error("token account is frozen")]
    AccountFrozen,
    #[error("insufficient funds")]
    InsufficientFunds,
    #[error("missing required signature")]
    MissingRequiredSignature,
    #[error("signer is neither owner nor an approved delegate")]
    OwnerOrDelegateRequired,
    #[error("CPI target is not an allowed program")]
    ArbitraryCpi,
    #[error("account is not the expected sysvar")]
    InvalidSysvar,
    #[error("account already initialized")]
    AlreadyInitialized,
    #[error("account was closed")]
    AccountClosed,
    #[error("account not initialized")]
    Uninitialized,
    #[error("invalid argument")]
    InvalidArgument,
    #[error("arithmetic overflow")]
    Overflow,
}
