//! Handles the admin_cancel_orders instruction logic for Raydium AMM
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    msg,
};
use crate::{
    error::AmmError,
    instruction::AdminCancelOrdersInstruction,
    invokers::Invokers,
    state::AmmInfo,
};
use crate::process::constants::AUTHORITY_AMM;
use crate::process::helpers::{authority_id, load_serum_market_order, get_amm_orders};

pub fn process_admin_cancel_orders(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    _cancel: AdminCancelOrdersInstruction,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let amm_info = next_account_info(account_info_iter)?;
    let amm_authority_info = next_account_info(account_info_iter)?;
    let amm_open_orders_info = next_account_info(account_info_iter)?;
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

    let amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;

    // Check authority
    let authority = authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?;
    if *amm_authority_info.key != authority {
        return Err(AmmError::InvalidProgramAddress.into());
    }

    // Check if user is owner
    if *user_wallet_info.key != amm.amm_owner {
        return Err(AmmError::InvalidOwner.into());
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

    // Cancel all bids
    if !bids.is_empty() {
        let mut amm_order_ids_vec = Vec::new();
        let mut order_ids = [0u64; 8];
        let mut count = 0;
        
        for order in bids.into_iter() {
            order_ids[count] = order.client_order_id();
            count += 1;
            if count == 8 {
                amm_order_ids_vec.push(order_ids);
                order_ids = [0u64; 8];
                count = 0;
            }
        }
        if count != 0 {
            amm_order_ids_vec.push(order_ids);
        }
        
        for ids in amm_order_ids_vec.iter() {
            Invokers::invoke_dex_cancel_orders_by_client_order_ids(
                market_program_info.clone(),
                market_info.clone(),
                market_bids_info.clone(),
                market_asks_info.clone(),
                amm_open_orders_info.clone(),
                amm_authority_info.clone(),
                market_event_queue_info.clone(),
                AUTHORITY_AMM,
                amm.nonce as u8,
                *ids,
            )?;
        }
    }

    // Cancel all asks
    if !asks.is_empty() {
        let mut amm_order_ids_vec = Vec::new();
        let mut order_ids = [0u64; 8];
        let mut count = 0;
        
        for order in asks.into_iter() {
            order_ids[count] = order.client_order_id();
            count += 1;
            if count == 8 {
                amm_order_ids_vec.push(order_ids);
                order_ids = [0u64; 8];
                count = 0;
            }
        }
        if count != 0 {
            amm_order_ids_vec.push(order_ids);
        }
        
        for ids in amm_order_ids_vec.iter() {
            Invokers::invoke_dex_cancel_orders_by_client_order_ids(
                market_program_info.clone(),
                market_info.clone(),
                market_bids_info.clone(),
                market_asks_info.clone(),
                amm_open_orders_info.clone(),
                amm_authority_info.clone(),
                market_event_queue_info.clone(),
                AUTHORITY_AMM,
                amm.nonce as u8,
                *ids,
            )?;
        }
    }

    // Settle funds
    Invokers::invoke_dex_settle_funds(
        market_program_info.clone(),
        market_info.clone(),
        amm_open_orders_info.clone(),
        amm_authority_info.clone(),
        market_coin_vault_info.clone(),
        market_pc_vault_info.clone(),
        market_coin_vault_info.clone(),
        market_pc_vault_info.clone(),
        market_vault_signer.clone(),
        token_program_info.clone(),
        None,
        AUTHORITY_AMM,
        amm.nonce as u8,
    )?;

    msg!("Admin orders cancelled successfully");
    Ok(())
} 