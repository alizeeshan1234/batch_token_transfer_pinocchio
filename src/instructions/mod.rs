pub mod init_batch_state;
pub use init_batch_state::*;

pub mod batch_transfer;
pub use batch_transfer::*;
use pinocchio::program_error::ProgramError;

#[repr(u8)]
pub enum BatchTransferInstructions {
    InitBatchTransfer = 0,
    ProcessBatchTransfer = 1,
}

impl TryFrom<&u8> for BatchTransferInstructions {
    type Error = ProgramError;

    fn try_from(value: &u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(BatchTransferInstructions::InitBatchTransfer),
            1 => Ok(BatchTransferInstructions::ProcessBatchTransfer),
            _ => return Err(ProgramError::InvalidInstructionData)
        }
    }
}