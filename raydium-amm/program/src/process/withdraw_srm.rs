//! Handles the withdraw_srm instruction logic for Raydium AMM
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    msg,
};
use crate::{
    error::AmmError,
    instruction::WithdrawSrmInstruction,
    invokers::Invokers,
    state::AmmInfo,
};
use crate::process::constants::AUTHORITY_AMM;
use crate::process::helpers::{authority_id, unpack_token_account};

pub fn process_withdraw_srm(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    withdrawsrm: WithdrawSrmInstruction,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let amm_info = next_account_info(account_info_iter)?;
    let amm_authority_info = next_account_info(account_info_iter)?;
    let amm_srm_vault_info = next_account_info(account_info_iter)?;
    let user_dest_srm_info = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let user_wallet_info = next_account_info(account_info_iter)?;

    if !user_wallet_info.is_signer {
        return Err(AmmError::InvalidSignAccount.into());
    }

    let mut amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;

    // Check authority
    let authority = authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?;
    if *amm_authority_info.key != authority {
        return Err(AmmError::InvalidProgramAddress.into());
    }

    // Check token program
    if *token_program_info.key != spl_token::id() {
        return Err(AmmError::InvalidSplTokenProgram.into());
    }

    // Unpack token accounts
    let amm_srm_vault = unpack_token_account(&amm_srm_vault_info, token_program_info.key)?;
    let user_dest_srm = unpack_token_account(&user_dest_srm_info, token_program_info.key)?;

    // Check user destination account
    if user_dest_srm.owner != *user_wallet_info.key {
        return Err(AmmError::InvalidOwner.into());
    }

    // Check SRM mint
    if user_dest_srm.mint != amm_srm_vault.mint {
        return Err(AmmError::InvalidCoinMint.into());
    }

    // Check withdrawal amount
    if withdrawsrm.amount > amm_srm_vault.amount {
        return Err(AmmError::InsufficientFunds.into());
    }

    if withdrawsrm.amount == 0 {
        return Err(AmmError::InvalidInput.into());
    }

    // Transfer SRM tokens
    Invokers::token_transfer_with_authority(
        program_id,
        token_program_info.clone(),
        amm_srm_vault_info.clone(),
        user_dest_srm_info.clone(),
        amm_authority_info.clone(),
        AUTHORITY_AMM,
        amm.nonce as u8,
        withdrawsrm.amount,
        amm_srm_vault_info.clone(),
        &[],
    )?;

    msg!("SRM withdrawn successfully: {}", withdrawsrm.amount);
    Ok(())
} 