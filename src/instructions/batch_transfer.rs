use pinocchio::{account_info::AccountInfo, instruction::Signer, program_error::ProgramError, sysvars::{rent::Rent, Sysvar}, *};
use pinocchio_system::instructions::CreateAccount;

use pinocchio_token::{instructions::{InitializeAccount3, Transfer}, state::TokenAccount};

use crate::state::BatchState;
use crate::state::BatchTransfer;

pub fn process_batch_transfer(accounts: &[AccountInfo], instruction_data: &[u8]) -> ProgramResult {

    if accounts.len() < 5 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let [creator, creator_token_account, mint, batch_state, batch_transfer, token_program, rest @..] = accounts else {
        return Err(ProgramError::InvalidAccountData);
    };

    if instruction_data.len() != BatchTransfer::LEN {
        return Err(ProgramError::InvalidInstructionData);
    }

    let mut batch_state_account_info = BatchState::from_account_info_mut(batch_state)?;

    let (batch_state_pda, batch_bump) = pubkey::find_program_address(
        &[b"batch_state", creator.key().as_ref(), mint.key().as_ref(), &batch_state_account_info.batch_id.to_le_bytes()],
        &crate::ID
    );

    let (creator_token_account_pda, bump2) = pubkey::find_program_address(
        &[b"creator_token_account", creator.key().as_ref(), mint.key().as_ref()],
        &crate::ID
    );

    if *creator_token_account.key() != creator_token_account_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    if *batch_state.key() != batch_state_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    let (batch_transfer_pda, bump) = pubkey::find_program_address(
        &[b"batch_transfer", batch_state.key().as_ref()],
        &crate::ID
    );

    if *batch_transfer.key() != batch_transfer_pda {
        return Err(ProgramError::InvalidAccountData);
    };

    let batch_transfer = BatchTransfer::from_account_info(batch_transfer)?;

    batch_state_account_info.validate_authority(creator.key())?;
    batch_state_account_info.validate_mint(mint.key())?;

    if batch_state_account_info.batch_id != batch_transfer.batch_id {
        return Err(ProgramError::InvalidAccountData);
    };

    for recipient_pubkey in batch_transfer.get_active_recipients() {
        let recipient_token_account = rest
            .iter()
            .find(|acc| acc.key() == recipient_pubkey)
            .ok_or(ProgramError::InvalidAccountData)?;

        if recipient_token_account.data_len() == 0 {
            msg!("Creating recipient token account");

            let lamports = Rent::get()?.minimum_balance(TokenAccount::LEN);

            CreateAccount {
                from: creator,
                to: recipient_token_account,
                lamports,
                space: TokenAccount::LEN as u64,
                owner: &pinocchio_token::id(),
            }
            .invoke()?;

            InitializeAccount3 {
                account: recipient_token_account,
                mint,
                owner: recipient_pubkey,
            }
            .invoke()?;
        };

        let batch_id = batch_state_account_info.batch_id.to_le_bytes();

        let seeds = seeds!(
            b"batch_state",
            creator.key().as_ref(),
            mint.key().as_ref(),
            batch_id.as_ref()
        );

        let signer_seeds = Signer::from(&seeds);

        Transfer {
            from: creator_token_account,
            to: recipient_token_account,
            authority: batch_state,
            amount: batch_transfer.amount_per_recipient,
        }
        .invoke_signed(&[signer_seeds])?; 

        batch_state_account_info.increment_processed();
    };

    Ok(())
}

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
    use crate::state::{BatchState, BatchTransfer};

    const PROGRAM_ID: Pubkey = pubkey!("7B3prxsmARuNjdD5qa5CmDur1tPWbH4UNbZu1AVJCRJo");
    const CREATOR: Pubkey = Pubkey::new_from_array([1u8; 32]);
    const BATCH_ID: u64 = 1;
    const MINT: Pubkey = Pubkey::new_from_array([2u8; 32]);
    const RECIPIENT_1: Pubkey = Pubkey::new_from_array([3u8; 32]);
    const RECIPIENT_2: Pubkey = Pubkey::new_from_array([4u8; 32]);

    #[test]
    fn test_process_batch_transfer() {
        let mollusk = Mollusk::new(&PROGRAM_ID, "target/deploy/batch_token_transfer");

        // Calculate PDAs
        let (batch_state_pda, batch_bump) = Pubkey::find_program_address(
            &[b"batch_state", CREATOR.as_ref(), MINT.as_ref(), &BATCH_ID.to_le_bytes()],
            &PROGRAM_ID
        );

        let (creator_token_account_pda, creator_bump) = Pubkey::find_program_address(
            &[b"creator_token_account", CREATOR.as_ref(), MINT.as_ref()],
            &PROGRAM_ID
        );

        let (batch_transfer_pda, transfer_bump) = Pubkey::find_program_address(
            &[b"batch_transfer", batch_state_pda.as_ref()],
            &PROGRAM_ID
        );

        let (system_program_id, system_account) = program::keyed_account_for_system_program();
        let (token_program_id, token_program_account) = program::keyed_account_for_system_program();

        // Create instruction data - BatchTransfer serialized data
        let mut batch_transfer_data = vec![0u8; BatchTransfer::LEN];
        
        // Create a mock BatchTransfer struct data
        // Assuming BatchTransfer has: batch_id (8 bytes) + amount_per_recipient (8 bytes) + recipients array
        batch_transfer_data[0..8].copy_from_slice(&BATCH_ID.to_le_bytes()); // batch_id
        batch_transfer_data[8..16].copy_from_slice(&1000u64.to_le_bytes()); // amount_per_recipient
        // Add recipient pubkeys to the data (assuming they come after amount)
        batch_transfer_data[16..48].copy_from_slice(RECIPIENT_1.as_ref());
        batch_transfer_data[48..80].copy_from_slice(RECIPIENT_2.as_ref());

        let mut instruction_data = vec![0u8; 1 + BatchTransfer::LEN];
        instruction_data[0] = 1; // ProcessBatchTransfer discriminator
        instruction_data[1..].copy_from_slice(&batch_transfer_data);

        let instruction = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(CREATOR, true),
                AccountMeta::new(creator_token_account_pda, false),
                AccountMeta::new_readonly(MINT, false),
                AccountMeta::new(batch_state_pda, false),
                AccountMeta::new_readonly(batch_transfer_pda, false),
                AccountMeta::new_readonly(token_program_id, false),
                // Recipient token accounts
                AccountMeta::new(RECIPIENT_1, false),
                AccountMeta::new(RECIPIENT_2, false),
            ],
            data: instruction_data
        };

        // Creator account
        let creator_account = Account {
            lamports: 10_000_000,
            data: vec![],
            owner: solana_sdk::system_program::id(),
            executable: false,
            rent_epoch: 0,
        };

        // Mint account (initialized)
        let mut mint_data = vec![0u8; 82]; // TokenMint size
        mint_data[0] = 1; // initialized
        let mint_account = Account {
            lamports: 1_000_000,
            data: mint_data,
            owner: pinocchio_token::ID.into(),
            executable: false,
            rent_epoch: 0,
        };

        // Creator token account (initialized with sufficient balance)
        let mut creator_token_data = vec![0u8; TokenAccount::LEN];
        creator_token_data[0] = 1; // initialized
        creator_token_data[32..64].copy_from_slice(MINT.as_ref()); // mint
        creator_token_data[64..96].copy_from_slice(CREATOR.as_ref()); // owner
        creator_token_data[96..104].copy_from_slice(&10000u64.to_le_bytes()); // amount (sufficient for transfers)
        
        let creator_token_account = Account {
            lamports: 2_000_000,
            data: creator_token_data,
            owner: pinocchio_token::ID.into(),
            executable: false,
            rent_epoch: 0,
        };

        // Batch state account (initialized)
        let mut batch_state_data = vec![0u8; BatchState::LEN];
        batch_state_data[0..8].copy_from_slice(&BATCH_ID.to_le_bytes()); // batch_id
        batch_state_data[8..40].copy_from_slice(CREATOR.as_ref()); // authority
        batch_state_data[40..72].copy_from_slice(MINT.as_ref()); // mint
        batch_state_data[72] = 2; // total_recipients
        batch_state_data[73] = 0; // processed_count
        batch_state_data[74] = 0; // is_completed
        batch_state_data[75] = batch_bump; // bump
        
        let batch_state_account = Account {
            lamports: 2_000_000,
            data: batch_state_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        };

        // Batch transfer account (initialized)
        let mut batch_transfer_data = vec![0u8; BatchTransfer::LEN];
        batch_transfer_data[0..8].copy_from_slice(&BATCH_ID.to_le_bytes()); // batch_id
        batch_transfer_data[8..16].copy_from_slice(&1000u64.to_le_bytes()); // amount_per_recipient
        batch_transfer_data[16..48].copy_from_slice(RECIPIENT_1.as_ref()); // recipient 1
        batch_transfer_data[48..80].copy_from_slice(RECIPIENT_2.as_ref()); // recipient 2
        
        let batch_transfer_account = Account {
            lamports: 2_000_000,
            data: batch_transfer_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        };

        // Recipient token accounts (uninitialized - will be created)
        let recipient_1_token_account = Account {
            lamports: 0,
            data: vec![],
            owner: solana_sdk::system_program::id(),
            executable: false,
            rent_epoch: 0,
        };

        let recipient_2_token_account = Account {
            lamports: 0,
            data: vec![],
            owner: solana_sdk::system_program::id(),
            executable: false,
            rent_epoch: 0,
        };

        mollusk.process_and_validate_instruction(
            &instruction,
            &vec![
                (CREATOR, creator_account),
                (creator_token_account_pda, creator_token_account),
                (MINT, mint_account),
                (batch_state_pda, batch_state_account),
                (batch_transfer_pda, batch_transfer_account),
                (token_program_id, token_program_account),
                (RECIPIENT_1, recipient_1_token_account),
                (RECIPIENT_2, recipient_2_token_account),
                (system_program_id, system_account),
            ],
            &[Check::success()],
        );
    }

    #[test]
    fn test_process_batch_transfer_with_existing_recipient_accounts() {
        let mollusk = Mollusk::new(&PROGRAM_ID, "target/deploy/batch_token_transfer");

        let (batch_state_pda, batch_bump) = Pubkey::find_program_address(
            &[b"batch_state", CREATOR.as_ref(), MINT.as_ref(), &BATCH_ID.to_le_bytes()],
            &PROGRAM_ID
        );

        let (creator_token_account_pda, creator_bump) = Pubkey::find_program_address(
            &[b"creator_token_account", CREATOR.as_ref(), MINT.as_ref()],
            &PROGRAM_ID
        );

        let (batch_transfer_pda, transfer_bump) = Pubkey::find_program_address(
            &[b"batch_transfer", batch_state_pda.as_ref()],
            &PROGRAM_ID
        );

        let (system_program_id, system_account) = program::keyed_account_for_system_program();
        let (token_program_id, token_program_account) = program::keyed_account_for_system_program();

        // Create instruction data
        let mut batch_transfer_data = vec![0u8; BatchTransfer::LEN];
        batch_transfer_data[0..8].copy_from_slice(&BATCH_ID.to_le_bytes());
        batch_transfer_data[8..16].copy_from_slice(&500u64.to_le_bytes()); // smaller amount
        batch_transfer_data[16..48].copy_from_slice(RECIPIENT_1.as_ref());
        batch_transfer_data[48..80].copy_from_slice(RECIPIENT_2.as_ref());

        let mut instruction_data = vec![0u8; 1 + BatchTransfer::LEN];
        instruction_data[0] = 1; // ProcessBatchTransfer discriminator
        instruction_data[1..].copy_from_slice(&batch_transfer_data);

        let instruction = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(CREATOR, true),
                AccountMeta::new(creator_token_account_pda, false),
                AccountMeta::new_readonly(MINT, false),
                AccountMeta::new(batch_state_pda, false),
                AccountMeta::new_readonly(batch_transfer_pda, false),
                AccountMeta::new_readonly(token_program_id, false),
                AccountMeta::new(RECIPIENT_1, false),
                AccountMeta::new(RECIPIENT_2, false),
            ],
            data: instruction_data
        };

        // Same setup as before but with initialized recipient accounts
        let creator_account = Account {
            lamports: 10_000_000,
            data: vec![],
            owner: solana_sdk::system_program::id(),
            executable: false,
            rent_epoch: 0,
        };

        let mut mint_data = vec![0u8; 82];
        mint_data[0] = 1;
        let mint_account = Account {
            lamports: 1_000_000,
            data: mint_data,
            owner: pinocchio_token::ID.into(),
            executable: false,
            rent_epoch: 0,
        };

        let mut creator_token_data = vec![0u8; TokenAccount::LEN];
        creator_token_data[0] = 1;
        creator_token_data[32..64].copy_from_slice(MINT.as_ref());
        creator_token_data[64..96].copy_from_slice(CREATOR.as_ref());
        creator_token_data[96..104].copy_from_slice(&5000u64.to_le_bytes()); // sufficient balance
        
        let creator_token_account = Account {
            lamports: 2_000_000,
            data: creator_token_data,
            owner: pinocchio_token::ID.into(),
            executable: false,
            rent_epoch: 0,
        };

        let mut batch_state_data = vec![0u8; BatchState::LEN];
        batch_state_data[0..8].copy_from_slice(&BATCH_ID.to_le_bytes());
        batch_state_data[8..40].copy_from_slice(CREATOR.as_ref());
        batch_state_data[40..72].copy_from_slice(MINT.as_ref());
        batch_state_data[72] = 2;
        batch_state_data[73] = 0;
        batch_state_data[74] = 0;
        batch_state_data[75] = batch_bump;
        
        let batch_state_account = Account {
            lamports: 2_000_000,
            data: batch_state_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        };

        let mut batch_transfer_data = vec![0u8; BatchTransfer::LEN];
        batch_transfer_data[0..8].copy_from_slice(&BATCH_ID.to_le_bytes());
        batch_transfer_data[8..16].copy_from_slice(&500u64.to_le_bytes());
        batch_transfer_data[16..48].copy_from_slice(RECIPIENT_1.as_ref());
        batch_transfer_data[48..80].copy_from_slice(RECIPIENT_2.as_ref());
        
        let batch_transfer_account = Account {
            lamports: 2_000_000,
            data: batch_transfer_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        };

        // Pre-initialized recipient token accounts
        let mut recipient_1_token_data = vec![0u8; TokenAccount::LEN];
        recipient_1_token_data[0] = 1; // initialized
        recipient_1_token_data[32..64].copy_from_slice(MINT.as_ref()); // mint
        recipient_1_token_data[64..96].copy_from_slice(RECIPIENT_1.as_ref()); // owner
        recipient_1_token_data[96..104].copy_from_slice(&0u64.to_le_bytes()); // amount (start with 0)
        
        let recipient_1_token_account = Account {
            lamports: 2_000_000,
            data: recipient_1_token_data,
            owner: pinocchio_token::ID.into(),
            executable: false,
            rent_epoch: 0,
        };

        let mut recipient_2_token_data = vec![0u8; TokenAccount::LEN];
        recipient_2_token_data[0] = 1;
        recipient_2_token_data[32..64].copy_from_slice(MINT.as_ref());
        recipient_2_token_data[64..96].copy_from_slice(RECIPIENT_2.as_ref());
        recipient_2_token_data[96..104].copy_from_slice(&0u64.to_le_bytes());
        
        let recipient_2_token_account = Account {
            lamports: 2_000_000,
            data: recipient_2_token_data,
            owner: pinocchio_token::ID.into(),
            executable: false,
            rent_epoch: 0,
        };

        mollusk.process_and_validate_instruction(
            &instruction,
            &vec![
                (CREATOR, creator_account),
                (creator_token_account_pda, creator_token_account),
                (MINT, mint_account),
                (batch_state_pda, batch_state_account),
                (batch_transfer_pda, batch_transfer_account),
                (token_program_id, token_program_account),
                (RECIPIENT_1, recipient_1_token_account),
                (RECIPIENT_2, recipient_2_token_account),
                (system_program_id, system_account),
            ],
            &[Check::success()],
        );
    }

    #[test]
    fn test_process_batch_transfer_insufficient_accounts() {
        let mollusk = Mollusk::new(&PROGRAM_ID, "target/deploy/batch_token_transfer");

        let mut instruction_data = vec![0u8; 1 + BatchTransfer::LEN];
        instruction_data[0] = 1; // ProcessBatchTransfer discriminator

        let instruction = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(CREATOR, true),
                AccountMeta::new(MINT, false),
                // Missing required accounts
            ],
            data: instruction_data
        };

        let creator_account = Account {
            lamports: 10_000_000,
            data: vec![],
            owner: solana_sdk::system_program::id(),
            executable: false,
            rent_epoch: 0,
        };

        let mint_account = Account {
            lamports: 1_000_000,
            data: vec![0u8; 82],
            owner: pinocchio_token::ID.into(),
            executable: false,
            rent_epoch: 0,
        };

        mollusk.process_and_validate_instruction(
            &instruction,
            &vec![
                (CREATOR, creator_account),
                (MINT, mint_account),
            ],
            &[Check::err(solana_sdk::program_error::ProgramError::InvalidAccountData)],
        );
    }

    #[test]
    fn test_process_batch_transfer_invalid_instruction_data() {
        let mollusk = Mollusk::new(&PROGRAM_ID, "target/deploy/batch_token_transfer");

        let (batch_state_pda, _) = Pubkey::find_program_address(
            &[b"batch_state", CREATOR.as_ref(), MINT.as_ref(), &BATCH_ID.to_le_bytes()],
            &PROGRAM_ID
        );

        let (creator_token_account_pda, _) = Pubkey::find_program_address(
            &[b"creator_token_account", CREATOR.as_ref(), MINT.as_ref()],
            &PROGRAM_ID
        );

        let (batch_transfer_pda, _) = Pubkey::find_program_address(
            &[b"batch_transfer", batch_state_pda.as_ref()],
            &PROGRAM_ID
        );

        let (token_program_id, token_program_account) = program::keyed_account_for_system_program();

        // Invalid instruction data (too short)
        let instruction_data = vec![1u8]; // Only discriminator, missing BatchTransfer data

        let instruction = Instruction {
            program_id: PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(CREATOR, true),
                AccountMeta::new(creator_token_account_pda, false),
                AccountMeta::new_readonly(MINT, false),
                AccountMeta::new(batch_state_pda, false),
                AccountMeta::new_readonly(batch_transfer_pda, false),
                AccountMeta::new_readonly(token_program_id, false),
            ],
            data: instruction_data
        };

        mollusk.process_and_validate_instruction(
            &instruction,
            &vec![
                (CREATOR, Account::default()),
                (creator_token_account_pda, Account::default()),
                (MINT, Account::default()),
                (batch_state_pda, Account::default()),
                (batch_transfer_pda, Account::default()),
                (token_program_id, token_program_account),
            ],
            &[Check::err(solana_sdk::program_error::ProgramError::InvalidAccountData)],
        );
    }
}