use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

const MAX_RECIPIENTS: usize = 10;

#[derive(Clone, Copy, PartialEq)]
#[repr(C)]
pub struct BatchTransfer {
    pub batch_id: u64,
    pub num_recipients: u8,
    pub amount_per_recipient: u64,
    pub recipients: [Pubkey; MAX_RECIPIENTS],
}

impl BatchTransfer {
    pub const LEN: usize = core::mem::size_of::<Self>();

    pub fn new(batch_id: u64, amount_per_recipient: u64) -> Self {
        Self {
            batch_id,
            num_recipients: 0,
            amount_per_recipient,
            recipients: [Pubkey::default(); MAX_RECIPIENTS],
        }
    }

    pub fn add_recipient(&mut self, recipient: Pubkey) -> Result<(), ProgramError> {
        if self.num_recipients >= MAX_RECIPIENTS as u8 {
            return Err(ProgramError::InvalidInstructionData);
        }

        self.recipients[self.num_recipients as usize] = recipient;
        self.num_recipients += 1;
        Ok(())
    }

    pub fn get_active_recipients(&self) -> &[Pubkey] {
        &self.recipients[..self.num_recipients as usize]
    }

    pub fn total_amount_needed(&self) -> u64 {
        self.amount_per_recipient.saturating_mul(self.num_recipients as u64)
    }

    pub fn from_account_info(account: &AccountInfo) -> Result<&Self, ProgramError> {
        if account.data_len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        let data = account.try_borrow_data()?;
        Ok(unsafe { &*(data.as_ptr() as *const Self) })
    }

    pub fn from_account_info_mut(account: &AccountInfo) -> Result<&mut Self, ProgramError> {
        if account.data_len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        let mut data = account.try_borrow_mut_data()?;
        Ok(unsafe { &mut *(data.as_mut_ptr() as *mut Self) })
    }
}