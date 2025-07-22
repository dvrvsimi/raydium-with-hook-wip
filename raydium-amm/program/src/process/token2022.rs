//! Handles Token-2022 mint creation and transfer hook operations

use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    msg,
    pubkey::Pubkey,
    program::invoke,
};
use solana_system_interface::instruction as system_instruction;
use crate::{
    error::AmmError,
    instruction::{CreateToken2022MintInstruction, CreateTransferHookInstruction, UpdateHookWhitelistInstruction},
};

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
    let rent = next_account_info(account_info_iter)?;

    if !payer.is_signer {
        return Err(AmmError::InvalidSignAccount.into());
    }

    // Validate token program
    if *token_program_2022.key != spl_token_2022::id() {
        return Err(AmmError::InvalidSplTokenProgram.into());
    }

    // Calculate space needed for mint (82 bytes for basic mint)
    let space = 82;
    let lamports = **rent.try_borrow_lamports()?;

    // Create the mint account
    let create_account_ix = system_instruction::create_account(
        payer.key,
        mint_account.key,
        lamports,
        space,
        &spl_token_2022::id(),
    );

    invoke(
        &create_account_ix,
        &[payer.clone(), mint_account.clone(), system_program.clone()],
    )?;

    // Initialize the mint
    let init_mint_ix = spl_token_2022::instruction::initialize_mint(
        &spl_token_2022::id(),
        mint_account.key,
        mint_authority.key,
        instruction.freeze_authority.as_ref(),
        instruction.decimals,
    )?;

    invoke(
        &init_mint_ix,
        &[mint_account.clone(), mint_authority.clone(), token_program_2022.clone()],
    )?;

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