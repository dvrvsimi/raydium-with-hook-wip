use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    program_error::ProgramError,
    entrypoint::ProgramResult,
};

use crate::state::{AmmInfo, TargetOrders, AmmStatus};
use crate::math::{U256, U128, InvariantPool, RoundDirection};
use crate::error::AmmError;
use crate::instruction::WithdrawInstruction;
use crate::invokers::Invokers;
use crate::process::constants::AUTHORITY_AMM;
use serum_dex::state::{MarketState, OpenOrders, ToAlignedBytes};
use std::cell::Ref;
use serum_dex::critbit::{LeafNode, Slab, SlabView};
use spl_token::solana_program::program_pack::Pack;

pub const LOG_SIZE: usize = 256;

pub fn authority_id(
    program_id: &Pubkey,
    amm_seed: &[u8],
    nonce: u8,
) -> Result<Pubkey, AmmError> {
    Pubkey::create_program_address(&[amm_seed, &[nonce]], program_id)
        .map_err(|_| AmmError::InvalidProgramAddress.into())
}

pub fn unpack_token_account(
    account_info: &AccountInfo,
    token_program_id: &Pubkey,
) -> Result<spl_token::state::Account, AmmError> {
    if account_info.owner != token_program_id {
        Err(AmmError::InvalidSplTokenProgram)
    } else {
        spl_token::state::Account::unpack(&account_info.data.borrow())
            .map_err(|_| AmmError::ExpectedAccount)
    }
}

pub fn unpack_mint(
    account_info: &AccountInfo,
    token_program_id: &Pubkey,
) -> Result<spl_token::state::Mint, AmmError> {
    if account_info.owner != token_program_id {
        Err(AmmError::InvalidSplTokenProgram)
    } else {
        spl_token::state::Mint::unpack(&account_info.data.borrow())
            .map_err(|_| AmmError::ExpectedMint)
    }
}

pub fn load_serum_market_order<'a>(
    market_acc: &AccountInfo<'a>,
    open_orders_acc: &AccountInfo<'a>,
    authority_acc: &AccountInfo<'a>,
    amm: &AmmInfo,
    allow_disabled: bool,
) -> Result<(Box<MarketState>, Box<OpenOrders>), ProgramError> {
    let market_state = MarketState::load_checked(market_acc, &amm.market_program, allow_disabled)?;
    let open_orders = OpenOrders::load_checked(
        open_orders_acc,
        Some(market_acc),
        Some(authority_acc),
        &amm.market_program,
    )?;
    if identity(open_orders.market) != market_acc.key.to_aligned_bytes() {
        return Err(AmmError::InvalidMarket.into());
    }
    if identity(open_orders.owner) != authority_acc.key.to_aligned_bytes() {
        return Err(AmmError::InvalidOwner.into());
    }
    if *open_orders_acc.key != amm.open_orders {
        return Err(AmmError::InvalidOpenOrders.into());
    }
    Ok((Box::new(*market_state), Box::new(*open_orders)))
}

pub fn get_amm_orders(
    open_orders: &OpenOrders,
    bids: Ref<Slab>,
    asks: Ref<Slab>,
) -> Result<(Vec<LeafNode>, Vec<LeafNode>), ProgramError> {
    let orders_number = open_orders.free_slot_bits.count_zeros();
    let mut bids_orders: Vec<LeafNode> = Vec::new();
    let mut asks_orders: Vec<LeafNode> = Vec::new();
    if orders_number != 0 {
        for i in 0..128 {
            let slot_mask = 1u128 << i;
            if open_orders.free_slot_bits & slot_mask != 0 {
                continue;
            }
            if open_orders.is_bid_bits & slot_mask != 0 {
                match bids.find_by_key(open_orders.orders[i]) {
                    None => continue,
                    Some(handle_bid) => {
                        let handle_bid_ref = bids.get(handle_bid).unwrap().as_leaf().unwrap();
                        bids_orders.push(*handle_bid_ref);
                    }
                }
            } else {
                match asks.find_by_key(open_orders.orders[i]) {
                    None => continue,
                    Some(handle_ask) => {
                        let handle_ask_ref = asks.get(handle_ask).unwrap().as_leaf().unwrap();
                        asks_orders.push(*handle_ask_ref);
                    }
                }
            };
        }
    }
    bids_orders.sort_by(|a, b| b.price().get().cmp(&a.price().get()));
    asks_orders.sort_by(|a, b| a.price().get().cmp(&b.price().get()));
    Ok((bids_orders, asks_orders))
}

/// Checks if an account is readonly
pub fn check_account_readonly(account_info: &AccountInfo) -> ProgramResult {
    if account_info.is_writable {
        return Err(AmmError::InvalidSignAccount.into());
    }
    Ok(())
}

/// Calculate take PnL for target orders
pub fn calc_take_pnl(
    target: &TargetOrders,
    amm: &mut AmmInfo,
    total_pc_without_take_pnl: &mut u64,
    total_coin_without_take_pnl: &mut u64,
    x1: U256,
    y1: U256,
) -> Result<(u128, u128), ProgramError> {
    // Simplified PnL calculation
    let delta_x = target.calc_pnl_x;
    let delta_y = target.calc_pnl_y;
    
    // Update AMM state data
    amm.state_data.total_pnl_pc = amm.state_data.total_pnl_pc.saturating_add(delta_x as u64);
    amm.state_data.total_pnl_coin = amm.state_data.total_pnl_coin.saturating_add(delta_y as u64);
    
    // Update totals
    *total_pc_without_take_pnl = total_pc_without_take_pnl.saturating_sub(delta_x as u64);
    *total_coin_without_take_pnl = total_coin_without_take_pnl.saturating_sub(delta_y as u64);
    
    Ok((delta_x, delta_y))
}

/// Update target orders PnL
pub fn update_target_orders_pnl(
    target_orders: &mut TargetOrders,
    x1: U128,
    y1: U128,
    pc_amount: u64,
    coin_amount: u64,
    delta_x: u128,
    delta_y: u128,
    amm: &AmmInfo,
) {
    // Update target orders with new PnL calculations
    target_orders.calc_pnl_x = delta_x;
    target_orders.calc_pnl_y = delta_y;
    target_orders.target_x = x1.as_u128();
    target_orders.target_y = y1.as_u128();
}

/// Validates slippage for withdrawal
pub fn validate_withdraw_slippage(
    withdraw: &WithdrawInstruction,
    coin_amount: u64,
    pc_amount: u64,
) -> Result<(), AmmError> {
    if withdraw.min_coin_amount.is_some() && withdraw.min_pc_amount.is_some() {
        if withdraw.min_coin_amount.unwrap() > coin_amount
            || withdraw.min_pc_amount.unwrap() > pc_amount
        {
            return Err(AmmError::ExceededSlippage);
        }
    }
    Ok(())
}

pub fn identity<T>(x: T) -> T { x }

/// Gets the associated address and bump seed for a given market and seed
pub fn get_associated_address_and_bump_seed(
    info_id: &Pubkey,
    market_address: &Pubkey,
    associated_seed: &[u8],
    program_id: &Pubkey,
) -> (Pubkey, u8) {
    let seeds = &[
        info_id.as_ref(),
        market_address.as_ref(),
        associated_seed,
    ];
    Pubkey::find_program_address(seeds, program_id)
}

/// Validates withdraw permissions and basic account checks
pub fn validate_withdraw_permissions(
    amm: &AmmInfo,
    amm_authority_info: &AccountInfo,
    source_lp_owner_info: &AccountInfo,
    program_id: &Pubkey,
) -> Result<(), AmmError> {
    if !AmmStatus::from_u64(amm.status).withdraw_permission() {
        return Err(AmmError::InvalidStatus);
    }
    
    if *amm_authority_info.key != authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)? {
        return Err(AmmError::InvalidProgramAddress);
    }
    
    if !source_lp_owner_info.is_signer {
        return Err(AmmError::InvalidSignAccount);
    }
    
    Ok(())
}

/// Validates vault accounts for withdraw operations
pub fn validate_withdraw_vaults(
    amm: &AmmInfo,
    amm_coin_vault_info: &AccountInfo,
    amm_pc_vault_info: &AccountInfo,
    user_dest_coin_info: &AccountInfo,
    user_dest_pc_info: &AccountInfo,
) -> Result<(), AmmError> {
    if *amm_coin_vault_info.key != amm.coin_vault || *user_dest_coin_info.key == amm.coin_vault {
        return Err(AmmError::InvalidCoinVault);
    }
    if *amm_pc_vault_info.key != amm.pc_vault || *user_dest_pc_info.key == amm.pc_vault {
        return Err(AmmError::InvalidPCVault);
    }
    Ok(())
}

/// Validates LP token withdrawal amounts
pub fn validate_lp_withdrawal(
    withdraw_amount: u64,
    user_lp_amount: u64,
    lp_mint_supply: u64,
    amm_lp_amount: u64,
) -> Result<(), AmmError> {
    if withdraw_amount > user_lp_amount {
        return Err(AmmError::InsufficientFunds);
    }
    if withdraw_amount > lp_mint_supply || withdraw_amount >= amm_lp_amount {
        return Err(AmmError::NotAllowZeroLP);
    }
    Ok(())
}

/// Cancels all AMM orders and settles funds
pub fn cancel_amm_orders_and_settle<'a>(
    market_program_info: &AccountInfo<'a>,
    market_info: &AccountInfo<'a>,
    market_bids_info: &AccountInfo<'a>,
    market_asks_info: &AccountInfo<'a>,
    amm_open_orders_info: &AccountInfo<'a>,
    amm_authority_info: &AccountInfo<'a>,
    market_event_q_info: &AccountInfo<'a>,
    market_coin_vault_info: &AccountInfo<'a>,
    market_pc_vault_info: &AccountInfo<'a>,
    amm_coin_vault_info: &AccountInfo<'a>,
    amm_pc_vault_info: &AccountInfo<'a>,
    market_vault_signer: &AccountInfo<'a>,
    token_program_info: &AccountInfo<'a>,
    referrer_pc_wallet: Option<&AccountInfo<'a>>,
    bids: &[LeafNode],
    asks: &[LeafNode],
    amm_nonce: u8,
) -> Result<(), ProgramError> {
    // Cancel all orders
    let mut amm_order_ids_vec = Vec::new();
    let mut order_ids = [0u64; 8];
    let mut count = 0;
    
    for i in 0..std::cmp::max(bids.len(), asks.len()) {
        if i < bids.len() {
            order_ids[count] = bids[i].client_order_id();
            count += 1;
        }
        if i < asks.len() {
            order_ids[count] = asks[i].client_order_id();
            count += 1;
        }
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
            market_event_q_info.clone(),
            AUTHORITY_AMM,
            amm_nonce,
            *ids,
        )?;
    }

    // Settle funds
    Invokers::invoke_dex_settle_funds(
        market_program_info.clone(),
        market_info.clone(),
        amm_open_orders_info.clone(),
        amm_authority_info.clone(),
        market_coin_vault_info.clone(),
        market_pc_vault_info.clone(),
        amm_coin_vault_info.clone(),
        amm_pc_vault_info.clone(),
        market_vault_signer.clone(),
        token_program_info.clone(),
        referrer_pc_wallet,
        AUTHORITY_AMM,
        amm_nonce,
    )?;

    Ok(())
}

/// Calculates withdrawal amounts based on LP token amount
pub fn calculate_withdrawal_amounts(
    withdraw_amount: u64,
    amm_lp_amount: u64,
    total_coin_without_take_pnl: u64,
    total_pc_without_take_pnl: u64,
) -> Result<(u64, u64), AmmError> {
    let invariant = InvariantPool {
        token_input: withdraw_amount,
        token_total: amm_lp_amount,
    };
    
    let coin_amount = invariant
        .exchange_pool_to_token(total_coin_without_take_pnl, RoundDirection::Floor)
        .ok_or(AmmError::CalculationExRateFailure)?;
    let pc_amount = invariant
        .exchange_pool_to_token(total_pc_without_take_pnl, RoundDirection::Floor)
        .ok_or(AmmError::CalculationExRateFailure)?;
    
    Ok((coin_amount, pc_amount))
} 