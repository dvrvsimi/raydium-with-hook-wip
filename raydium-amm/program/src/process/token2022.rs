//! Handles Token-2022 mint creation and transfer hook operations

use solana_system_interface::instruction as system_instruction;
use crate::{
    error::AmmError,
    instruction::{CreateToken2022MintInstruction, CreateTransferHookInstruction, UpdateHookWhitelistInstruction, TokenTransferInstruction},
    state::{find_whitelist_pda, HookWhitelist},
};


use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    program::{invoke},
    pubkey::Pubkey,
    program_error::ProgramError,
    rent::Rent,
    sysvar::Sysvar,
};
use spl_token_2022::{
    extension::{ExtensionType, BaseStateWithExtensions, StateWithExtensions},
    state::Mint,
};
use solana_program::program_pack::Pack;
use spl_tlv_account_resolution::state::ExtraAccountMetaList;


pub fn process_create_token2022_mint(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction: CreateToken2022MintInstruction,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let mint_account = next_account_info(account_info_iter)?;
    let mint_authority = next_account_info(account_info_iter)?;
    let payer = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    let token_program_2022 = next_account_info(account_info_iter)?;

    // Validation
    if !payer.is_signer {
        return Err(AmmError::InvalidSignAccount.into());
    }

    if *token_program_2022.key != spl_token_2022::id() {
        return Err(AmmError::InvalidSplTokenProgram.into());
    }

    if !mint_account.is_signer {
        return Err(AmmError::InvalidSignAccount.into());
    }

    // Calculate space needed for mint with extensions
    let mut extensions = vec![];

    // Add transfer hook extension if specified
    if instruction.transfer_hook_program_id.is_some() {
        extensions.push(ExtensionType::TransferHook);
    }

    // Add metadata pointer extension for name/symbol/uri (see if this won't cause issues)
    if !instruction.name.is_empty() || !instruction.symbol.is_empty() || !instruction.uri.is_empty() {
        extensions.push(ExtensionType::MetadataPointer);
    }

    // Calculate total space needed
    let space = if extensions.is_empty() {
        ExtensionType::try_calculate_account_len::<Mint>(&[])
            .map_err(|_| ProgramError::InvalidAccountData)?
    } else {
        ExtensionType::try_calculate_account_len::<Mint>(&extensions)
            .map_err(|_| ProgramError::InvalidAccountData)?
    };

    // Get rent
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(space);

    // Create the mint account
    let create_account_ix = system_instruction::create_account(
        payer.key,
        mint_account.key,
        lamports,
        space as u64,
        &spl_token_2022::id(),
    );

    invoke(
        &create_account_ix,
        &[payer.clone(), mint_account.clone(), system_program.clone()],
    )?;

    // Initialize extensions BEFORE initializing the mint
    
    // Initialize metadata pointer extension first (might need to remove)
    if !instruction.name.is_empty() || !instruction.symbol.is_empty() || !instruction.uri.is_empty() {
        let init_metadata_pointer_ix = spl_token_2022::extension::metadata_pointer::instruction::initialize(
            &spl_token_2022::id(),
            mint_account.key,
            Some(*mint_authority.key),
            Some(*mint_account.key), // metadata stored in mint account itself
        )?;
        
        invoke(
            &init_metadata_pointer_ix,
            &[mint_account.clone(), token_program_2022.clone()],
        )?;
        
        msg!("Metadata pointer extension initialized");
    }
    
    // Initialize transfer hook extension if specified
    if let Some(hook_program_id) = instruction.transfer_hook_program_id {
        let init_hook_ix = spl_token_2022::extension::transfer_hook::instruction::initialize(
            &spl_token_2022::id(),
            mint_account.key,
            Some(*mint_authority.key),
            Some(hook_program_id),
        )?;
        
        invoke(
            &init_hook_ix,
            &[mint_account.clone(), token_program_2022.clone()],
        )?;
        
        msg!("Transfer hook extension initialized: {}", hook_program_id);
    }

    // Initialize the base mint (must be done AFTER extensions)
    let init_mint_ix = spl_token_2022::instruction::initialize_mint2(
        &spl_token_2022::id(),
        mint_account.key,
        mint_authority.key,
        instruction.freeze_authority.as_ref(),
        instruction.decimals,
    )?;

    invoke(
        &init_mint_ix,
        &[mint_account.clone(), token_program_2022.clone()],
    )?;

    // Initialize metadata if we have any (must be AFTER mint initialization)
    if !instruction.name.is_empty() || !instruction.symbol.is_empty() || !instruction.uri.is_empty() {
        let init_metadata_ix = spl_token_metadata_interface::instruction::initialize(
            &spl_token_2022::id(),
            mint_account.key,
            mint_authority.key,
            mint_account.key, // metadata account (same as mint for embedded)
            mint_authority.key, // update authority
            instruction.name,
            instruction.symbol,
            instruction.uri,
        );

        invoke(
            &init_metadata_ix,
            &[
                mint_account.clone(),
                mint_authority.clone(),
                token_program_2022.clone(),
            ],
        )?;

        msg!("Token metadata initialized");
    }

    // Log success
    msg!("Token-2022 mint created successfully: {}", mint_account.key);
    msg!("Decimals: {}", instruction.decimals);
    msg!("Mint Authority: {}", mint_authority.key);
    
    if let Some(freeze_authority) = instruction.freeze_authority {
        msg!("Freeze Authority: {}", freeze_authority);
    }
    
    if let Some(hook_program_id) = instruction.transfer_hook_program_id {
        msg!("Transfer Hook Program: {}", hook_program_id);
    }

    Ok(())
}

pub fn process_create_transfer_hook(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _instruction: CreateTransferHookInstruction,
) -> ProgramResult {
    // This would create a transfer hook program
    // For now, just return success
    msg!("Transfer hook creation not implemented yet");
    Ok(())
}

pub fn process_update_hook_whitelist(
    _program_id: &Pubkey,
    _accounts: &[AccountInfo],
    _instruction: UpdateHookWhitelistInstruction,
) -> ProgramResult {
    // This would update the hook whitelist
    // For now, just return success
    msg!("Hook whitelist update not implemented yet");
    Ok(())
}

pub fn process_token_transfer(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction: TokenTransferInstruction,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    // Get accounts
    let source_account = next_account_info(account_info_iter)?;
    let destination_account = next_account_info(account_info_iter)?;
    let mint_account = next_account_info(account_info_iter)?;
    let authority_account = next_account_info(account_info_iter)?;
    let token_program = next_account_info(account_info_iter)?;
    
    // Optionally get extra accounts for transfer hooks
    let remaining_accounts: Vec<AccountInfo> = account_info_iter.cloned().collect();
    
    // Verify authority is signer
    if !authority_account.is_signer {
        return Err(AmmError::InvalidSignAccount.into());
    }
    
    // Check if this is Token-2022 or SPL Token
    let is_token_2022 = *token_program.key == spl_token_2022::id();
    let is_spl_token = *token_program.key == spl_token::id();
    
    if !is_token_2022 && !is_spl_token {
        return Err(AmmError::InvalidSplTokenProgram.into());
    }
    
    if is_token_2022 {
        // Read mint data to get decimals and check for transfer hook
        let mint_data = mint_account.try_borrow_data()?;
        let mint_state = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;
        let decimals = mint_state.base.decimals;
        
        // Check if mint has transfer hook extension
        let has_transfer_hook = mint_state.get_extension::<spl_token_2022::extension::transfer_hook::TransferHook>().is_ok();
        
        if has_transfer_hook {
            // Use transfer_checked with hook execution
            let transfer_ix = spl_token_2022::instruction::transfer_checked(
                token_program.key,
                source_account.key,
                mint_account.key,
                destination_account.key,
                authority_account.key,
                &[],
                instruction.amount,
                decimals,
            )?;
            
            // Include remaining accounts for hook execution
            let mut invoke_accounts = vec![
                source_account.clone(),
                mint_account.clone(),
                destination_account.clone(),
                authority_account.clone(),
                token_program.clone(),
            ];
            invoke_accounts.extend(remaining_accounts);
            
            invoke(&transfer_ix, &invoke_accounts)?;
        } else {
            // Regular Token-2022 transfer without hooks
            let transfer_ix = spl_token_2022::instruction::transfer_checked(
                token_program.key,
                source_account.key,
                mint_account.key,
                destination_account.key,
                authority_account.key,
                &[],
                instruction.amount,
                decimals,
            )?;
            
            invoke(
                &transfer_ix,
                &[
                    source_account.clone(),
                    mint_account.clone(),
                    destination_account.clone(),
                    authority_account.clone(),
                    token_program.clone(),
                ],
            )?;
        }
    } else {
        // SPL Token transfer (no hooks supported)
        let transfer_ix = spl_token::instruction::transfer(
            token_program.key,
            source_account.key,
            destination_account.key,
            authority_account.key,
            &[],
            instruction.amount,
        )?;
        
        invoke(
            &transfer_ix,
            &[
                source_account.clone(),
                destination_account.clone(),
                authority_account.clone(),
                token_program.clone(),
            ],
        )?;
    }
    
    msg!("Token transfer executed successfully");
    msg!("Amount: {}", instruction.amount);
    msg!("From: {}", source_account.key);
    msg!("To: {}", destination_account.key);
    
    Ok(())
}

// TransferHook processing removed - use SPL Transfer Hook Interface instead

/// Initialize extra account meta list for transfer hook
pub fn process_initialize_extra_account_meta_list(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    extra_account_metas: Vec<spl_tlv_account_resolution::account::ExtraAccountMeta>,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    
    let extra_account_meta_list = next_account_info(account_info_iter)?;
    let mint = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;
    let system_program = next_account_info(account_info_iter)?;
    
    // Validation
    if !authority.is_signer {
        return Err(AmmError::InvalidSignAccount.into());
    }
    
    if *system_program.key != solana_program::system_program::id() {
        return Err(AmmError::InvalidSystemProgram.into());
    }
    
    // Create extra account meta list account
    let space = ExtraAccountMetaList::size_of(extra_account_metas.len())?;
    let lamports = Rent::get()?.minimum_balance(space);
    
    let create_account_ix = system_instruction::create_account(
        authority.key,
        extra_account_meta_list.key,
        lamports,
        space as u64,
        program_id,
    );
    
    invoke(
        &create_account_ix,
        &[authority.clone(), extra_account_meta_list.clone(), system_program.clone()],
    )?;
    
    // Initialize the extra account meta list using spl_transfer_hook_interface
    let init_ix = spl_transfer_hook_interface::instruction::initialize_extra_account_meta_list(
        program_id,
        extra_account_meta_list.key,
        mint.key,
        authority.key,
        &extra_account_metas,
    );
    
    invoke(
        &init_ix,
        &[extra_account_meta_list.clone(), mint.clone(), authority.clone(), system_program.clone()],
    )?;
    
    msg!("Extra account meta list initialized successfully");
    msg!("Authority: {}", authority.key);
    msg!("Extra accounts: {:?}", extra_account_metas);
    
    Ok(())
}