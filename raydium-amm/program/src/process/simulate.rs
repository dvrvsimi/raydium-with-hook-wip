//! Handles the simulate instruction logic for Raydium AMM
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    program_error::ProgramError,
    pubkey::Pubkey,
    msg,
};
use crate::{
    error::AmmError,
    instruction::{SimulateInstruction, SwapInstructionBaseIn, SwapInstructionBaseOut},
    state::{AmmInfo, AmmStatus, SimulateParams},
    math::{Calculator, SwapDirection, U128, CheckedCeilDiv},
};
use crate::process::constants::AUTHORITY_AMM;
use crate::process::helpers::{authority_id, load_serum_market_order, unpack_token_account, unpack_mint, identity};

pub fn process_simulate_info(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    simulate: SimulateInstruction,
) -> ProgramResult {
    match SimulateParams::from_u64(simulate.param.into()) {
        SimulateParams::PoolInfo => {
            let pool_data = simulate_pool_info(program_id, accounts)?;
            msg!("Pool data: {:?}", pool_data);
            Ok(())
        }
        SimulateParams::SwapBaseInInfo => {
            if let Some(swap) = simulate.swap_base_in_value {
                let swap_data = simulate_swap_base_in(program_id, accounts, swap)?;
                msg!("Swap base in data: {:?}", swap_data);
            }
            Ok(())
        }
        SimulateParams::SwapBaseOutInfo => {
            if let Some(swap) = simulate.swap_base_out_value {
                let swap_data = simulate_swap_base_out(program_id, accounts, swap)?;
                msg!("Swap base out data: {:?}", swap_data);
            }
            Ok(())
        }
        SimulateParams::RunCrankInfo => {
            let crank_data = simulate_run_crank(program_id, accounts)?;
            msg!("Run crank data: {:?}", crank_data);
            Ok(())
        }
    }
}

fn simulate_pool_info(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> Result<crate::state::GetPoolData, ProgramError> {
    let account_info_iter = &mut accounts.iter();
    let amm_info = next_account_info(account_info_iter)?;
    let amm_authority_info = next_account_info(account_info_iter)?;
    let amm_open_orders_info = next_account_info(account_info_iter)?;
    let amm_coin_vault_info = next_account_info(account_info_iter)?;
    let amm_pc_vault_info = next_account_info(account_info_iter)?;
    let amm_lp_mint_info = next_account_info(account_info_iter)?;
    let market_info = next_account_info(account_info_iter)?;
    let market_event_queue_info = next_account_info(account_info_iter)?;
    let _amm_target_orders_info = next_account_info(account_info_iter)?;

    let amm = AmmInfo::load_checked(&amm_info, program_id)?;

    // Check authority
    let authority = authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?;
    if *amm_authority_info.key != authority {
        return Err(AmmError::InvalidProgramAddress.into());
    }

    // Unpack token accounts
    let amm_coin_vault = unpack_token_account(&amm_coin_vault_info, &spl_token::id())?;
    let amm_pc_vault = unpack_token_account(&amm_pc_vault_info, &spl_token::id())?;
    let amm_lp_mint = unpack_mint(&amm_lp_mint_info, &spl_token::id())?;

    // Calculate pool data
    let pool_data = crate::state::GetPoolData {
        status: amm.status,
        coin_decimals: amm.coin_decimals,
        pc_decimals: amm.pc_decimals,
        lp_decimals: amm_lp_mint.decimals.into(),
        pool_pc_amount: amm_pc_vault.amount,
        pool_coin_amount: amm_coin_vault.amount,
        pnl_pc_amount: amm.state_data.total_pnl_pc,
        pnl_coin_amount: amm.state_data.total_pnl_coin,
        pool_lp_supply: amm_lp_mint.supply,
        pool_open_time: amm.state_data.pool_open_time,
        amm_id: amm_info.key.to_string(),
    };

    Ok(pool_data)
}

fn simulate_swap_base_in(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    swap: SwapInstructionBaseIn,
) -> Result<crate::state::GetSwapBaseInData, ProgramError> {
    let account_info_iter = &mut accounts.iter();
    let amm_info = next_account_info(account_info_iter)?;
    let amm_authority_info = next_account_info(account_info_iter)?;
    let amm_open_orders_info = next_account_info(account_info_iter)?;
    let amm_target_orders_info = next_account_info(account_info_iter)?;
    let amm_coin_vault_info = next_account_info(account_info_iter)?;
    let amm_pc_vault_info = next_account_info(account_info_iter)?;
    let amm_lp_mint_info = next_account_info(account_info_iter)?;
    let market_program_info = next_account_info(account_info_iter)?;
    let market_info = next_account_info(account_info_iter)?;
    let market_event_queue_info = next_account_info(account_info_iter)?;
    let user_source_info = next_account_info(account_info_iter)?;
    let user_destination_info = next_account_info(account_info_iter)?;
    let _user_source_owner = next_account_info(account_info_iter)?;

    let amm = AmmInfo::load_checked(&amm_info, program_id)?;

    // Check authority
    let authority = authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?;
    if *amm_authority_info.key != authority {
        return Err(AmmError::InvalidProgramAddress.into());
    }

    // Unpack token accounts
    let amm_coin_vault = unpack_token_account(&amm_coin_vault_info, &spl_token::id())?;
    let amm_pc_vault = unpack_token_account(&amm_pc_vault_info, &spl_token::id())?;
    let user_source = unpack_token_account(&user_source_info, &spl_token::id())?;
    let user_destination = unpack_token_account(&user_destination_info, &spl_token::id())?;

    // Determine swap direction
    let swap_direction = if user_source.mint == amm_coin_vault.mint && user_destination.mint == amm_pc_vault.mint {
        SwapDirection::Coin2PC
    } else if user_source.mint == amm_pc_vault.mint && user_destination.mint == amm_coin_vault.mint {
        SwapDirection::PC2Coin
    } else {
        return Err(AmmError::InvalidUserToken.into());
    };

    // Calculate swap amount
    let swap_fee = U128::from(swap.amount_in)
        .checked_mul(amm.fees.swap_fee_numerator.into())
        .unwrap()
        .checked_ceil_div(amm.fees.swap_fee_denominator.into())
        .unwrap()
        .0;

    let swap_in_after_deduct_fee = U128::from(swap.amount_in).checked_sub(swap_fee).unwrap();
    
    // Calculate output amount (simplified calculation)
    let swap_amount_out = if swap_direction == SwapDirection::Coin2PC {
        amm_pc_vault.amount.saturating_sub(1) // Simplified calculation
    } else {
        amm_coin_vault.amount.saturating_sub(1) // Simplified calculation
    };

    let pool_data = simulate_pool_info(program_id, accounts)?;

    let swap_data = crate::state::GetSwapBaseInData {
        pool_data,
        amount_in: swap.amount_in,
        minimum_amount_out: swap.minimum_amount_out,
        price_impact: 0, // Simplified
    };

    Ok(swap_data)
}

fn simulate_swap_base_out(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    swap: SwapInstructionBaseOut,
) -> Result<crate::state::GetSwapBaseOutData, ProgramError> {
    let account_info_iter = &mut accounts.iter();
    let amm_info = next_account_info(account_info_iter)?;
    let amm_authority_info = next_account_info(account_info_iter)?;
    let amm_open_orders_info = next_account_info(account_info_iter)?;
    let amm_target_orders_info = next_account_info(account_info_iter)?;
    let amm_coin_vault_info = next_account_info(account_info_iter)?;
    let amm_pc_vault_info = next_account_info(account_info_iter)?;
    let amm_lp_mint_info = next_account_info(account_info_iter)?;
    let market_program_info = next_account_info(account_info_iter)?;
    let market_info = next_account_info(account_info_iter)?;
    let market_event_queue_info = next_account_info(account_info_iter)?;
    let user_source_info = next_account_info(account_info_iter)?;
    let user_destination_info = next_account_info(account_info_iter)?;
    let _user_source_owner = next_account_info(account_info_iter)?;

    let amm = AmmInfo::load_checked(&amm_info, program_id)?;

    // Check authority
    let authority = authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?;
    if *amm_authority_info.key != authority {
        return Err(AmmError::InvalidProgramAddress.into());
    }

    // Unpack token accounts
    let amm_coin_vault = unpack_token_account(&amm_coin_vault_info, &spl_token::id())?;
    let amm_pc_vault = unpack_token_account(&amm_pc_vault_info, &spl_token::id())?;
    let user_source = unpack_token_account(&user_source_info, &spl_token::id())?;
    let user_destination = unpack_token_account(&user_destination_info, &spl_token::id())?;

    // Determine swap direction
    let swap_direction = if user_source.mint == amm_coin_vault.mint && user_destination.mint == amm_pc_vault.mint {
        SwapDirection::Coin2PC
    } else if user_source.mint == amm_pc_vault.mint && user_destination.mint == amm_coin_vault.mint {
        SwapDirection::PC2Coin
    } else {
        return Err(AmmError::InvalidUserToken.into());
    };

    // Calculate input amount (simplified calculation)
    let max_amount_in = if swap_direction == SwapDirection::Coin2PC {
        amm_coin_vault.amount.saturating_add(1) // Simplified calculation
    } else {
        amm_pc_vault.amount.saturating_add(1) // Simplified calculation
    };

    let pool_data = simulate_pool_info(program_id, accounts)?;

    let swap_data = crate::state::GetSwapBaseOutData {
        pool_data,
        max_amount_in,
        amount_out: swap.amount_out,
        price_impact: 0, // Simplified
    };

    Ok(swap_data)
}

fn simulate_run_crank(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
) -> Result<crate::state::RunCrankData, ProgramError> {
    let account_info_iter = &mut accounts.iter();
    let amm_info = next_account_info(account_info_iter)?;
    let amm_authority_info = next_account_info(account_info_iter)?;
    let amm_open_orders_info = next_account_info(account_info_iter)?;
    let amm_target_orders_info = next_account_info(account_info_iter)?;
    let amm_coin_vault_info = next_account_info(account_info_iter)?;
    let amm_pc_vault_info = next_account_info(account_info_iter)?;
    let market_program_info = next_account_info(account_info_iter)?;
    let market_info = next_account_info(account_info_iter)?;
    let market_bids_info = next_account_info(account_info_iter)?;
    let market_asks_info = next_account_info(account_info_iter)?;
    let market_event_queue_info = next_account_info(account_info_iter)?;

    let amm = AmmInfo::load_checked(&amm_info, program_id)?;

    // Check authority
    let authority = authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?;
    if *amm_authority_info.key != authority {
        return Err(AmmError::InvalidProgramAddress.into());
    }

    let crank_data = crate::state::RunCrankData {
        status: amm.status,
        state: amm.state,
        run_crank: true,
    };

    Ok(crank_data)
} 