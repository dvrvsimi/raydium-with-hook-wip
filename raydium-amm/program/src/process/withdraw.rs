//! Handles the withdraw instruction logic for Raydium AMM
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use crate::{
    error::AmmError,
    instruction::WithdrawInstruction,
    invokers::Invokers,
    math::{Calculator, RoundDirection, InvariantPool, U128},
    state::{AmmInfo, AmmStatus, TargetOrders},
};
use crate::process::constants::AUTHORITY_AMM;
use crate::process::config;
use crate::process::helpers::{identity, authority_id, unpack_token_account, unpack_mint, load_serum_market_order, calc_take_pnl, get_amm_orders};
use crate::process::args::LogType;
use crate::log::WithdrawLog;
use serum_dex::state::ToAlignedBytes;
use crate::log::{log_keys_mismatch, encode_ray_log};
use crate::check_assert_eq;

/// The number of accounts expected for a withdraw instruction.
/// This is based on the order of next_account_info calls in the function:
/// [token_program_info, amm_info, amm_authority_info, amm_open_orders_info, amm_target_orders_info, amm_lp_mint_info, amm_coin_vault_info, amm_pc_vault_info, market_program_info, market_info, market_coin_vault_info, market_pc_vault_info, market_vault_signer, user_source_lp_info, user_dest_coin_info, user_dest_pc_info, source_lp_owner_info, market_event_q_info, market_bids_info, market_asks_info]
/// = 20 accounts (base) + optional referrer_pc_wallet
const ACCOUNT_LEN: usize = 20;

pub fn process_withdraw(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    withdraw: WithdrawInstruction,
) -> ProgramResult {
    let input_account_len = accounts.len();
    if input_account_len != ACCOUNT_LEN
        && input_account_len != ACCOUNT_LEN + 1
        && input_account_len != ACCOUNT_LEN + 2
        && input_account_len != ACCOUNT_LEN + 3
    {
        return Err(AmmError::WrongAccountsNumber.into());
    }
    
    let account_info_iter = &mut accounts.iter();
    let token_program_info = next_account_info(account_info_iter)?;
    let amm_info = next_account_info(account_info_iter)?;
    let amm_authority_info = next_account_info(account_info_iter)?;
    let amm_open_orders_info = next_account_info(account_info_iter)?;
    let amm_target_orders_info = next_account_info(account_info_iter)?;
            let amm_lp_mint_info = next_account_info(account_info_iter)?;
        let amm_coin_vault_info = next_account_info(account_info_iter)?;
        let amm_pc_vault_info = next_account_info(account_info_iter)?;
        let amm_coin_mint_info = next_account_info(account_info_iter)?;
        let amm_pc_mint_info = next_account_info(account_info_iter)?;
    
    // Handle optional padding accounts
    if input_account_len == ACCOUNT_LEN + 2 || input_account_len == ACCOUNT_LEN + 3 {
        let _padding_account_info1 = next_account_info(account_info_iter)?;
        let _padding_account_info2 = next_account_info(account_info_iter)?;
    }

    let market_program_info = next_account_info(account_info_iter)?;
    let market_info = next_account_info(account_info_iter)?;
    let market_coin_vault_info = next_account_info(account_info_iter)?;
    let market_pc_vault_info = next_account_info(account_info_iter)?;
    let market_vault_signer = next_account_info(account_info_iter)?;

    let user_source_lp_info = next_account_info(account_info_iter)?;
    let user_dest_coin_info = next_account_info(account_info_iter)?;
    let user_dest_pc_info = next_account_info(account_info_iter)?;
    let source_lp_owner_info = next_account_info(account_info_iter)?;

    let market_event_q_info = next_account_info(account_info_iter)?;
    let market_bids_info = next_account_info(account_info_iter)?;
    let market_asks_info = next_account_info(account_info_iter)?;

    // Handle optional referrer PC wallet
    let mut referrer_pc_wallet = None;
    if input_account_len == ACCOUNT_LEN + 1 || input_account_len == ACCOUNT_LEN + 3 {
        referrer_pc_wallet = Some(next_account_info(account_info_iter)?);
        // Validate referrer PC wallet if provided
        if *referrer_pc_wallet.unwrap().key != Pubkey::default() {
            let referrer_pc_token = unpack_token_account(
                &referrer_pc_wallet.unwrap(),
                token_program_info.key,
            )?;
            check_assert_eq!(
                referrer_pc_token.owner,
                config::referrer_pc_wallet::id()?,
                "referrer_pc_owner",
                AmmError::InvalidOwner
            );
        }
    }

    if referrer_pc_wallet.is_none() {
        referrer_pc_wallet = Some(amm_pc_vault_info);
    }

    if !source_lp_owner_info.is_signer {
        return Err(AmmError::InvalidSignAccount.into());
    }

    let mut amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;
    let mut target_orders = TargetOrders::load_mut_checked(&amm_target_orders_info, program_id, amm_info.key)?;

    if !AmmStatus::from_u64(amm.status).withdraw_permission() {
        return Err(AmmError::InvalidStatus.into());
    }

    if *amm_authority_info.key != authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)? {
        return Err(AmmError::InvalidProgramAddress.into());
    }

    let enable_orderbook = AmmStatus::from_u64(amm.status).orderbook_permission();
    let spl_token_program_id = token_program_info.key;

    // Validate token program
    check_assert_eq!(
        *token_program_info.key,
        spl_token::id(),
        "spl_token_program",
        AmmError::InvalidSplTokenProgram
    );

    // Validate vault accounts
    if *amm_coin_vault_info.key != amm.coin_vault || *user_dest_coin_info.key == amm.coin_vault {
        return Err(AmmError::InvalidCoinVault.into());
    }
    if *amm_pc_vault_info.key != amm.pc_vault || *user_dest_pc_info.key == amm.pc_vault {
        return Err(AmmError::InvalidPCVault.into());
    }

    check_assert_eq!(
        *amm_target_orders_info.key,
        amm.target_orders,
        "target_orders",
        AmmError::InvalidTargetOrders
    );
    check_assert_eq!(
        *amm_lp_mint_info.key,
        amm.lp_mint,
        "lp_mint",
        AmmError::InvalidPoolMint
    );

    let amm_coin_vault = unpack_token_account(&amm_coin_vault_info, spl_token_program_id)?;
    let amm_pc_vault = unpack_token_account(&amm_pc_vault_info, spl_token_program_id)?;
    let user_dest_coin = unpack_token_account(&user_dest_coin_info, spl_token_program_id)?;
    let user_dest_pc = unpack_token_account(&user_dest_pc_info, spl_token_program_id)?;

    let lp_mint = unpack_mint(&amm_lp_mint_info, spl_token_program_id)?;
    let user_source_lp = unpack_token_account(&user_source_lp_info, spl_token_program_id)?;

    if user_source_lp.mint != *amm_lp_mint_info.key {
        return Err(AmmError::InvalidTokenLP.into());
    }

    if withdraw.amount > user_source_lp.amount {
        return Err(AmmError::InsufficientFunds.into());
    }

    if withdraw.amount > lp_mint.supply || withdraw.amount >= amm.lp_amount {
        return Err(AmmError::NotAllowZeroLP.into());
    }

    let (mut total_pc_without_take_pnl, mut total_coin_without_take_pnl) = if enable_orderbook {
        // Validate market accounts
        check_assert_eq!(
            *market_info.key,
            amm.market,
            "market",
            AmmError::InvalidMarket
        );
        check_assert_eq!(
            *market_program_info.key,
            amm.market_program,
            "market_program",
            AmmError::InvalidMarketProgram
        );
        check_assert_eq!(
            *amm_open_orders_info.key,
            amm.open_orders,
            "open_orders",
            AmmError::InvalidOpenOrders
        );

        // Load market state and orders
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

        // Cancel all orders and settle funds
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
                amm.nonce as u8,
                *ids,
            )?;
        }

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
            referrer_pc_wallet.clone(),
            AUTHORITY_AMM,
            amm.nonce as u8,
        )?;

        if identity(market_state.coin_mint) != amm_coin_vault.mint.to_aligned_bytes()
            || identity(market_state.coin_mint) != user_dest_coin.mint.to_aligned_bytes()
        {
            return Err(AmmError::InvalidCoinMint.into());
        }
        if identity(market_state.pc_mint) != amm_pc_vault.mint.to_aligned_bytes()
            || identity(market_state.pc_mint) != user_dest_pc.mint.to_aligned_bytes()
        {
            return Err(AmmError::InvalidPCMint.into());
        }

        Calculator::calc_total_without_take_pnl(
            amm_pc_vault.amount,
            amm_coin_vault.amount,
            &open_orders,
            &amm,
            &market_state,
            &market_event_q_info,
            &amm_open_orders_info,
        )?
    } else {
        Calculator::calc_total_without_take_pnl_no_orderbook(
            amm_pc_vault.amount,
            amm_coin_vault.amount,
            &amm,
        )?
    };

    let x1 = Calculator::normalize_decimal_v2(
        total_pc_without_take_pnl,
        amm.pc_decimals,
        amm.sys_decimal_value,
    );
    let y1 = Calculator::normalize_decimal_v2(
        total_coin_without_take_pnl,
        amm.coin_decimals,
        amm.sys_decimal_value,
    );

    // Calculate and update PnL
    let mut delta_x: u128 = 0;
    let mut delta_y: u128 = 0;
    if amm.status != AmmStatus::WithdrawOnly.into_u64() {
        (delta_x, delta_y) = calc_take_pnl(
            &target_orders,
            &mut amm,
            &mut total_pc_without_take_pnl,
            &mut total_coin_without_take_pnl,
            x1.as_u128().into(),
            y1.as_u128().into(),
        )?;
    }

    // Calculate withdrawal amounts
    let invariant = InvariantPool {
        token_input: withdraw.amount,
        token_total: amm.lp_amount,
    };
    let coin_amount = invariant
        .exchange_pool_to_token(total_coin_without_take_pnl, RoundDirection::Floor)
        .ok_or(AmmError::CalculationExRateFailure)?;
    let pc_amount = invariant
        .exchange_pool_to_token(total_pc_without_take_pnl, RoundDirection::Floor)
        .ok_or(AmmError::CalculationExRateFailure)?;

    encode_ray_log(WithdrawLog {
        log_type: LogType::Withdraw.into_u8(),
        withdraw_lp: withdraw.amount,
        user_lp: user_source_lp.amount,
        pool_coin: total_coin_without_take_pnl,
        pool_pc: total_pc_without_take_pnl,
        pool_lp: amm.lp_amount,
        calc_pnl_x: target_orders.calc_pnl_x,
        calc_pnl_y: target_orders.calc_pnl_y,
        out_coin: coin_amount,
        out_pc: pc_amount,
    });

    if withdraw.amount == 0 || coin_amount == 0 || pc_amount == 0 {
        return Err(AmmError::InvalidInput.into());
    }

    if coin_amount < amm_coin_vault.amount && pc_amount < amm_pc_vault.amount {
        if withdraw.min_coin_amount.is_some() && withdraw.min_pc_amount.is_some() {
            if withdraw.min_coin_amount.unwrap() > coin_amount
                || withdraw.min_pc_amount.unwrap() > pc_amount
            {
                return Err(AmmError::ExceededSlippage.into());
            }
        }

        // Transfer tokens from AMM vaults to user
        // For Token-2022 support, we need to pass the mint accounts
        Invokers::token_transfer_with_authority(
            token_program_info.clone(),
            amm_coin_vault_info.clone(),
            user_dest_coin_info.clone(),
            amm_authority_info.clone(),
            AUTHORITY_AMM,
            amm.nonce as u8,
            coin_amount,
            amm_coin_mint_info.clone(),
            &[],
        )?;

        Invokers::token_transfer_with_authority(
            token_program_info.clone(),
            amm_pc_vault_info.clone(),
            user_dest_pc_info.clone(),
            amm_authority_info.clone(),
            AUTHORITY_AMM,
            amm.nonce as u8,
            pc_amount,
            amm_pc_mint_info.clone(),
            &[],
        )?;

        // Burn LP tokens from user
        Invokers::token_burn(
            token_program_info.clone(),
            user_source_lp_info.clone(),
            amm_lp_mint_info.clone(),
            source_lp_owner_info.clone(),
            withdraw.amount,
        )?;

        amm.lp_amount = amm.lp_amount.checked_sub(withdraw.amount).unwrap();
    } else {
        return Err(AmmError::TakePnlError.into());
    }

    // Update target orders PnL calculations
    target_orders.calc_pnl_x = x1
        .checked_sub(Calculator::normalize_decimal_v2(
            pc_amount,
            amm.pc_decimals,
            amm.sys_decimal_value,
        ))
        .unwrap()
        .checked_sub(U128::from(delta_x))
        .unwrap()
        .as_u128();

    target_orders.calc_pnl_y = y1
        .checked_sub(Calculator::normalize_decimal_v2(
            coin_amount,
            amm.coin_decimals,
            amm.sys_decimal_value,
        ))
        .unwrap()
        .checked_sub(U128::from(delta_y))
        .unwrap()
        .as_u128();

    amm.recent_epoch = Clock::get()?.epoch;
    Ok(())
} 