//! Capstone 01, patched. Compare with `../capstone/src/lib.rs`.
//! Each `// FIX` block is a check the vulnerable version forgot.

include!("../../shared/model.rs");

/// Stored in a program-owned account: who may withdraw from which treasury.
#[derive(Debug, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
pub struct VaultState {
    pub authority: Pubkey,
    pub treasury: Pubkey,
}

pub const STATE_TAG: [u8; 8] = *b"vstate__";
pub const STATE_LEN: usize = 8 + 32 + 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreasuryInstruction {
    /// `[state (writable, ours, zeroed), treasury (ours), authority (signer)]`
    Initialize,
    /// `[state, treasury (writable), authority (signer), destination (writable)]`
    Withdraw { amount: u64 },
}

pub fn decode(data: &[u8]) -> Result<TreasuryInstruction, ProgramError> {
    match data {
        [0] => Ok(TreasuryInstruction::Initialize),
        [1, rest @ ..] => Ok(TreasuryInstruction::Withdraw {
            amount: u64::from_le_bytes(
                rest.try_into()
                    .map_err(|_| ProgramError::InvalidInstructionData)?,
            ),
        }),
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn load_state(program_id: &Pubkey, acc: &AccountInfo) -> Result<VaultState, ProgramError> {
    // FIX 2 (owner check): only state written by this program can be trusted.
    // Anyone can create an account with these exact bytes under their own program.
    if acc.owner != *program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    // END FIX
    if acc.data.len() != STATE_LEN || acc.data[..8] != STATE_TAG {
        return Err(ProgramError::InvalidAccountData);
    }
    VaultState::try_from_slice(&acc.data[8..]).map_err(|_| ProgramError::InvalidAccountData)
}

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &mut [AccountInfo],
    data: &[u8],
) -> Result<(), ProgramError> {
    match decode(data)? {
        TreasuryInstruction::Initialize => {
            let [state, treasury, authority, ..] = accounts else {
                return Err(ProgramError::NotEnoughAccountKeys);
            };
            if state.owner != *program_id || treasury.owner != *program_id {
                return Err(ProgramError::IncorrectProgramId);
            }
            if !state.is_writable {
                return Err(ProgramError::AccountNotWritable);
            }
            if !authority.is_signer {
                return Err(ProgramError::MissingRequiredSignature);
            }
            if state.data.len() != STATE_LEN {
                return Err(ProgramError::InvalidAccountData);
            }
            if state.data.iter().any(|&b| b != 0) {
                return Err(ProgramError::AccountAlreadyInitialized);
            }
            let mut out = STATE_TAG.to_vec();
            VaultState {
                authority: authority.key,
                treasury: treasury.key,
            }
            .serialize(&mut out)
            .expect("Vec writer cannot fail");
            state.data = out;
            Ok(())
        }
        TreasuryInstruction::Withdraw { amount } => {
            let [state, treasury, authority, destination, ..] = accounts else {
                return Err(ProgramError::NotEnoughAccountKeys);
            };
            let s = load_state(program_id, state)?;
            if s.treasury != treasury.key {
                return Err(ProgramError::InvalidAccountData);
            }
            if s.authority != authority.key {
                return Err(ProgramError::InvalidAuthority);
            }
            // FIX 1 (signer check): matching the stored key proves nothing;
            // anyone can put Alice's public key in a transaction.
            if !authority.is_signer {
                return Err(ProgramError::MissingRequiredSignature);
            }
            // END FIX
            if !treasury.is_writable || !destination.is_writable {
                return Err(ProgramError::AccountNotWritable);
            }
            if treasury.key == destination.key {
                return Err(ProgramError::InvalidAccountData);
            }
            let new_treasury = treasury
                .lamports
                .checked_sub(amount)
                .ok_or(ProgramError::ArithmeticOverflow)?;
            let new_dest = destination
                .lamports
                .checked_add(amount)
                .ok_or(ProgramError::ArithmeticOverflow)?;
            treasury.lamports = new_treasury;
            destination.lamports = new_dest;
            Ok(())
        }
    }
}
