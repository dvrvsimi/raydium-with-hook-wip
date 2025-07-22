//! Handles the config instruction logic for Raydium AMM
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    msg,
};
use crate::{
    error::AmmError,
    instruction::ConfigArgs,
    state::AmmConfig,
};
use crate::process::constants::AMM_CONFIG_SEED;
use crate::process::helpers::get_associated_address_and_bump_seed;

pub fn process_create_config(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let user_wallet_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;
    let rent_info = next_account_info(account_info_iter)?;

    if !user_wallet_info.is_signer {
        return Err(AmmError::InvalidSignAccount.into());
    }

    // Check system program
    if *system_program_info.key != solana_program::system_program::id() {
        return Err(AmmError::InvalidMarketProgram.into());
    }

    // Check rent sysvar
    if *rent_info.key != solana_program::sysvar::rent::id() {
        return Err(AmmError::InvalidMarketProgram.into());
    }

    // Generate config address
    let (config_address, _) = get_associated_address_and_bump_seed(
        user_wallet_info.key,
        user_wallet_info.key,
        AMM_CONFIG_SEED,
        program_id,
    );

    if *config_info.key != config_address {
        return Err(AmmError::InvalidConfigAccount.into());
    }

    // Initialize config
    let mut config = AmmConfig::default();
    config.pnl_owner = *user_wallet_info.key;
    
    let mut config_data = config_info.try_borrow_mut_data()?;
    config_data.copy_from_slice(&bytemuck::bytes_of(&config));

    msg!("Config account created successfully");
    Ok(())
}

pub fn process_update_config(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    config_args: ConfigArgs,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let config_info = next_account_info(account_info_iter)?;
    let user_wallet_info = next_account_info(account_info_iter)?;

    if !user_wallet_info.is_signer {
        return Err(AmmError::InvalidSignAccount.into());
    }

    let mut config = AmmConfig::load_mut_checked(&config_info, program_id)?;

    // Check if user is owner
    if *user_wallet_info.key != config.pnl_owner {
        return Err(AmmError::InvalidOwner.into());
    }

    // Update config parameters
    if let Some(owner) = config_args.owner {
        config.pnl_owner = owner;
    }
    if let Some(create_pool_fee) = config_args.create_pool_fee {
        config.create_pool_fee = create_pool_fee;
    }

    msg!("Config updated successfully");
    Ok(())
}

pub mod referrer_pc_wallet {
    use solana_program::pubkey::Pubkey;
    use solana_program::program_error::ProgramError;
    
    pub fn id() -> Result<Pubkey, ProgramError> {
        // Return a default pubkey for referrer PC wallet
        // This is a placeholder - in a real implementation, this would be configurable
        Ok(Pubkey::default())
    }
} 