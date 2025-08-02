//! Handles the swap instruction logic for Raydium AMM
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    sysvar::Sysvar,
    msg,
};
use crate::{
    error::AmmError,
    instruction::{SwapInstructionBaseIn, SwapInstructionBaseOut},
    invokers::Invokers,
    math::{Calculator, SwapDirection, U128, CheckedCeilDiv},
    state::{AmmInfo, AmmStatus},
};
use crate::process::constants::AUTHORITY_AMM;
use crate::process::helpers::{identity, authority_id, unpack_token_account, load_serum_market_order, get_amm_orders};
use crate::process::args::{SwapBaseInLog, SwapBaseOutLog, LogType};
use crate::log::{log_keys_mismatch, encode_ray_log};
use crate::check_assert_eq;
use serum_dex::critbit::LeafNode;

/// The number of accounts expected for a swap instruction.
/// This is based on the order of next_account_info calls in the function:
/// [token_program_info, amm_info, amm_authority_info, amm_open_orders_info, amm_coin_vault_info, amm_pc_vault_info, amm_coin_mint_info, amm_pc_mint_info, market_program_info, market_info, market_bids_info, market_asks_info, market_event_queue_info, market_coin_vault_info, market_pc_vault_info, market_vault_signer, user_source_info, user_destination_info, user_source_owner]
/// = 19 accounts (base) + optional amm_target_orders_info
const ACCOUNT_LEN: usize = 19;

pub fn process_swap_base_in(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    swap: SwapInstructionBaseIn,
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
    if input_account_len == ACCOUNT_LEN + 1 {
        let _amm_target_orders_info = next_account_info(account_info_iter)?;
    }
    let amm_coin_vault_info = next_account_info(account_info_iter)?;
    let amm_pc_vault_info = next_account_info(account_info_iter)?;
    let amm_coin_mint_info = next_account_info(account_info_iter)?;
    let amm_pc_mint_info = next_account_info(account_info_iter)?;

    let market_program_info = next_account_info(account_info_iter)?;

    let mut amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;
    let enable_orderbook;
    if AmmStatus::from_u64(amm.status).orderbook_permission() {
        enable_orderbook = true;
    } else {
        enable_orderbook = false;
    }
    let market_info = next_account_info(account_info_iter)?;
    let market_bids_info = next_account_info(account_info_iter)?;
    let market_asks_info = next_account_info(account_info_iter)?;
    let market_event_queue_info = next_account_info(account_info_iter)?;
    let market_coin_vault_info = next_account_info(account_info_iter)?;
    let market_pc_vault_info = next_account_info(account_info_iter)?;
    let market_vault_signer = next_account_info(account_info_iter)?;

    let user_source_info = next_account_info(account_info_iter)?;
    let user_destination_info = next_account_info(account_info_iter)?;
    let user_source_owner = next_account_info(account_info_iter)?;
    if !user_source_owner.is_signer {
        return Err(AmmError::InvalidSignAccount.into());
    }
    check_assert_eq!(
        *token_program_info.key,
        spl_token::id(),
        "spl_token_program",
        AmmError::InvalidSplTokenProgram
    );
    let spl_token_program_id = token_program_info.key;
    if *amm_authority_info.key
        != authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?
    {
        return Err(AmmError::InvalidProgramAddress.into());
    }
    check_assert_eq!(
        *amm_coin_vault_info.key,
        amm.coin_vault,
        "coin_vault",
        AmmError::InvalidCoinVault
    );
    check_assert_eq!(
        *amm_pc_vault_info.key,
        amm.pc_vault,
        "pc_vault",
        AmmError::InvalidPCVault
    );

    if *user_source_info.key == amm.pc_vault || *user_source_info.key == amm.coin_vault {
        return Err(AmmError::InvalidUserToken.into());
    }
    if *user_destination_info.key == amm.pc_vault
        || *user_destination_info.key == amm.coin_vault
    {
        return Err(AmmError::InvalidUserToken.into());
    }

    let amm_coin_vault =
        unpack_token_account(&amm_coin_vault_info, spl_token_program_id)?;
    let amm_pc_vault = unpack_token_account(&amm_pc_vault_info, spl_token_program_id)?;

    let user_source = unpack_token_account(&user_source_info, spl_token_program_id)?;
    let user_destination =
        unpack_token_account(&user_destination_info, spl_token_program_id)?;

    if !AmmStatus::from_u64(amm.status).swap_permission() {
        msg!(&format!("swap_base_in: status {}", identity(amm.status)));
        let clock = Clock::get()?;
        if amm.status == AmmStatus::OrderBookOnly.into_u64()
            && (clock.unix_timestamp as u64) >= amm.state_data.orderbook_to_init_time
        {
            amm.status = AmmStatus::Initialized.into_u64();
            msg!("swap_base_in: OrderBook to Initialized");
        } else {
            return Err(AmmError::InvalidStatus.into());
        }
    } else if amm.status == AmmStatus::WaitingTrade.into_u64() {
        let clock = Clock::get()?;
        if (clock.unix_timestamp as u64) < amm.state_data.pool_open_time {
            return Err(AmmError::InvalidStatus.into());
        } else {
            amm.status = AmmStatus::SwapOnly.into_u64();
            msg!("swap_base_in: WaitingTrade to SwapOnly");
        }
    }

    let total_pc_without_take_pnl;
    let total_coin_without_take_pnl;
    let mut bids: Vec<LeafNode> = Vec::new();
    let mut asks: Vec<LeafNode> = Vec::new();
    if enable_orderbook {
        check_assert_eq!(
            *amm_open_orders_info.key,
            amm.open_orders,
            "open_orders",
            AmmError::InvalidOpenOrders
        );
        check_assert_eq!(
            *market_program_info.key,
            amm.market_program,
            "market_program",
            AmmError::InvalidMarketProgram
        );
        check_assert_eq!(
            *market_info.key,
            amm.market,
            "market",
            AmmError::InvalidMarket
        );
        let (market_state, open_orders) = load_serum_market_order(
            market_info,
            amm_open_orders_info,
            amm_authority_info,
            &amm,
            false,
        )?;
        let bids_orders = market_state.load_bids_checked(&market_bids_info)?;
        let asks_orders = market_state.load_asks_checked(&market_asks_info)?;
        (bids, asks) = get_amm_orders(&open_orders, bids_orders, asks_orders)?;
        (total_pc_without_take_pnl, total_coin_without_take_pnl) =
            Calculator::calc_total_without_take_pnl(
                amm_pc_vault.amount,
                amm_coin_vault.amount,
                &open_orders,
                &amm,
                &market_state,
                &market_event_queue_info,
                &amm_open_orders_info,
            )?;
    } else {
        (total_pc_without_take_pnl, total_coin_without_take_pnl) =
            Calculator::calc_total_without_take_pnl_no_orderbook(
                amm_pc_vault.amount,
                amm_coin_vault.amount,
                &amm,
            )?;
    }

    let swap_direction;
    if user_source.mint == amm_coin_vault.mint && user_destination.mint == amm_pc_vault.mint {
        swap_direction = SwapDirection::Coin2PC
    } else if user_source.mint == amm_pc_vault.mint
        && user_destination.mint == amm_coin_vault.mint
    {
        swap_direction = SwapDirection::PC2Coin
    } else {
        return Err(AmmError::InvalidUserToken.into());
    }
    if user_source.amount < swap.amount_in {
        encode_ray_log(SwapBaseInLog {
            log_type: LogType::SwapBaseIn.into_u8(),
            amount_in: swap.amount_in,
            minimum_out: swap.minimum_amount_out,
            direction: swap_direction as u64,
            user_source: user_source.amount,
            pool_coin: total_coin_without_take_pnl,
            pool_pc: total_pc_without_take_pnl,
            out_amount: 0,
        });
        return Err(AmmError::InsufficientFunds.into());
    }
    let swap_fee = U128::from(swap.amount_in)
        .checked_mul(amm.fees.swap_fee_numerator.into())
        .unwrap()
        .checked_ceil_div(amm.fees.swap_fee_denominator.into())
        .unwrap()
        .0;
    let swap_in_after_deduct_fee = U128::from(swap.amount_in).checked_sub(swap_fee).unwrap();
    let swap_amount_out = Calculator::swap_token_amount_base_in(
        swap_in_after_deduct_fee,
        total_pc_without_take_pnl.into(),
        total_coin_without_take_pnl.into(),
        swap_direction,
    )
    .as_u64();
    encode_ray_log(SwapBaseInLog {
        log_type: LogType::SwapBaseIn.into_u8(),
        amount_in: swap.amount_in,
        minimum_out: swap.minimum_amount_out,
        direction: swap_direction as u64,
        user_source: user_source.amount,
        pool_coin: total_coin_without_take_pnl,
        pool_pc: total_pc_without_take_pnl,
        out_amount: swap_amount_out,
    });
    if swap_amount_out < swap.minimum_amount_out {
        return Err(AmmError::ExceededSlippage.into());
    }
    if swap_amount_out == 0 || swap.amount_in == 0 {
        return Err(AmmError::InvalidInput.into());
    }

    match swap_direction {
        SwapDirection::Coin2PC => {
            if swap_amount_out >= total_pc_without_take_pnl {
                return Err(AmmError::InsufficientFunds.into());
            }

            if enable_orderbook {
                // coin -> pc, need cancel buy order
                if !bids.is_empty() {
                    let mut amm_order_ids_vec = Vec::new();
                    let mut order_ids = [0u64; 8];
                    let mut count = 0;
                    // fetch cancel order ids{
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

                if swap_amount_out > amm_pc_vault.amount {
                    // need settle funds
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
                        Some(&amm_pc_vault_info.clone()),
                        AUTHORITY_AMM,
                        amm.nonce as u8,
                    )?;
                }
            }
            // deposit source coin to amm_coin_vault
            Invokers::token_transfer(
                program_id,
                token_program_info.clone(),
                user_source_info.clone(),
                amm_coin_vault_info.clone(),
                user_source_owner.clone(),
                swap.amount_in,
                amm_coin_mint_info.clone(),
                &[],
            )?;
            // withdraw amm_pc_vault to destination pc
            Invokers::token_transfer_with_authority(
                program_id,
                token_program_info.clone(),
                amm_pc_vault_info.clone(),
                user_destination_info.clone(),
                amm_authority_info.clone(),
                AUTHORITY_AMM,
                amm.nonce as u8,
                swap_amount_out,
                amm_pc_mint_info.clone(),
                &[],
            )?;
            // update state_data data
            amm.state_data.swap_coin_in_amount = amm
                .state_data
                .swap_coin_in_amount
                .checked_add(swap.amount_in.into())
                .unwrap();
            amm.state_data.swap_pc_out_amount = amm
                .state_data
                .swap_pc_out_amount
                .checked_add(swap_amount_out.into())
                .unwrap();
            // charge coin as swap fee
            amm.state_data.swap_acc_coin_fee = amm
                .state_data
                .swap_acc_coin_fee
                .checked_add(swap_fee.as_u64())
                .unwrap();
        }
        SwapDirection::PC2Coin => {
            if swap_amount_out >= total_coin_without_take_pnl {
                return Err(AmmError::InsufficientFunds.into());
            }

            if enable_orderbook {
                // pc -> coin, need cancel sell order
                if !asks.is_empty() {
                    let mut amm_order_ids_vec = Vec::new();
                    let mut order_ids = [0u64; 8];
                    let mut count = 0;
                    // fetch cancel order ids{
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

                if swap_amount_out > amm_coin_vault.amount {
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
                        Some(&amm_pc_vault_info.clone()),
                        AUTHORITY_AMM,
                        amm.nonce as u8,
                    )?;
                }
            }
            // deposit source pc to amm_pc_vault
            Invokers::token_transfer(
                program_id,
                token_program_info.clone(),
                user_source_info.clone(),
                amm_pc_vault_info.clone(),
                user_source_owner.clone(),
                swap_amount_out,
                amm_pc_mint_info.clone(),
                &[],
            )?;
            // withdraw amm_coin_vault to destination coin
            Invokers::token_transfer_with_authority(
                program_id,
                token_program_info.clone(),
                amm_coin_vault_info.clone(),
                user_destination_info.clone(),
                amm_authority_info.clone(),
                AUTHORITY_AMM,
                amm.nonce as u8,
                swap_amount_out,
                amm_coin_mint_info.clone(),
                &[],
            )?;
            // update state_data data
            amm.state_data.swap_pc_in_amount = amm
                .state_data
                .swap_pc_in_amount
                .checked_add(swap.amount_in.into())
                .unwrap();
            amm.state_data.swap_coin_out_amount = amm
                .state_data
                .swap_coin_out_amount
                .checked_add(swap_amount_out.into())
                .unwrap();
            // charge pc as swap fee
            amm.state_data.swap_acc_pc_fee = amm
                .state_data
                .swap_acc_pc_fee
                .checked_add(swap_fee.as_u64())
                .unwrap();
        }
    };
    amm.recent_epoch = Clock::get()?.epoch;

    Ok(())
}

pub fn process_swap_base_out(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    swap: SwapInstructionBaseOut,
) -> ProgramResult {
    const SWAP_ACCOUNT_NUM: usize = 19;
    let input_account_len = accounts.len();
    if input_account_len != SWAP_ACCOUNT_NUM && input_account_len != SWAP_ACCOUNT_NUM + 1 {
        return Err(AmmError::WrongAccountsNumber.into());
    }
    let account_info_iter = &mut accounts.iter();
    let token_program_info = next_account_info(account_info_iter)?;

    let amm_info = next_account_info(account_info_iter)?;
    let amm_authority_info = next_account_info(account_info_iter)?;
    let amm_open_orders_info = next_account_info(account_info_iter)?;
    if input_account_len == SWAP_ACCOUNT_NUM + 1 {
        let _amm_target_orders_info = next_account_info(account_info_iter)?;
    }
    let amm_coin_vault_info = next_account_info(account_info_iter)?;
    let amm_pc_vault_info = next_account_info(account_info_iter)?;
    let amm_coin_mint_info = next_account_info(account_info_iter)?;
    let amm_pc_mint_info = next_account_info(account_info_iter)?;

    let market_program_info = next_account_info(account_info_iter)?;

    let mut amm = AmmInfo::load_mut_checked(&amm_info, program_id)?;
    let enable_orderbook;
    if AmmStatus::from_u64(amm.status).orderbook_permission() {
        enable_orderbook = true;
    } else {
        enable_orderbook = false;
    }

    let market_info = next_account_info(account_info_iter)?;
    let market_bids_info = next_account_info(account_info_iter)?;
    let market_asks_info = next_account_info(account_info_iter)?;
    let market_event_queue_info = next_account_info(account_info_iter)?;
    let market_coin_vault_info = next_account_info(account_info_iter)?;
    let market_pc_vault_info = next_account_info(account_info_iter)?;
    let market_vault_signer = next_account_info(account_info_iter)?;

    let user_source_info = next_account_info(account_info_iter)?;
    let user_destination_info = next_account_info(account_info_iter)?;
    let user_source_owner = next_account_info(account_info_iter)?;
    if !user_source_owner.is_signer {
        return Err(AmmError::InvalidSignAccount.into());
    }

    check_assert_eq!(
        *token_program_info.key,
        spl_token::id(),
        "spl_token_program",
        AmmError::InvalidSplTokenProgram
    );
    let spl_token_program_id = token_program_info.key;
    let authority = authority_id(program_id, AUTHORITY_AMM, amm.nonce as u8)?;
    check_assert_eq!(
        *amm_authority_info.key,
        authority,
        "authority",
        AmmError::InvalidProgramAddress
    );
    check_assert_eq!(
        *amm_coin_vault_info.key,
        amm.coin_vault,
        "coin_vault",
        AmmError::InvalidCoinVault
    );
    check_assert_eq!(
        *amm_pc_vault_info.key,
        amm.pc_vault,
        "pc_vault",
        AmmError::InvalidPCVault
    );

    if *user_source_info.key == amm.pc_vault || *user_source_info.key == amm.coin_vault {
        return Err(AmmError::InvalidUserToken.into());
    }
    if *user_destination_info.key == amm.pc_vault
        || *user_destination_info.key == amm.coin_vault
    {
        return Err(AmmError::InvalidUserToken.into());
    }

    let amm_coin_vault =
        unpack_token_account(&amm_coin_vault_info, spl_token_program_id)?;
    let amm_pc_vault = unpack_token_account(&amm_pc_vault_info, spl_token_program_id)?;

    let user_source = unpack_token_account(&user_source_info, spl_token_program_id)?;
    let user_destination =
        unpack_token_account(&user_destination_info, spl_token_program_id)?;

    if !AmmStatus::from_u64(amm.status).swap_permission() {
        msg!(&format!("swap_base_out: status {}", identity(amm.status)));
        let clock = Clock::get()?;
        if amm.status == AmmStatus::OrderBookOnly.into_u64()
            && (clock.unix_timestamp as u64) >= amm.state_data.orderbook_to_init_time
        {
            amm.status = AmmStatus::Initialized.into_u64();
            msg!("swap_base_out: OrderBook to Initialized");
        } else {
            return Err(AmmError::InvalidStatus.into());
        }
    } else if amm.status == AmmStatus::WaitingTrade.into_u64() {
        let clock = Clock::get()?;
        if (clock.unix_timestamp as u64) < amm.state_data.pool_open_time {
            return Err(AmmError::InvalidStatus.into());
        } else {
            amm.status = AmmStatus::SwapOnly.into_u64();
            msg!("swap_base_out: WaitingTrade to SwapOnly");
        }
    }

    let total_pc_without_take_pnl;
    let total_coin_without_take_pnl;
    let mut bids: Vec<LeafNode> = Vec::new();
    let mut asks: Vec<LeafNode> = Vec::new();
    if enable_orderbook {
        check_assert_eq!(
            *amm_open_orders_info.key,
            amm.open_orders,
            "open_orders",
            AmmError::InvalidOpenOrders
        );
        check_assert_eq!(
            *market_program_info.key,
            amm.market_program,
            "market_program",
            AmmError::InvalidMarketProgram
        );
        check_assert_eq!(
            *market_info.key,
            amm.market,
            "market",
            AmmError::InvalidMarket
        );
        let (market_state, open_orders) = load_serum_market_order(
            market_info,
            amm_open_orders_info,
            amm_authority_info,
            &amm,
            false,
        )?;
        let bids_orders = market_state.load_bids_checked(&market_bids_info)?;
        let asks_orders = market_state.load_asks_checked(&market_asks_info)?;
        (bids, asks) = get_amm_orders(&open_orders, bids_orders, asks_orders)?;
        (total_pc_without_take_pnl, total_coin_without_take_pnl) =
            Calculator::calc_total_without_take_pnl(
                amm_pc_vault.amount,
                amm_coin_vault.amount,
                &open_orders,
                &amm,
                &market_state,
                &market_event_queue_info,
                &amm_open_orders_info,
            )?;
    } else {
        (total_pc_without_take_pnl, total_coin_without_take_pnl) =
            Calculator::calc_total_without_take_pnl_no_orderbook(
                amm_pc_vault.amount,
                amm_coin_vault.amount,
                &amm,
            )?;
    }

    let swap_direction;
    if user_source.mint == amm_coin_vault.mint && user_destination.mint == amm_pc_vault.mint {
        swap_direction = SwapDirection::Coin2PC
    } else if user_source.mint == amm_pc_vault.mint
        && user_destination.mint == amm_coin_vault.mint
    {
        swap_direction = SwapDirection::PC2Coin
    } else {
        return Err(AmmError::InvalidUserToken.into());
    }

    let swap_in_before_add_fee = Calculator::swap_token_amount_base_out(
        swap.amount_out.into(),
        total_pc_without_take_pnl.into(),
        total_coin_without_take_pnl.into(),
        swap_direction,
    );
    // swap_in_after_add_fee * (1 - 0.0025) = swap_in_before_add_fee
    // swap_in_after_add_fee = swap_in_before_add_fee / (1 - 0.0025)
    let swap_in_after_add_fee = swap_in_before_add_fee
        .checked_mul(amm.fees.swap_fee_denominator.into())
        .unwrap()
        .checked_ceil_div(
            (amm.fees
                .swap_fee_denominator
                .checked_sub(amm.fees.swap_fee_numerator)
                .unwrap())
            .into(),
        )
        .unwrap()
        .0
        .as_u64();
    let swap_fee = swap_in_after_add_fee
        .checked_sub(swap_in_before_add_fee.as_u64())
        .unwrap();
    encode_ray_log(SwapBaseOutLog {
        log_type: LogType::SwapBaseOut.into_u8(),
        max_in: swap.max_amount_in,
        amount_out: swap.amount_out,
        direction: swap_direction as u64,
        user_source: user_source.amount,
        pool_coin: total_coin_without_take_pnl,
        pool_pc: total_pc_without_take_pnl,
        deduct_in: swap_in_after_add_fee,
    });
    if user_source.amount < swap_in_after_add_fee {
        return Err(AmmError::InsufficientFunds.into());
    }
    if swap.max_amount_in < swap_in_after_add_fee {
        return Err(AmmError::ExceededSlippage.into());
    }
    if swap_in_after_add_fee == 0 || swap.amount_out == 0 {
        return Err(AmmError::InvalidInput.into());
    }

    match swap_direction {
        SwapDirection::Coin2PC => {
            if swap.amount_out >= total_pc_without_take_pnl {
                return Err(AmmError::InsufficientFunds.into());
            }

            if enable_orderbook {
                // coin -> pc, need cancel buy order
                if !bids.is_empty() {
                    let mut amm_order_ids_vec = Vec::new();
                    let mut order_ids = [0u64; 8];
                    let mut count = 0;
                    // fetch cancel order ids
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
                if swap.amount_out > amm_pc_vault.amount {
                    // need settle funds
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
                        Some(&amm_pc_vault_info.clone()),
                        AUTHORITY_AMM,
                        amm.nonce as u8,
                    )?;
                }
            }
            // deposit source coin to amm_coin_vault
            Invokers::token_transfer(
                program_id,
                token_program_info.clone(),
                user_source_info.clone(),
                amm_coin_vault_info.clone(),
                user_source_owner.clone(),
                swap_in_after_add_fee,
                amm_coin_mint_info.clone(),
                &[],
            )?;
            // withdraw amm_pc_vault to destination pc
            Invokers::token_transfer_with_authority(
                program_id,
                token_program_info.clone(),
                amm_pc_vault_info.clone(),
                user_destination_info.clone(),
                amm_authority_info.clone(),
                AUTHORITY_AMM,
                amm.nonce as u8,
                swap.amount_out,
                amm_pc_mint_info.clone(),
                &[],
            )?;
            // update state_data data
            amm.state_data.swap_coin_in_amount = amm
                .state_data
                .swap_coin_in_amount
                .checked_add(swap_in_after_add_fee.into())
                .unwrap();
            amm.state_data.swap_pc_out_amount = amm
                .state_data
                .swap_pc_out_amount
                .checked_add(Calculator::to_u128(swap.amount_out)?)
                .unwrap();
            // charge coin as swap fee
            amm.state_data.swap_acc_coin_fee = amm
                .state_data
                .swap_acc_coin_fee
                .checked_add(swap_fee)
                .unwrap();
        }
        SwapDirection::PC2Coin => {
            if swap.amount_out >= total_coin_without_take_pnl {
                return Err(AmmError::InsufficientFunds.into());
            }

            if enable_orderbook {
                // pc -> coin, need cancel sell order
                if !asks.is_empty() {
                    let mut amm_order_ids_vec = Vec::new();
                    let mut order_ids = [0u64; 8];
                    let mut count = 0;
                    // fetch cancel order ids
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
                if swap.amount_out > amm_coin_vault.amount {
                    Invokers::invoke_dex_settle_funds(
                        market_program_info.clone(),
                        market_info.clone(),
                        amm_open_orders_info.clone(),
                        amm_authority_info.clone(),
                        market_asks_info.clone(),
                        market_pc_vault_info.clone(),
                        amm_coin_vault_info.clone(),
                        amm_pc_vault_info.clone(),
                        market_vault_signer.clone(),
                        token_program_info.clone(),
                        Some(&amm_pc_vault_info.clone()),
                        AUTHORITY_AMM,
                        amm.nonce as u8,
                    )?;
                }
            }

            // deposit source pc to amm_pc_vault
            Invokers::token_transfer(
                program_id,
                token_program_info.clone(),
                user_source_info.clone(),
                amm_pc_vault_info.clone(),
                user_source_owner.clone(),
                swap_in_after_add_fee,
                amm_pc_mint_info.clone(),
                &[],
            )?;
            // withdraw amm_coin_vault to destination coin
            Invokers::token_transfer_with_authority(
                program_id,
                token_program_info.clone(),
                amm_coin_vault_info.clone(),
                user_destination_info.clone(),
                amm_authority_info.clone(),
                AUTHORITY_AMM,
                amm.nonce as u8,
                swap.amount_out,
                amm_coin_mint_info.clone(),
                &[],
            )?;
            // update state_data data
            amm.state_data.swap_pc_in_amount = amm
                .state_data
                .swap_pc_in_amount
                .checked_add(swap_in_after_add_fee.into())
                .unwrap();
            amm.state_data.swap_coin_out_amount = amm
                .state_data
                .swap_coin_out_amount
                .checked_add(swap.amount_out.into())
                .unwrap();
            // charge pc as swap fee
            amm.state_data.swap_acc_pc_fee = amm
                .state_data
                .swap_acc_pc_fee
                .checked_add(swap_fee)
                .unwrap();
        }
    };
    amm.recent_epoch = Clock::get()?.epoch;

    Ok(())
} 