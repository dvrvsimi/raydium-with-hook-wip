//! Handles the initialize2 instruction logic for Raydium AMM
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    entrypoint::ProgramResult,
    pubkey::Pubkey,
    msg,
};
use crate::{
    error::AmmError,
    instruction::InitializeInstruction2,
    state::{AmmInfo, TargetOrders, TargetOrder, AmmStatus, MAX_ORDER_LIMIT},
};


use crate::process::helpers::authority_id;
use crate::process::constants::AUTHORITY_AMM;

pub fn process_initialize2(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    init: InitializeInstruction2,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let amm_info = next_account_info(account_info_iter)?;
    let amm_authority_info = next_account_info(account_info_iter)?;
    let amm_open_orders_info = next_account_info(account_info_iter)?;
    let amm_target_orders_info = next_account_info(account_info_iter)?;
    let amm_coin_vault_info = next_account_info(account_info_iter)?;
    let amm_pc_vault_info = next_account_info(account_info_iter)?;
    let amm_lp_mint_info = next_account_info(account_info_iter)?;
    let pool_withdraw_queue_info = next_account_info(account_info_iter)?;
    let lp_withdraw_queue_info = next_account_info(account_info_iter)?;
    let market_program_info = next_account_info(account_info_iter)?;
    let market_info = next_account_info(account_info_iter)?;
    let market_bids_info = next_account_info(account_info_iter)?;
    let market_asks_info = next_account_info(account_info_iter)?;
    let market_event_queue_info = next_account_info(account_info_iter)?;
    let market_coin_vault_info = next_account_info(account_info_iter)?;
    let market_pc_vault_info = next_account_info(account_info_iter)?;
    let market_vault_signer = next_account_info(account_info_iter)?;
    let token_program_info = next_account_info(account_info_iter)?;
    let system_program_info = next_account_info(account_info_iter)?;
    let rent_info = next_account_info(account_info_iter)?;
    let user_wallet_info = next_account_info(account_info_iter)?;
    let user_token_coin_info = next_account_info(account_info_iter)?;
    let user_token_pc_info = next_account_info(account_info_iter)?;
    let user_token_lp_info = next_account_info(account_info_iter)?;
    let coin_mint_info = next_account_info(account_info_iter)?;
    let pc_mint_info = next_account_info(account_info_iter)?;
    let srm_token_info = next_account_info(account_info_iter)?;
    let referrer_pc_info = next_account_info(account_info_iter)?;

    if !user_wallet_info.is_signer {
        return Err(AmmError::InvalidSignAccount.into());
    }

    // Check token program
    if *token_program_info.key != spl_token::id() {
        return Err(AmmError::InvalidSplTokenProgram.into());
    }

    // Check system program
    if *system_program_info.key != solana_program::system_program::id() {
        return Err(AmmError::InvalidMarketProgram.into());
    }

    // Check rent sysvar
    if *rent_info.key != solana_program::sysvar::rent::id() {
        return Err(AmmError::InvalidMarketProgram.into());
    }

    // Generate authority
    let authority = authority_id(program_id, AUTHORITY_AMM, init.nonce)?;
    if *amm_authority_info.key != authority {
        return Err(AmmError::InvalidProgramAddress.into());
    }

    // Initialize AMM info with default values and the provided fields
    let mut amm = AmmInfo {
        status: AmmStatus::Initialized.into_u64(),
        nonce: init.nonce as u64,
        order_num: 0,
        depth: 0,
        coin_decimals: 0,
        pc_decimals: 0,
        state: 0,
        reset_flag: 0,
        min_size: 0,
        vol_max_cut_ratio: 0,
        amount_wave: 0,
        coin_lot_size: 0,
        pc_lot_size: 0,
        min_price_multiplier: 0,
        max_price_multiplier: 0,
        sys_decimal_value: 0,
        fees: crate::state::Fees::default(),
        state_data: crate::state::StateData::default(),
        coin_vault: *amm_coin_vault_info.key,
        pc_vault: *amm_pc_vault_info.key,
        coin_vault_mint: *coin_mint_info.key,
        pc_vault_mint: *pc_mint_info.key,
        lp_mint: *amm_lp_mint_info.key,
        open_orders: *amm_open_orders_info.key,
        market: *market_info.key,
        market_program: *market_program_info.key,
        target_orders: *amm_target_orders_info.key,
        padding1: [0; 8],
        amm_owner: *user_wallet_info.key,
        lp_amount: 0,
        client_order_id: 0,
        recent_epoch: 0,
        padding2: 0,
    };

    // Initialize state data
    amm.state_data.initialize(init.open_time)?;

    // Initialize fees
    amm.fees.initialize()?;

    // Save AMM info by writing to account data
    let mut amm_data = amm_info.try_borrow_mut_data()?;
    amm_data.copy_from_slice(&bytemuck::bytes_of(&amm));

    // Initialize target orders manually since Default is only available in test mode
    let mut target_orders = TargetOrders {
        owner: [0; 4],
        buy_orders: [TargetOrder { price: 0, vol: 0 }; 50],
        padding1: [0; 8],
        target_x: 0,
        target_y: 0,
        plan_x_buy: 0,
        plan_y_buy: 0,
        plan_x_sell: 0,
        plan_y_sell: 0,
        placed_x: 0,
        placed_y: 0,
        calc_pnl_x: 0,
        calc_pnl_y: 0,
        sell_orders: [TargetOrder { price: 0, vol: 0 }; 50],
        padding2: [0; 6],
        replace_buy_client_id: [0; MAX_ORDER_LIMIT],
        replace_sell_client_id: [0; MAX_ORDER_LIMIT],
        last_order_numerator: 0,
        last_order_denominator: 0,
        plan_orders_cur: 0,
        place_orders_cur: 0,
        valid_buy_order_num: 0,
        valid_sell_order_num: 0,
        padding3: [0; 10],
        free_slot_bits: std::u128::MAX,
    };
    target_orders.check_init(0, 0, amm_info.key)?;
    
    let mut target_orders_data = amm_target_orders_info.try_borrow_mut_data()?;
    target_orders_data.copy_from_slice(&bytemuck::bytes_of(&target_orders));

    // Calculate initial LP amount (geometric mean)
    let product = (init.init_coin_amount as u128)
        .checked_mul(init.init_pc_amount as u128)
        .unwrap();
    
    // Simple integer square root implementation
    let mut initial_lp_amount = 0u64;
    if product > 0 {
        let mut x = product;
        let mut y = (x + 1) / 2;
        while y < x {
            x = y;
            y = (x + product / x) / 2;
        }
        initial_lp_amount = x as u64;
    }

    // Mint initial LP tokens using spl_token directly
    let mint_ix = spl_token::instruction::mint_to(
        &spl_token::id(),
        amm_lp_mint_info.key,
        user_token_lp_info.key,
        amm_authority_info.key,
        &[],
        initial_lp_amount,
    )?;

    solana_program::program::invoke_signed(
        &mint_ix,
        &[
            amm_lp_mint_info.clone(),
            user_token_lp_info.clone(),
            amm_authority_info.clone(),
        ],
        &[&[AUTHORITY_AMM, &[init.nonce]]],
    )?;

    amm.lp_amount = initial_lp_amount;
    let mut amm_data = amm_info.try_borrow_mut_data()?;
    amm_data.copy_from_slice(&bytemuck::bytes_of(&amm));

    // Transfer initial tokens from user to AMM vaults
    let transfer_coin_ix = spl_token::instruction::transfer(
        &spl_token::id(),
        user_token_coin_info.key,
        amm_coin_vault_info.key,
        user_wallet_info.key,
        &[],
        init.init_coin_amount,
    )?;

    let transfer_pc_ix = spl_token::instruction::transfer(
        &spl_token::id(),
        user_token_pc_info.key,
        amm_pc_vault_info.key,
        user_wallet_info.key,
        &[],
        init.init_pc_amount,
    )?;

    // Execute token transfers
    solana_program::program::invoke(
        &transfer_coin_ix,
        &[
            user_token_coin_info.clone(),
            amm_coin_vault_info.clone(),
            user_wallet_info.clone(),
        ],
    )?;

    solana_program::program::invoke(
        &transfer_pc_ix,
        &[
            user_token_pc_info.clone(),
            amm_pc_vault_info.clone(),
            user_wallet_info.clone(),
        ],
    )?;

    // Initialize market accounts if needed
    // Note: In a real implementation, you'd also initialize the Serum/OpenBook market
    // For now, we'll just validate the accounts are present

    msg!("AMM initialized successfully with {} LP tokens", initial_lp_amount);
    msg!("Initial coin amount: {}", init.init_coin_amount);
    msg!("Initial pc amount: {}", init.init_pc_amount);
    Ok(())
} 