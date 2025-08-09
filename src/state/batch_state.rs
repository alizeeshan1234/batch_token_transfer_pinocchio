use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct BatchState {
    pub batch_id: u64,
    pub authority: Pubkey,
    pub mint: Pubkey,
    pub total_recipients: u8,
    pub processed_count: u8,
    pub is_completed: u8,
    pub bump: u8,
}

impl BatchState {
    pub const LEN: usize = core::mem::size_of::<BatchState>();

    pub fn from_account_info(accounts: &AccountInfo) -> Result<&Self, ProgramError> {
        if accounts.data_len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(unsafe {
            &*(accounts.try_borrow_data()?.as_ptr() as *const Self)
        })
    }

    pub fn from_account_info_mut(accounts: &AccountInfo) -> Result<&mut Self, ProgramError> {
        if accounts.data_len() < Self::LEN {
            return Err(ProgramError::InvalidAccountData);
        }

        Ok(unsafe {
            &mut *(accounts.try_borrow_mut_data()?.as_mut_ptr() as *mut Self)
        })
    }

    pub fn is_completed(&self) -> bool {
        self.is_completed != 0
    }

    pub fn mark_completed(&mut self) {
        self.is_completed = 1;
    }

    pub fn increment_processed(&mut self) {
        self.processed_count = self.processed_count.saturating_add(1);
        if self.processed_count >= self.total_recipients {
            self.mark_completed();
        }
    }

    pub fn validate_authority(&self, expected_authority: &Pubkey) -> Result<(), ProgramError> {
        if self.authority != *expected_authority {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }

    pub fn validate_mint(&self, expected_mint: &Pubkey) -> Result<(), ProgramError> {
        if self.mint != *expected_mint {
            return Err(ProgramError::InvalidAccountData);
        }
        Ok(())
    }
}