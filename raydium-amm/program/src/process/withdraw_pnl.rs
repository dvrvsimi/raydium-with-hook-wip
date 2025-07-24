//! Handles the withdraw_pnl instruction logic for Raydium AMM
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    msg,
};
use crate::{
    error::AmmError,
    invokers::Invokers,
    state::AmmInfo,
};
use crate::process::constants::AUTHORITY_AMM;
use crate::process::helpers::{authority_id, unpack_token_account};

pub fn process_withdrawpnl(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let amm_info = next_account_info(account_info_iter)?;
    let amm_authority_info = next_account_info(account_info_iter)?;
    let amm_coin_vault_info = next_account_info(account_info_iter)?;
    let amm_pc_vault_info = next_account_info(account_info_iter)?;
    let user_dest_coin_info = next_account_info(account_info_iter)?;
    let user_dest_pc_info = next_account_info(account_info_iter)?;
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

    // Check vaults
    if *amm_coin_vault_info.key != amm.coin_vault {
        return Err(AmmError::InvalidCoinVault.into());
    }
    if *amm_pc_vault_info.key != amm.pc_vault {
        return Err(AmmError::InvalidPCVault.into());
    }

    // Unpack token accounts
    let amm_coin_vault = unpack_token_account(&amm_coin_vault_info, token_program_info.key)?;
    let amm_pc_vault = unpack_token_account(&amm_pc_vault_info, token_program_info.key)?;
    let user_dest_coin = unpack_token_account(&user_dest_coin_info, token_program_info.key)?;
    let user_dest_pc = unpack_token_account(&user_dest_pc_info, token_program_info.key)?;

    // Check user destination accounts
    if user_dest_coin.owner != *user_wallet_info.key {
        return Err(AmmError::InvalidOwner.into());
    }
    if user_dest_pc.owner != *user_wallet_info.key {
        return Err(AmmError::InvalidOwner.into());
    }

    // Check mints
    if user_dest_coin.mint != amm_coin_vault.mint {
        return Err(AmmError::InvalidCoinMint.into());
    }
    if user_dest_pc.mint != amm_pc_vault.mint {
        return Err(AmmError::InvalidPCMint.into());
    }

    // Calculate PnL amounts
    let coin_pnl = if amm.state_data.total_pnl_coin > 0 {
        amm.state_data.total_pnl_coin
    } else {
        0
    };

    let pc_pnl = if amm.state_data.total_pnl_pc > 0 {
        amm.state_data.total_pnl_pc
    } else {
        0
    };

    if coin_pnl == 0 && pc_pnl == 0 {
        return Err(AmmError::InsufficientFunds.into());
    }

    // Transfer PnL tokens
    if coin_pnl > 0 {
        Invokers::token_transfer_with_authority(
            token_program_info.clone(),
            amm_coin_vault_info.clone(),
            user_dest_coin_info.clone(),
            amm_authority_info.clone(),
            AUTHORITY_AMM,
            amm.nonce as u8,
            coin_pnl,
            amm_coin_vault_info.clone(),
            &[],
        )?;
    }

    if pc_pnl > 0 {
        Invokers::token_transfer_with_authority(
            token_program_info.clone(),
            amm_pc_vault_info.clone(),
            user_dest_pc_info.clone(),
            amm_authority_info.clone(),
            AUTHORITY_AMM,
            amm.nonce as u8,
            pc_pnl,
            amm_pc_vault_info.clone(),
            &[],
        )?;
    }

    // Reset PnL amounts
    amm.state_data.total_pnl_coin = 0;
    amm.state_data.total_pnl_pc = 0;

    msg!("PnL withdrawn successfully: {} coin, {} pc", coin_pnl, pc_pnl);
    Ok(())
} 