//! Handles the deposit instruction logic for Raydium AMM
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    sysvar::Sysvar,
};
use crate::{
    error::AmmError,
    instruction::DepositInstruction,
    invokers::Invokers,
    math::{Calculator, RoundDirection, InvariantToken, InvariantPool, U128},
    state::{AmmInfo, AmmStatus, TargetOrders},
};
use crate::process::constants::AUTHORITY_AMM;
use crate::process::helpers::{encode_ray_log, identity, authority_id, unpack_token_account, load_serum_market_order, calc_take_pnl};
use crate::process::args::{DepositLog, LogType};
use serum_dex::state::ToAlignedBytes;
use crate::log::log_keys_mismatch;

/// The number of accounts expected for a deposit instruction.
/// This is based on the order of next_account_info calls in the function:
/// [token_program_info, amm_info, amm_authority_info, amm_open_orders_info, amm_target_orders_info, amm_lp_mint_info, amm_coin_vault_info, amm_pc_vault_info, market_info, user_source_coin_info, user_source_pc_info, user_dest_lp_info, source_owner_info, market_event_queue_info, coin_mint_info, pc_mint_info]
/// = 16 accounts
const ACCOUNT_LEN: usize = 16;

pub fn process_deposit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    deposit: DepositInstruction,
) -> ProgramResult {
    let input_account_len = accounts.len();
    if input_account_len != ACCOUNT_LEN && input_account_len != ACCOUNT_LEN + 1 {
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
    let market_info = next_account_info(account_info_iter)?;
    let user_source_coin_info = next_account_info(account_info_iter)?;
    let user_source_pc_info = next_account_info(account_info_iter)?;
    let user_dest_lp_info = next_account_info(account_info_iter)?;
    let source_owner_info = next_account_info(account_info_iter)?;
    let market_event_queue_info = next_account_info(account_info_iter)?;
    let coin_mint_info = next_account_info(account_info_iter)?;
    let pc_mint_info = next_account_info(account_info_iter)?;
    let mut amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;
    if deposit.max_coin_amount == 0 || deposit.max_pc_amount == 0 {
        encode_ray_log(DepositLog {
            log_type: LogType::Deposit.into_u8(),
            max_coin: deposit.max_coin_amount,
            max_pc: deposit.max_pc_amount,
            base: deposit.base_side,
            pool_coin: 0,
            pool_pc: 0,
            pool_lp: 0,
            calc_pnl_x: 0,
            calc_pnl_y: 0,
            deduct_coin: 0,
            deduct_pc: 0,
            mint_lp: 0,
        });
        return Err(AmmError::InvalidInput.into());
    }
    if !source_owner_info.is_signer {
        return Err(AmmError::InvalidSignAccount.into());
    }
    if !AmmStatus::from_u64(amm.status).deposit_permission() {
        return Err(AmmError::InvalidStatus.into());
    }
    if *amm_authority_info.key != authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)? {
        return Err(AmmError::InvalidProgramAddress.into());
    }
    let enable_orderbook = AmmStatus::from_u64(amm.status).orderbook_permission();
    let spl_token_program_id = token_program_info.key;
    if *amm_coin_vault_info.key != amm.coin_vault || *user_source_coin_info.key == amm.coin_vault {
        return Err(AmmError::InvalidCoinVault.into());
    }
    if *amm_pc_vault_info.key != amm.pc_vault || *user_source_pc_info.key == amm.pc_vault {
        return Err(AmmError::InvalidPCVault.into());
    }
    check_assert_eq!(
        *amm_lp_mint_info.key,
        amm.lp_mint,
        "lp_mint",
        AmmError::InvalidPoolMint
    );
    check_assert_eq!(
        *amm_target_orders_info.key,
        amm.target_orders,
        "target_orders",
        AmmError::InvalidTargetOrders
    );
    let amm_coin_vault = unpack_token_account(&amm_coin_vault_info, spl_token_program_id)?;
    let amm_pc_vault = unpack_token_account(&amm_pc_vault_info, spl_token_program_id)?;
    let user_source_coin = unpack_token_account(&user_source_coin_info, spl_token_program_id)?;
    let user_source_pc = unpack_token_account(&user_source_pc_info, spl_token_program_id)?;
    let mut target_orders = TargetOrders::load_mut_checked(&amm_target_orders_info, program_id, amm_info.key)?;
    let (mut total_pc_without_take_pnl, mut total_coin_without_take_pnl) = if enable_orderbook {
        check_assert_eq!(
            *market_info.key,
            amm.market,
            "market",
            AmmError::InvalidMarket
        );
        check_assert_eq!(
            *amm_open_orders_info.key,
            amm.open_orders,
            "open_orders",
            AmmError::InvalidOpenOrders
        );
        let (market_state, open_orders) = load_serum_market_order(
            market_info,
            amm_open_orders_info,
            amm_authority_info,
            &amm,
            false,
        )?;
        if identity(market_state.coin_mint) != amm_coin_vault.mint.to_aligned_bytes()
            || identity(market_state.coin_mint) != user_source_coin.mint.to_aligned_bytes()
        {
            return Err(AmmError::InvalidCoinMint.into());
        }
        if identity(market_state.pc_mint) != amm_pc_vault.mint.to_aligned_bytes()
            || identity(market_state.pc_mint) != user_source_pc.mint.to_aligned_bytes()
        {
            return Err(AmmError::InvalidPCMint.into());
        }
        Calculator::calc_total_without_take_pnl(
            amm_pc_vault.amount,
            amm_coin_vault.amount,
            &open_orders,
            &amm,
            &market_state,
            &market_event_queue_info,
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
    let (delta_x, delta_y) = calc_take_pnl(
        &target_orders,
        &mut amm,
        &mut total_pc_without_take_pnl,
        &mut total_coin_without_take_pnl,
        x1.as_u128().into(),
        y1.as_u128().into(),
    )?;
    let invariant = InvariantToken {
        token_coin: total_coin_without_take_pnl,
        token_pc: total_pc_without_take_pnl,
    };
    if amm.lp_amount == 0 {
        encode_ray_log(DepositLog {
            log_type: LogType::Deposit.into_u8(),
            max_coin: deposit.max_coin_amount,
            max_pc: deposit.max_pc_amount,
            base: deposit.base_side,
            pool_coin: total_coin_without_take_pnl,
            pool_pc: total_pc_without_take_pnl,
            pool_lp: amm.lp_amount,
            calc_pnl_x: target_orders.calc_pnl_x,
            calc_pnl_y: target_orders.calc_pnl_y,
            deduct_coin: 0,
            deduct_pc: 0,
            mint_lp: 0,
        });
        return Err(AmmError::NotAllowZeroLP.into());
    }
    let (deduct_pc_amount, deduct_coin_amount, mint_lp_amount) = if deposit.base_side == 0 {
        let deduct_pc_amount = invariant
            .exchange_coin_to_pc(deposit.max_coin_amount, RoundDirection::Ceiling)
            .ok_or(AmmError::CalculationExRateFailure)?;
        let deduct_coin_amount = deposit.max_coin_amount;
        if deduct_pc_amount > deposit.max_pc_amount {
            encode_ray_log(DepositLog {
                log_type: LogType::Deposit.into_u8(),
                max_coin: deposit.max_coin_amount,
                max_pc: deposit.max_pc_amount,
                base: deposit.base_side,
                pool_coin: total_coin_without_take_pnl,
                pool_pc: total_pc_without_take_pnl,
                pool_lp: amm.lp_amount,
                calc_pnl_x: target_orders.calc_pnl_x,
                calc_pnl_y: target_orders.calc_pnl_y,
                deduct_coin: deduct_coin_amount,
                deduct_pc: deduct_pc_amount,
                mint_lp: 0,
            });
            return Err(AmmError::ExceededSlippage.into());
        }
        if let Some(other_min) = deposit.other_amount_min {
            if deduct_pc_amount < other_min {
                encode_ray_log(DepositLog {
                    log_type: LogType::Deposit.into_u8(),
                    max_coin: deposit.max_coin_amount,
                    max_pc: deposit.max_pc_amount,
                    base: deposit.base_side,
                    pool_coin: total_coin_without_take_pnl,
                    pool_pc: total_pc_without_take_pnl,
                    pool_lp: amm.lp_amount,
                    calc_pnl_x: target_orders.calc_pnl_x,
                    calc_pnl_y: target_orders.calc_pnl_y,
                    deduct_coin: deduct_coin_amount,
                    deduct_pc: deduct_pc_amount,
                    mint_lp: 0,
                });
                return Err(AmmError::ExceededSlippage.into());
            }
        }
        let invariant_coin = InvariantPool {
            token_input: deduct_coin_amount,
            token_total: total_coin_without_take_pnl,
        };
        let mint_lp_amount = invariant_coin
            .exchange_token_to_pool(amm.lp_amount, RoundDirection::Floor)
            .ok_or(AmmError::CalculationExRateFailure)?;
        (deduct_pc_amount, deduct_coin_amount, mint_lp_amount)
    } else {
        let deduct_coin_amount = invariant
            .exchange_pc_to_coin(deposit.max_pc_amount, RoundDirection::Ceiling)
            .ok_or(AmmError::CalculationExRateFailure)?;
        let deduct_pc_amount = deposit.max_pc_amount;
        if deduct_coin_amount > deposit.max_coin_amount {
            encode_ray_log(DepositLog {
                log_type: LogType::Deposit.into_u8(),
                max_coin: deposit.max_coin_amount,
                max_pc: deposit.max_pc_amount,
                base: deposit.base_side,
                pool_coin: total_coin_without_take_pnl,
                pool_pc: total_pc_without_take_pnl,
                pool_lp: amm.lp_amount,
                calc_pnl_x: target_orders.calc_pnl_x,
                calc_pnl_y: target_orders.calc_pnl_y,
                deduct_coin: deduct_coin_amount,
                deduct_pc: deduct_pc_amount,
                mint_lp: 0,
            });
            return Err(AmmError::ExceededSlippage.into());
        }
        if let Some(other_min) = deposit.other_amount_min {
            if deduct_coin_amount < other_min {
                encode_ray_log(DepositLog {
                    log_type: LogType::Deposit.into_u8(),
                    max_coin: deposit.max_coin_amount,
                    max_pc: deposit.max_pc_amount,
                    base: deposit.base_side,
                    pool_coin: total_coin_without_take_pnl,
                    pool_pc: total_pc_without_take_pnl,
                    pool_lp: amm.lp_amount,
                    calc_pnl_x: target_orders.calc_pnl_x,
                    calc_pnl_y: target_orders.calc_pnl_y,
                    deduct_coin: deduct_coin_amount,
                    deduct_pc: deduct_pc_amount,
                    mint_lp: 0,
                });
                return Err(AmmError::ExceededSlippage.into());
            }
        }
        let invariant_pc = InvariantPool {
            token_input: deduct_pc_amount,
            token_total: total_pc_without_take_pnl,
        };
        let mint_lp_amount = invariant_pc
            .exchange_token_to_pool(amm.lp_amount, RoundDirection::Floor)
            .ok_or(AmmError::CalculationExRateFailure)?;
        (deduct_pc_amount, deduct_coin_amount, mint_lp_amount)
    };
    encode_ray_log(DepositLog {
        log_type: LogType::Deposit.into_u8(),
        max_coin: deposit.max_coin_amount,
        max_pc: deposit.max_pc_amount,
        base: deposit.base_side,
        pool_coin: total_coin_without_take_pnl,
        pool_pc: total_pc_without_take_pnl,
        pool_lp: amm.lp_amount,
        calc_pnl_x: target_orders.calc_pnl_x,
        calc_pnl_y: target_orders.calc_pnl_y,
        deduct_coin: deduct_coin_amount,
        deduct_pc: deduct_pc_amount,
        mint_lp: mint_lp_amount,
    });
    if deduct_coin_amount > user_source_coin.amount || deduct_pc_amount > user_source_pc.amount {
        return Err(AmmError::InsufficientFunds.into());
    }
    if mint_lp_amount == 0 || deduct_coin_amount == 0 || deduct_pc_amount == 0 {
        return Err(AmmError::InvalidInput.into());
    }
    // Get mint account infos for the transfers
    
    Invokers::token_transfer(
        token_program_info.clone(),
        user_source_coin_info.clone(),
        amm_coin_vault_info.clone(),
        source_owner_info.clone(),
        deduct_coin_amount,
        coin_mint_info.clone(),
        &[],
    )?;
    Invokers::token_transfer(
        token_program_info.clone(),
        user_source_pc_info.clone(),
        amm_pc_vault_info.clone(),
        source_owner_info.clone(),
        deduct_pc_amount,
        pc_mint_info.clone(),
        &[],
    )?;
    Invokers::token_mint_to(
        token_program_info.clone(),
        amm_lp_mint_info.clone(),
        user_dest_lp_info.clone(),
        amm_authority_info.clone(),
        AUTHORITY_AMM,
        amm.nonce as u8,
        mint_lp_amount,
    )?;
    amm.lp_amount = amm.lp_amount.checked_add(mint_lp_amount).unwrap();
    target_orders.calc_pnl_x = x1
        .checked_add(Calculator::normalize_decimal_v2(
            deduct_pc_amount,
            amm.pc_decimals,
            amm.sys_decimal_value,
        ))
        .unwrap()
        .checked_sub(U128::from(delta_x))
        .unwrap()
        .as_u128();
    target_orders.calc_pnl_y = y1
        .checked_add(Calculator::normalize_decimal_v2(
            deduct_coin_amount,
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