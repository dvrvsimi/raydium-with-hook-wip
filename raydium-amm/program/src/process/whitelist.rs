use crate::{
    instruction::{UpdateHookWhitelistInstruction, HookWhitelistAction},
    state::{find_whitelist_pda, HookWhitelist},
};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    program_pack::Pack,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
    program::invoke_signed,
    msg,
};
use solana_system_interface::instruction as system_instruction;


/// Initialize the hook whitelist PDA
pub fn process_initialize_hook_whitelist(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    authority: Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let whitelist_account_info = next_account_info(account_info_iter)?;
    let payer_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;
    
    // Verify payer is signer
    if !payer_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Verify whitelist account PDA, bump is not used here
    let (expected_whitelist_pda, _) = find_whitelist_pda(program_id);
    if whitelist_account_info.key != &expected_whitelist_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Check if already initialized
    if !whitelist_account_info.data_is_empty() {
        msg!("Whitelist already initialized");
        return Err(ProgramError::AccountAlreadyInitialized);
    }
    
    // Calculate rent
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(HookWhitelist::LEN);
    
    // Create the whitelist account
    let create_account_ix = system_instruction::create_account(
        payer_info.key,
        whitelist_account_info.key,
        lamports,
        HookWhitelist::LEN as u64,
        program_id,
    );
    
    let seeds: &[&[u8]] = &[b"hook_whitelist"];
    let signer_seeds = &[seeds];

    invoke_signed(
        &create_account_ix,
        &[
            payer_info.clone(),
            whitelist_account_info.clone(),
            system_program_info.clone(),
        ],
        signer_seeds,
    )?;
    
    // Initialize the whitelist data
    let mut whitelist = HookWhitelist::default();
    whitelist.authority = authority;
    
    // Pack and store the data
    let mut data = vec![0u8; HookWhitelist::LEN];
    whitelist.pack_into_slice(&mut data);
    whitelist_account_info.try_borrow_mut_data()?.copy_from_slice(&data);
    
    msg!("Hook whitelist initialized with authority: {}", authority);
    Ok(())
}

/// Update the hook whitelist (add or remove hooks)
pub fn process_update_hook_whitelist(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction: UpdateHookWhitelistInstruction,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Get accounts
    let whitelist_account_info = next_account_info(account_info_iter)?;
    let authority_info = next_account_info(account_info_iter)?;
    
    // Verify authority is signer
    if !authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Verify whitelist account
    let (expected_whitelist_pda, _) = find_whitelist_pda(program_id);
    if whitelist_account_info.key != &expected_whitelist_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Check if whitelist is initialized
    if whitelist_account_info.data_is_empty() {
        msg!("Whitelist not initialized");
        return Err(ProgramError::UninitializedAccount);
    }
    
    // Load existing whitelist
    let whitelist_data = whitelist_account_info.try_borrow_data()?;
    let mut whitelist = HookWhitelist::unpack(&whitelist_data)?;
    drop(whitelist_data); // Release borrow before mutable borrow
    
    // Verify authority
    if whitelist.authority != *authority_info.key {
        msg!("Invalid authority. Expected: {}, Got: {}", whitelist.authority, authority_info.key);
        return Err(ProgramError::InvalidAccountOwner);
    }
    
    // Update whitelist based on action
    match instruction.action {
        HookWhitelistAction::Add => {
            whitelist.add_hook(instruction.hook_program_id)?;
            msg!("Added hook program to whitelist: {}", instruction.hook_program_id);
        }
        HookWhitelistAction::Remove => {
            whitelist.remove_hook(&instruction.hook_program_id)?;
            msg!("Removed hook program from whitelist: {}", instruction.hook_program_id);
        }
    }
    
    // Pack and store updated whitelist
    let mut updated_data = vec![0u8; HookWhitelist::LEN];
    whitelist.pack_into_slice(&mut updated_data);
    whitelist_account_info.try_borrow_mut_data()?.copy_from_slice(&updated_data);
    
    Ok(())
}

/// Update whitelist authority (transfer ownership)
pub fn process_update_whitelist_authority(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    new_authority: Pubkey,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let whitelist_account_info = next_account_info(account_info_iter)?;
    let current_authority_info = next_account_info(account_info_iter)?;
    
    // Verify current authority is signer
    if !current_authority_info.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }
    
    // Verify whitelist account
    let (expected_whitelist_pda, _) = find_whitelist_pda(program_id);
    if whitelist_account_info.key != &expected_whitelist_pda {
        return Err(ProgramError::InvalidAccountData);
    }
    
    // Check if whitelist is initialized
    if whitelist_account_info.data_is_empty() {
        return Err(ProgramError::UninitializedAccount);
    }
    
    // Load existing whitelist
    let whitelist_data = whitelist_account_info.try_borrow_data()?;
    let mut whitelist = HookWhitelist::unpack(&whitelist_data)?;
    drop(whitelist_data);
    
    // Verify current authority
    if whitelist.authority != *current_authority_info.key {
        return Err(ProgramError::InvalidAccountOwner);
    }
    
    // Update authority
    whitelist.authority = new_authority;
    
    // Pack and store updated whitelist
    let mut updated_data = vec![0u8; HookWhitelist::LEN];
    whitelist.pack_into_slice(&mut updated_data);
    whitelist_account_info.try_borrow_mut_data()?.copy_from_slice(&updated_data);
    
    msg!("Whitelist authority updated to: {}", new_authority);
    Ok(())
}

/// Check if a hook program is whitelisted
pub fn is_hook_whitelisted(
    program_id: &Pubkey,
    whitelist_account: &AccountInfo,
    hook_program_id: &Pubkey,
) -> Result<bool, ProgramError> {
    // Verify PDA
    let (expected_pda, _) = find_whitelist_pda(program_id);
    if whitelist_account.key != &expected_pda {
        msg!("Invalid whitelist PDA provided");
        return Err(ProgramError::InvalidSeeds);
    }

    // If whitelist account is empty, no hooks are allowed
    if whitelist_account.data_is_empty() {
        msg!("Whitelist not initialized - no hooks allowed");
        return Ok(false);
    }

    // Load whitelist data
    let whitelist_data = whitelist_account.try_borrow_data()?;
    let whitelist = HookWhitelist::unpack(&whitelist_data)?;

    // Check if hook is in the whitelist
    Ok(whitelist.contains_hook(hook_program_id))
}

/// Get all whitelisted hooks (for querying)
pub fn get_whitelisted_hooks(
    program_id: &Pubkey,
    whitelist_account: &AccountInfo,
) -> Result<Vec<Pubkey>, ProgramError> {
    // Verify PDA
    let (expected_pda, _) = find_whitelist_pda(program_id);
    if whitelist_account.key != &expected_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // If whitelist account is empty, return empty vec
    if whitelist_account.data_is_empty() {
        return Ok(Vec::new());
    }

    // Load whitelist data
    let whitelist_data = whitelist_account.try_borrow_data()?;
    let whitelist = HookWhitelist::unpack(&whitelist_data)?;

    Ok(whitelist.get_active_hooks().to_vec())
}