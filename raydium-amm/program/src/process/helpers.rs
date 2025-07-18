use solana_program::{
    account_info::AccountInfo,
    pubkey::Pubkey,
    program_error::ProgramError,
    msg,
};
use crate::state::{AmmInfo, TargetOrders};
use crate::math::{U256, Calculator, U128};
use crate::error::AmmError;
use serum_dex::state::{MarketState, OpenOrders, ToAlignedBytes};
use arrform::{arrform, ArrForm};
use serde::Serialize;
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

pub fn calc_take_pnl(
    target: &TargetOrders,
    amm: &mut AmmInfo,
    total_pc_without_take_pnl: &mut u64,
    total_coin_without_take_pnl: &mut u64,
    x1: U256,
    y1: U256,
) -> Result<(u128, u128), ProgramError> {
    // calc pnl
    let mut delta_x: u128;
    let mut delta_y: u128;
    let calc_pc_amount = Calculator::restore_decimal(
        target.calc_pnl_x.into(),
        amm.pc_decimals,
        amm.sys_decimal_value,
    );
    let calc_coin_amount = Calculator::restore_decimal(
        target.calc_pnl_y.into(),
        amm.coin_decimals,
        amm.sys_decimal_value,
    );
    let pool_pc_amount = U128::from(*total_pc_without_take_pnl);
    let pool_coin_amount = U128::from(*total_coin_without_take_pnl);
    if pool_pc_amount.checked_mul(pool_coin_amount).unwrap()
        >= (calc_pc_amount).checked_mul(calc_coin_amount).unwrap()
    {
        let x2_power = Calculator::calc_x_power(
            target.calc_pnl_x.into(),
            target.calc_pnl_y.into(),
            x1,
            y1,
        );
        let x2 = x2_power.integer_sqrt();
        let y2 = x2.checked_mul(y1).unwrap().checked_div(x1).unwrap();

        // transfer to token_coin_pnl and token_pc_pnl
        // (x1 -x2) * pnl / sys_decimal_value
        let diff_x = U128::from(x1.checked_sub(x2).unwrap().as_u128());
        let diff_y = U128::from(y1.checked_sub(y2).unwrap().as_u128());
        delta_x = diff_x
            .checked_mul(amm.fees.pnl_numerator.into())
            .unwrap()
            .checked_div(amm.fees.pnl_denominator.into())
            .unwrap()
            .as_u128();
        delta_y = diff_y
            .checked_mul(amm.fees.pnl_numerator.into())
            .unwrap()
            .checked_div(amm.fees.pnl_denominator.into())
            .unwrap()
            .as_u128();

        let diff_pc_pnl_amount =
            Calculator::restore_decimal(diff_x, amm.pc_decimals, amm.sys_decimal_value);
        let diff_coin_pnl_amount =
            Calculator::restore_decimal(diff_y, amm.coin_decimals, amm.sys_decimal_value);
        let pc_pnl_amount = diff_pc_pnl_amount
            .checked_mul(amm.fees.pnl_numerator.into())
            .unwrap()
            .checked_div(amm.fees.pnl_denominator.into())
            .unwrap()
            .as_u64();
        let coin_pnl_amount = diff_coin_pnl_amount
            .checked_mul(amm.fees.pnl_numerator.into())
            .unwrap()
            .checked_div(amm.fees.pnl_denominator.into())
            .unwrap()
            .as_u64();
        if pc_pnl_amount != 0 && coin_pnl_amount != 0 {
            // step2: save total_pnl_pc & total_pnl_coin
            amm.state_data.total_pnl_pc = amm
                .state_data
                .total_pnl_pc
                .checked_add(diff_pc_pnl_amount.as_u64())
                .unwrap();
            amm.state_data.total_pnl_coin = amm
                .state_data
                .total_pnl_coin
                .checked_add(diff_coin_pnl_amount.as_u64())
                .unwrap();
            amm.state_data.need_take_pnl_pc = amm
                .state_data
                .need_take_pnl_pc
                .checked_add(pc_pnl_amount)
                .unwrap();
            amm.state_data.need_take_pnl_coin = amm
                .state_data
                .need_take_pnl_coin
                .checked_add(coin_pnl_amount)
                .unwrap();

            // step3: update total_coin and total_pc without pnl
            *total_pc_without_take_pnl = (*total_pc_without_take_pnl)
                .checked_sub(pc_pnl_amount)
                .unwrap();
            *total_coin_without_take_pnl = (*total_coin_without_take_pnl)
                .checked_sub(coin_pnl_amount)
                .unwrap();
        } else {
            delta_x = 0;
            delta_y = 0;
        }
    } else {
        msg!(arrform!(
            LOG_SIZE,
            "calc_take_pnl error x:{}, y:{}, calc_pnl_x:{}, calc_pnl_y:{}",
            x1,
            y1,
            identity(target.calc_pnl_x),
            identity(target.calc_pnl_y)
        )
        .as_str());
        return Err(AmmError::CalcPnlError.into());
    }

    Ok((delta_x, delta_y))
}

pub fn identity<T>(x: T) -> T { x }

pub fn encode_ray_log<T: Serialize>(log: T) {
    // encode
    let bytes = bincode::serialize(&log).unwrap();
    let mut out_buf = Vec::new();
    out_buf.resize(bytes.len() * 4 / 3 + 4, 0);
    let bytes_written = base64::encode_config_slice(bytes, base64::STANDARD, &mut out_buf);
    out_buf.resize(bytes_written, 0);
    let msg_str = unsafe { std::str::from_utf8_unchecked(&out_buf) };
    msg!(arrform!(LOG_SIZE, "ray_log: {}", msg_str).as_str());
} 