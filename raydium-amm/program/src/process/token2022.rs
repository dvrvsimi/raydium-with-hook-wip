//! Handles Token-2022 mint creation and transfer hook operations

use solana_system_interface::instruction as system_instruction;
use crate::{
    error::AmmError,
    instruction::{CreateToken2022MintInstruction, UpdateHookWhitelistInstruction},
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



pub fn process_update_hook_whitelist(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction: UpdateHookWhitelistInstruction,
) -> ProgramResult {
    // this function should just validate and delegate to the whitelist module
    crate::process::whitelist::process_update_hook_whitelist(program_id, accounts, instruction)
}

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