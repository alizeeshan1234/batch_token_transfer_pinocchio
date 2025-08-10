use pinocchio::{account_info::AccountInfo, program_error::ProgramError, sysvars::{rent::Rent, Sysvar}, *};
use pinocchio_system::instructions::CreateAccount;

use pinocchio_token::{instructions::InitializeAccount3, state::TokenAccount};

use crate::state::BatchState;

pub fn process_initialize_batch_state(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {

    if accounts.len() < 6 {
        return Err(ProgramError::InvalidAccountData);
    };

    let [creator, mint, batch_state_account, creator_token_account, system_program, token_program] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if instruction_data.len() < 9 {
        return Err(ProgramError::InvalidInstructionData);
    };

    let batch_id = u64::from_le_bytes(
        instruction_data[0..8].try_into().map_err(|_| ProgramError::InvalidInstructionData)?
    );

    let total_recipients = instruction_data[8];

    if !creator.is_signer() {
        return Err(ProgramError::InvalidAccountData);
    };

    if *mint.owner() != pinocchio_token::ID {
        return Err(ProgramError::InvalidAccountOwner);
    };

    let (batch_state_pda, bump) = pubkey::find_program_address(
        &[b"batch_state", creator.key().as_ref(), mint.key().as_ref(), &batch_id.to_le_bytes()],
        &crate::ID
    );

    let (creator_token_account_pda, bump2) = pubkey::find_program_address(
        &[b"creator_token_account", creator.key().as_ref(), mint.key().as_ref()],
        &crate::ID
    );

    if *creator_token_account.key() != creator_token_account_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    if *batch_state_account.key() != batch_state_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    if batch_state_account.data_len() == 0 {
        msg!("Creating batch state account");

        let lamports = Rent::get()?.minimum_balance(BatchState::LEN);

        CreateAccount {
            from: creator,
            to: batch_state_account,
            lamports,
            space: BatchState::LEN as u64,
            owner: &crate::ID
        }.invoke()?;

        let mut batch_state_account_mut = BatchState::from_account_info_mut(batch_state_account)?;
        batch_state_account_mut.batch_id = batch_id;
        batch_state_account_mut.authority = *creator.key();
        batch_state_account_mut.mint = *mint.key();
        batch_state_account_mut.total_recipients = total_recipients;
        batch_state_account_mut.processed_count = 0;
        batch_state_account_mut.is_completed = 0; //False
        batch_state_account_mut.bump = bump;

        msg!("Batch state initialized");
    } else {
        return Err(ProgramError::AccountAlreadyInitialized);
    };

    if creator_token_account.data_len() == 0  {

        msg!("Creating creator token account");

        let lamports = Rent::get()?.minimum_balance(TokenAccount::LEN);

        CreateAccount {
            from: creator,
            to: creator_token_account,
            lamports: lamports,
            space: TokenAccount::LEN as u64,
            owner: &pinocchio_token::id()
        }.invoke()?;

        InitializeAccount3 {
            account: creator_token_account,
            mint,
            owner: creator.key(),
        }.invoke()?;

        msg!("Creator token account initialized");

    } else {
        let token_account_data = TokenAccount::from_account_info(creator_token_account)?;
        if *token_account_data.mint() != *mint.key() || *token_account_data.owner() != *creator.key() {
            return Err(ProgramError::InvalidAccountData);
        }

        msg!("Creator token account already exists and is valid");
    };

    Ok(())
}

//------------------- TESTING process_initialize_batch_state -------------------

#[cfg(test)]
mod testing {
   use super::*;
    use mollusk_svm::{program, Mollusk, result::Check};
    use solana_sdk::{
        account::Account,
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        pubkey,
    };

    const PROGRAM_ID: Pubkey = pubkey!("7B3prxsmARuNjdD5qa5CmDur1tPWbH4UNbZu1AVJCRJo");
    const CREATOR: Pubkey = Pubkey::new_from_array([1u8; 32]);
    const BATCH_ID: u64 = 1;
    const MINT: Pubkey = Pubkey::new_from_array([2u8; 32]);

  #[test]
fn test_process_initialize_batch_transfer() {
    let mollusk = Mollusk::new(&PROGRAM_ID, "target/deploy/batch_token_transfer");

    let (batch_state_pda, bump) = Pubkey::find_program_address(
        &[b"batch_state", CREATOR.as_ref(), MINT.as_ref(), &BATCH_ID.to_le_bytes()],
        &PROGRAM_ID
    );

    let (creator_token_account_pda, bump2) = Pubkey::find_program_address(
        &[b"creator_token_account", CREATOR.as_ref(), MINT.as_ref()],
        &PROGRAM_ID
    );

    let (system_program_id, system_account) = program::keyed_account_for_system_program();
    
    // Add token program
    let token_program_id = pinocchio_token::ID;
    let token_program_account = Account {
        lamports: 0,
        data: vec![],
        owner: solana_sdk::native_loader::id(),
        executable: true,
        rent_epoch: 0,
    };

     // Most programs expect an instruction discriminator first
    // You'll need to check your instruction enum to see what discriminator to use
    // This assumes InitializeBatchState is the first variant (discriminator = 0)
    let mut instruction_data = vec![0u8; 17];
    instruction_data[0] = 0; // instruction discriminator for InitializeBatchState
    instruction_data[1..9].copy_from_slice(&BATCH_ID.to_le_bytes());
    instruction_data[9] = 2; // total_recipients

    let instruction = Instruction {
        program_id: PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(CREATOR, true),
            AccountMeta::new(MINT, false),
            AccountMeta::new(batch_state_pda, false),
            AccountMeta::new(creator_token_account_pda, false),
            AccountMeta::new_readonly(system_program_id, false), // Add system program
            AccountMeta::new_readonly(token_program_id.into(), false),  // Add token program
        ],
        data: instruction_data
    };

    let creator_account = Account {
        lamports: 10_000_000, // 0.01 SOL
        data: vec![],
        owner: solana_sdk::system_program::id(),
        executable: false,
        rent_epoch: 0,
    };

    let batch_state_account = Account {
        lamports: 0,
        data: vec![],
        owner: solana_sdk::system_program::id(),
        executable: false,
        rent_epoch: 0,
    };

    let creator_token_account = Account {
        lamports: 0,
        data: vec![],
        owner: pinocchio_token::ID.into(),
        executable: false,
        rent_epoch: 0,
    };

    let mint_account = Account {
        lamports: 0,
        data: vec![],
        owner: pinocchio_token::ID.into(),
        executable: false,
        rent_epoch: 0,
    };

    mollusk.process_and_validate_instruction(
        &instruction,
        &vec![
            (CREATOR, creator_account),
            (MINT, mint_account),
            (batch_state_pda, batch_state_account),
            (creator_token_account_pda, creator_token_account),
            (system_program_id, system_account),
            (token_program_id.into(), token_program_account), // Add token program account
        ],
        &[Check::success()],
    );
}

}