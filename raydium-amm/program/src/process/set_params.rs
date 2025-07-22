//! Handles the set_params instruction logic for Raydium AMM
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    msg,
};
use crate::{
    error::AmmError,
    instruction::SetParamsInstruction,
    state::{AmmInfo, AmmParams},
};
use crate::process::constants::AUTHORITY_AMM;
use crate::process::helpers::authority_id;

pub fn process_set_params(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    setparams: SetParamsInstruction,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let amm_info = next_account_info(account_info_iter)?;
    let amm_authority_info = next_account_info(account_info_iter)?;
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

    // Check if user is owner
    if *user_wallet_info.key != amm.amm_owner {
        return Err(AmmError::InvalidOwner.into());
    }

    // Update parameters based on param field
    match AmmParams::from_u64(setparams.param.into()) {
        AmmParams::Status => {
            if let Some(value) = setparams.value {
                amm.status = value;
            }
        }
        AmmParams::State => {
            if let Some(value) = setparams.value {
                amm.state = value;
            }
        }
        AmmParams::OrderNum => {
            if let Some(value) = setparams.value {
                amm.order_num = value;
            }
        }
        AmmParams::Depth => {
            if let Some(value) = setparams.value {
                amm.depth = value;
            }
        }
        AmmParams::AmountWave => {
            if let Some(value) = setparams.value {
                amm.amount_wave = value;
            }
        }
        AmmParams::MinPriceMultiplier => {
            if let Some(value) = setparams.value {
                amm.min_price_multiplier = value;
            }
        }
        AmmParams::MaxPriceMultiplier => {
            if let Some(value) = setparams.value {
                amm.max_price_multiplier = value;
            }
        }
        AmmParams::MinSize => {
            if let Some(value) = setparams.value {
                amm.min_size = value;
            }
        }
        AmmParams::VolMaxCutRatio => {
            if let Some(value) = setparams.value {
                amm.vol_max_cut_ratio = value;
            }
        }
        AmmParams::Fees => {
            if let Some(fees) = setparams.fees {
                amm.fees = fees;
            }
        }
        AmmParams::AmmOwner => {
            if let Some(new_owner) = setparams.new_pubkey {
                amm.amm_owner = new_owner;
            }
        }
        AmmParams::SetOpenTime => {
            if let Some(value) = setparams.value {
                amm.state_data.pool_open_time = value;
            }
        }
        AmmParams::LastOrderDistance => {
            if let Some(last_order_distance) = setparams.last_order_distance {
                // Update target orders with new last order distance
                // This would need to be implemented based on the target orders structure
            }
        }
        AmmParams::InitOrderDepth => {
            // Implementation for init order depth
        }
        AmmParams::SetSwitchTime => {
            // Implementation for set switch time
        }
        AmmParams::ClearOpenTime => {
            amm.state_data.pool_open_time = 0;
        }
        AmmParams::Seperate => {
            // Implementation for separate
        }
        AmmParams::UpdateOpenOrder => {
            // Implementation for update open order
        }
    }

    msg!("Parameters updated successfully");
    Ok(())
} 