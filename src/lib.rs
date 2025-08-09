use pinocchio::{account_info::AccountInfo, entrypoint, pubkey::Pubkey,program_error::ProgramError, *};
use pinocchio_pubkey::declare_id;

use crate::instructions::{process_batch_transfer, process_initialize_batch_state, BatchTransferInstructions};

entrypoint!(process_instruction);

declare_id!("7B3prxsmARuNjdD5qa5CmDur1tPWbH4UNbZu1AVJCRJo");

mod state;
mod instructions;

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8]
) -> ProgramResult {

    let (ix_disc, instruction_data) = instruction_data.split_first().ok_or(ProgramError::InvalidInstructionData)?;

    match BatchTransferInstructions::try_from(ix_disc)? {
        BatchTransferInstructions::InitBatchTransfer => process_initialize_batch_state(accounts, instruction_data)?,
        BatchTransferInstructions::ProcessBatchTransfer => process_batch_transfer(accounts, instruction_data)?,
        _ => return Err(ProgramError::InvalidInstructionData)
    }

    Ok(())
}