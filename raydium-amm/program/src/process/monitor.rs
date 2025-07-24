//! Handles the monitor_step instruction logic for Raydium AMM
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    msg,
};
use crate::{
    error::AmmError,
    instruction::MonitorStepInstruction,
    state::{AmmInfo, TargetOrders},
    math::{Calculator, U256, U128},
};
use crate::process::constants::AUTHORITY_AMM;
use crate::process::helpers::{authority_id, load_serum_market_order, get_amm_orders, unpack_token_account};

pub fn process_monitor_step(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _monitor: MonitorStepInstruction,
) -> ProgramResult {
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
    let market_coin_vault_info = next_account_info(account_info_iter)?;
    let market_pc_vault_info = next_account_info(account_info_iter)?;
    let market_vault_signer = next_account_info(account_info_iter)?;
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

    // Check market accounts
    if *market_info.key != amm.market {
        return Err(AmmError::InvalidMarket.into());
    }
    if *amm_open_orders_info.key != amm.open_orders {
        return Err(AmmError::InvalidOpenOrders.into());
    }
    if *amm_target_orders_info.key != amm.target_orders {
        return Err(AmmError::InvalidTargetOrders.into());
    }

    // Load market state
    let (market_state, open_orders) = load_serum_market_order(
        market_info,
        amm_open_orders_info,
        amm_authority_info,
        &amm,
        false,
    )?;

    let bids_orders = market_state.load_bids_checked(&market_bids_info)?;
    let asks_orders = market_state.load_asks_checked(&market_asks_info)?;
    let (bids, asks) = get_amm_orders(&open_orders, bids_orders, asks_orders)?;

    // Load target orders
    let mut target_orders = TargetOrders::load_mut_checked(&amm_target_orders_info, program_id, amm_info.key)?;

    // Get vault amounts
    let amm_coin_vault = unpack_token_account(&amm_coin_vault_info, token_program_info.key)?;
    let amm_pc_vault = unpack_token_account(&amm_pc_vault_info, token_program_info.key)?;

    // Calculate total amounts
    let (mut total_pc_without_take_pnl, mut total_coin_without_take_pnl) =
        Calculator::calc_total_without_take_pnl(
            amm_pc_vault.amount,
            amm_coin_vault.amount,
            &open_orders,
            &amm,
            &market_state,
            &market_event_queue_info,
            &amm_open_orders_info,
        )?;

    // Process monitor step - calculate PnL
    let total_pc_u256 = U256::from(total_pc_without_take_pnl);
    let total_coin_u256 = U256::from(total_coin_without_take_pnl);
    
    let (delta_x, delta_y) = crate::process::helpers::calc_take_pnl(
        &target_orders,
        &mut amm,
        &mut total_pc_without_take_pnl,
        &mut total_coin_without_take_pnl,
        total_pc_u256,
        total_coin_u256,
    )?;

    // Update target orders PnL
    crate::process::helpers::update_target_orders_pnl(
        &mut target_orders,
        U128::from(total_pc_without_take_pnl),
        U128::from(total_coin_without_take_pnl),
        total_pc_without_take_pnl,
        total_coin_without_take_pnl,
        delta_x,
        delta_y,
        &amm,
    );

    // Cancel orders if needed
    if !bids.is_empty() || !asks.is_empty() {
        crate::process::helpers::cancel_amm_orders_and_settle(
            market_program_info,
            market_info,
            market_bids_info,
            market_asks_info,
            amm_open_orders_info,
            amm_authority_info,
            market_event_queue_info,
            market_coin_vault_info,
            market_pc_vault_info,
            amm_coin_vault_info,
            amm_pc_vault_info,
            market_vault_signer,
            token_program_info,
            None,
            &bids,
            &asks,
            amm.nonce as u8,
        )?;
    }

    msg!("Monitor step completed successfully");
    Ok(())
} 