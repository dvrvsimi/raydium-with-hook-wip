//! Program state invoker

use solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
};
use std::num::NonZeroU64;
use spl_token_2022::extension::{
    StateWithExtensions, transfer_hook::TransferHook,
    StateWithExtensionsMut, transfer_hook_account::TransferHookAccount,
};
// use spl_transfer_hook_interface::instruction::TransferHookInstruction;
use solana_program::program_pack::Pack;
// use spl_transfer_hook_interface::state::ExtraAccountMetaList;

// Hardcoded whitelist for demo (replace with on-chain registry for production)
const HOOK_WHITELIST: &[Pubkey] = &[
    // Add allowed hook program pubkeys here
    Pubkey::new_from_array([
        0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf0,
        0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88,
        0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff, 0x00,
        0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
    ]), // TODO: convert base64 to pubkey
    
];

/// Helper to get the transfer hook program id from a mint's TLV extension
fn get_transfer_hook_program_id(mint_account: &AccountInfo) -> Option<Pubkey> {
    let data = mint_account.data.borrow();
    let state = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&data).ok()?;
    let ext = state.get_extension::<TransferHook>().ok()?;
    Some(ext.program_id)
}

/// Set and unset the transferring flag for a token account
fn set_transferring(account: &AccountInfo) -> Result<(), ProgramError> {
    let mut data = account.data.borrow_mut();
    let mut state = StateWithExtensionsMut::<spl_token_2022::state::Account>::unpack(&mut data)?;
    let ext = state.get_extension_mut::<TransferHookAccount>()?;
    ext.transferring = true.into();
    Ok(())
}

fn unset_transferring(account: &AccountInfo) -> Result<(), ProgramError> {
    let mut data = account.data.borrow_mut();
    let mut state = StateWithExtensionsMut::<spl_token_2022::state::Account>::unpack(&mut data)?;
    let ext = state.get_extension_mut::<TransferHookAccount>()?;
    ext.transferring = false.into();
    Ok(())
}

pub struct Invokers {}

impl Invokers {
    /// Issue a associated_spl_token `create_associated_token_account` instruction
    pub fn create_ata_spl_token<'a>(
        associated_account: AccountInfo<'a>,
        funding_account: AccountInfo<'a>,
        wallet_account: AccountInfo<'a>,
        token_mint_account: AccountInfo<'a>,
        token_program_account: AccountInfo<'a>,
        ata_program_account: AccountInfo<'a>,
        system_program_account: AccountInfo<'a>,
    ) -> Result<(), ProgramError> {
        let ix = spl_associated_token_account::instruction::create_associated_token_account(
            funding_account.key,
            wallet_account.key,
            token_mint_account.key,
            token_program_account.key,
        );
        solana_program::program::invoke_signed(
            &ix,
            &[
                associated_account,
                funding_account,
                wallet_account,
                token_mint_account,
                token_program_account,
                ata_program_account,
                system_program_account,
            ],
            &[],
        )
    }
    /// Issue a spl_token `Burn` instruction.
    pub fn token_burn<'a>(
        token_program: AccountInfo<'a>,
        burn_account: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        owner: AccountInfo<'a>,
        burn_amount: u64,
    ) -> Result<(), ProgramError> {
        let ix = spl_token::instruction::burn(
            token_program.key,
            burn_account.key,
            mint.key,
            owner.key,
            &[],
            burn_amount,
        )?;

        solana_program::program::invoke_signed(
            &ix,
            &[burn_account, mint, owner, token_program],
            &[],
        )
    }

    /// Close Account
    pub fn token_close_with_authority<'a>(
        token_program: AccountInfo<'a>,
        close_account: AccountInfo<'a>,
        destination_account: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        amm_seed: &[u8],
        nonce: u8,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::close_account(
            token_program.key,
            close_account.key,
            destination_account.key,
            authority.key,
            &[],
        )?;

        solana_program::program::invoke_signed(
            &ix,
            &[close_account, destination_account, authority, token_program],
            signers,
        )
    }

    /// Issue a spl_token `Burn` instruction.
    pub fn token_burn_with_authority<'a>(
        token_program: AccountInfo<'a>,
        burn_account: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        amm_seed: &[u8],
        nonce: u8,
        burn_amount: u64,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::burn(
            token_program.key,
            burn_account.key,
            mint.key,
            authority.key,
            &[],
            burn_amount,
        )?;

        solana_program::program::invoke_signed(
            &ix,
            &[burn_account, mint, authority, token_program],
            signers,
        )
    }

    /// Issue a spl_token `MintTo` instruction.
    pub fn token_mint_to<'a>(
        token_program: AccountInfo<'a>,
        mint: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        amm_seed: &[u8],
        nonce: u8,
        amount: u64,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::mint_to(
            token_program.key,
            mint.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?;

        solana_program::program::invoke_signed(
            &ix,
            &[mint, destination, authority, token_program],
            signers,
        )
    }

    /// Issue a spl_token `Transfer` instruction.
    pub fn token_transfer<'a>(
        token_program: AccountInfo<'a>,
        source: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        owner: AccountInfo<'a>,
        deposit_amount: u64,
        mint: AccountInfo<'a>,
        remaining_accounts: &[AccountInfo<'a>],
    ) -> Result<(), ProgramError> {
        if let Some(hook_program_id) = get_transfer_hook_program_id(&mint) {
            if !HOOK_WHITELIST.contains(&hook_program_id) {
                return Err(ProgramError::Custom(1234)); // Use a real error code
            }
            // Set transferring flag (ignore error if not present)
            let _ = set_transferring(&source);
            let hook_ix_data = TransferHookInstruction::Execute {
                amount: deposit_amount,
            }
            .pack();
            let (extra_account_meta_list_pda, _bump) = Pubkey::find_program_address(
                &[b"extra-account-metas", mint.key.as_ref()],
                &hook_program_id,
            );
            let mut account_metas = vec![
                AccountMeta::new(*source.key, false),
                AccountMeta::new(*mint.key, false),
                AccountMeta::new(*destination.key, false),
                AccountMeta::new(*owner.key, false),
                AccountMeta::new(extra_account_meta_list_pda, false),
            ];
            let mut account_infos = vec![
                source.clone(),
                mint.clone(),
                destination.clone(),
                owner.clone(),
            ];
            // Find the ExtraAccountMetaList AccountInfo in remaining_accounts
            let extra_account_meta_list_info = remaining_accounts.iter().find(|ai| ai.key == &extra_account_meta_list_pda)
                .ok_or(ProgramError::NotEnoughAccountKeys)?;
            account_infos.push(extra_account_meta_list_info.clone());
            // Parse the ExtraAccountMetaList
            let meta_list_data = extra_account_meta_list_info.try_borrow_data()?;
            let extra_account_meta_list = ExtraAccountMetaList::unpack(&meta_list_data)?;
            // Add extra accounts in order
            let mut extra_account_index = 0;
            for meta in extra_account_meta_list.iter() {
                let acc_info = remaining_accounts.get(extra_account_index)
                    .ok_or(ProgramError::NotEnoughAccountKeys)?;
                account_metas.push(meta.to_account_meta());
                account_infos.push(acc_info.clone());
                extra_account_index += 1;
            }
            let hook_ix = Instruction {
                program_id: hook_program_id,
                accounts: account_metas,
                data: hook_ix_data,
            };
            solana_program::program::invoke(
                &hook_ix,
                &account_infos,
            )?;
            let _ = unset_transferring(&source);
        }
        let ix = spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            owner.key,
            &[],
            deposit_amount,
        )?;
        solana_program::program::invoke_signed(
            &ix,
            &[source, destination, owner, token_program],
            &[],
        )
    }

    /// Issue a spl_token `Transfer` instruction.
    pub fn token_transfer_with_authority<'a>(
        token_program: AccountInfo<'a>,
        source: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        authority: AccountInfo<'a>,
        amm_seed: &[u8],
        nonce: u8,
        amount: u64,
        mint: AccountInfo<'a>,
        remaining_accounts: &[AccountInfo<'a>],
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];
        if let Some(hook_program_id) = get_transfer_hook_program_id(&mint) {
            if !HOOK_WHITELIST.contains(&hook_program_id) {
                return Err(ProgramError::Custom(1234));
            }
            let _ = set_transferring(&source);
            let hook_ix_data = TransferHookInstruction::Execute {
                amount,
            }
            .pack();
            let (extra_account_meta_list_pda, _bump) = Pubkey::find_program_address(
                &[b"extra-account-metas", mint.key.as_ref()],
                &hook_program_id,
            );
            let mut account_metas = vec![
                AccountMeta::new(*source.key, false),
                AccountMeta::new(*mint.key, false),
                AccountMeta::new(*destination.key, false),
                AccountMeta::new(*authority.key, false),
                AccountMeta::new(extra_account_meta_list_pda, false),
            ];
            let mut account_infos = vec![
                source.clone(),
                mint.clone(),
                destination.clone(),
                authority.clone(),
            ];
            // Find the ExtraAccountMetaList AccountInfo in remaining_accounts
            let extra_account_meta_list_info = remaining_accounts.iter().find(|ai| ai.key == &extra_account_meta_list_pda)
                .ok_or(ProgramError::NotEnoughAccountKeys)?;
            account_infos.push(extra_account_meta_list_info.clone());
            // Parse the ExtraAccountMetaList
            let meta_list_data = extra_account_meta_list_info.try_borrow_data()?;
            let extra_account_meta_list = ExtraAccountMetaList::unpack(&meta_list_data)?;
            // Add extra accounts in order
            let mut extra_account_index = 0;
            for meta in extra_account_meta_list.iter() {
                let acc_info = remaining_accounts.get(extra_account_index)
                    .ok_or(ProgramError::NotEnoughAccountKeys)?;
                account_metas.push(meta.to_account_meta());
                account_infos.push(acc_info.clone());
                extra_account_index += 1;
            }
            let hook_ix = Instruction {
                program_id: hook_program_id,
                accounts: account_metas,
                data: hook_ix_data,
            };
            solana_program::program::invoke(
                &hook_ix,
                &account_infos,
            )?;
            let _ = unset_transferring(&source);
        }
        let ix = spl_token::instruction::transfer(
            token_program.key,
            source.key,
            destination.key,
            authority.key,
            &[],
            amount,
        )?;
        solana_program::program::invoke_signed(
            &ix,
            &[source, destination, authority, token_program],
            signers,
        )
    }

    pub fn token_set_authority<'a>(
        token_program: AccountInfo<'a>,
        account: AccountInfo<'a>, // mint or token account
        authority: AccountInfo<'a>,
        new_authority: AccountInfo<'a>,
        amm_seed: &[u8],
        authority_nonce: u8,
        authority_type: spl_token::instruction::AuthorityType,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[authority_nonce]];
        let signers = &[&authority_signature_seeds[..]];
        let ix = spl_token::instruction::set_authority(
            token_program.key,
            account.key,
            Some(new_authority.key),
            authority_type,
            authority.key,
            &[],
        )?;
        solana_program::program::invoke_signed(&ix, &[account, authority, token_program], signers)
    }

    /// Issue a dex `InitOpenOrders` instruction
    pub fn invoke_dex_init_open_orders<'a>(
        dex_program: AccountInfo<'a>,
        open_orders: AccountInfo<'a>,
        open_orders_owner: AccountInfo<'a>,
        market: AccountInfo<'a>,
        rent_sysvar: AccountInfo<'a>,
        amm_seed: &[u8],
        nonce: u8,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];

        let ix = serum_dex::instruction::init_open_orders(
            dex_program.key,
            open_orders.key,
            open_orders_owner.key,
            market.key,
            None,
        )?;

        let accounts = vec![
            dex_program,
            open_orders,
            open_orders_owner,
            market,
            rent_sysvar,
        ];
        solana_program::program::invoke_signed(&ix, &accounts, signers)
    }

    pub fn invoke_dex_close_open_orders<'a>(
        dex_program: AccountInfo<'a>,
        open_orders: AccountInfo<'a>,
        open_orders_owner: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        market: AccountInfo<'a>,
        amm_seed: &[u8],
        nonce: u8,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];

        let ix = serum_dex::instruction::close_open_orders(
            dex_program.key,
            open_orders.key,
            open_orders_owner.key,
            destination.key,
            market.key,
        )?;
        let accounts = vec![
            dex_program,
            open_orders,
            open_orders_owner,
            destination,
            market,
        ];
        solana_program::program::invoke_signed(&ix, &accounts, signers)
    }

    pub fn replace_order_by_client_id(
        market: &Pubkey,
        open_orders_account: &Pubkey,
        request_queue: &Pubkey,
        event_queue: &Pubkey,
        market_bids: &Pubkey,
        market_asks: &Pubkey,
        order_payer: &Pubkey,
        open_orders_account_owner: &Pubkey,
        coin_vault: &Pubkey,
        pc_vault: &Pubkey,
        spl_token_program_id: &Pubkey,
        rent_sysvar_id: &Pubkey,
        srm_account_referral: Option<&Pubkey>,
        program_id: &Pubkey,
        side: serum_dex::matching::Side,
        limit_price: NonZeroU64,
        max_coin_qty: NonZeroU64,
        order_type: serum_dex::matching::OrderType,
        client_order_id: u64,
        self_trade_behavior: serum_dex::instruction::SelfTradeBehavior,
        limit: u16,
        max_native_pc_qty_including_fees: NonZeroU64,
        max_ts: i64,
    ) -> Result<Instruction, serum_dex::error::DexError> {
        let data = serum_dex::instruction::MarketInstruction::ReplaceOrderByClientId(
            serum_dex::instruction::NewOrderInstructionV3 {
                side,
                limit_price,
                max_coin_qty,
                order_type,
                client_order_id,
                self_trade_behavior,
                limit,
                max_native_pc_qty_including_fees,
                max_ts,
            },
        )
        .pack();
        let mut accounts = vec![
            AccountMeta::new(*market, false),
            AccountMeta::new(*open_orders_account, false),
            AccountMeta::new(*request_queue, false),
            AccountMeta::new(*event_queue, false),
            AccountMeta::new(*market_bids, false),
            AccountMeta::new(*market_asks, false),
            AccountMeta::new(*order_payer, false),
            AccountMeta::new_readonly(*open_orders_account_owner, true),
            AccountMeta::new(*coin_vault, false),
            AccountMeta::new(*pc_vault, false),
            AccountMeta::new_readonly(*spl_token_program_id, false),
            AccountMeta::new_readonly(*rent_sysvar_id, false),
        ];
        if let Some(key) = srm_account_referral {
            accounts.push(AccountMeta::new_readonly(*key, false))
        }
        Ok(Instruction {
            program_id: *program_id,
            data,
            accounts,
        })
    }
    /// Issue a dex `ReplaceOrderByClientId` instruction.
    pub fn invoke_dex_replace_order_by_client_id<'a>(
        dex_program: AccountInfo<'a>,
        market: AccountInfo<'a>,
        open_orders: AccountInfo<'a>,
        req_q: AccountInfo<'a>,
        event_q: AccountInfo<'a>,
        bids: AccountInfo<'a>,
        asks: AccountInfo<'a>,
        payer: AccountInfo<'a>,
        open_orders_owner: AccountInfo<'a>,
        coin_vault: AccountInfo<'a>,
        pc_vault: AccountInfo<'a>,
        token_program: AccountInfo<'a>,
        rent_account: AccountInfo<'a>,
        srm_account_referral: Option<&AccountInfo<'a>>,
        amm_seed: &[u8],
        nonce: u8,

        side: serum_dex::matching::Side,
        limit_price: NonZeroU64,
        max_coin_qty: NonZeroU64,
        max_native_pc_qty_including_fees: NonZeroU64,
        order_type: serum_dex::matching::OrderType,
        client_order_id: u64,
        limit: u16,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];

        let mut srm_account_referral_key = None;
        if let Some(srm_account_referral_account) = srm_account_referral {
            srm_account_referral_key = Some(srm_account_referral_account.key);
        }

        let ix = Self::replace_order_by_client_id(
            market.key,
            open_orders.key,
            req_q.key,
            event_q.key,
            bids.key,
            asks.key,
            payer.key,
            open_orders_owner.key,
            coin_vault.key,
            pc_vault.key,
            token_program.key,
            rent_account.key,
            srm_account_referral_key,
            dex_program.key,
            side,
            limit_price,
            max_coin_qty,
            order_type,
            client_order_id,
            serum_dex::instruction::SelfTradeBehavior::CancelProvide,
            limit,
            max_native_pc_qty_including_fees,
            i64::MAX,
        )?;

        let mut accounts = vec![
            dex_program,
            market,
            open_orders,
            req_q,
            event_q,
            bids,
            asks,
            payer,
            open_orders_owner,
            coin_vault,
            pc_vault,
            token_program,
            rent_account,
        ];
        if let Some(srm_account) = srm_account_referral {
            accounts.push(srm_account.clone());
        }

        solana_program::program::invoke_signed(&ix, &accounts, signers)
    }

    /// Issue a dex `NewOrder` instruction.
    pub fn invoke_dex_new_order_v3<'a>(
        dex_program: AccountInfo<'a>,
        market: AccountInfo<'a>,
        open_orders: AccountInfo<'a>,
        req_q: AccountInfo<'a>,
        event_q: AccountInfo<'a>,
        bids: AccountInfo<'a>,
        asks: AccountInfo<'a>,
        payer: AccountInfo<'a>,
        open_orders_owner: AccountInfo<'a>,
        coin_vault: AccountInfo<'a>,
        pc_vault: AccountInfo<'a>,
        token_program: AccountInfo<'a>,
        rent_account: AccountInfo<'a>,
        srm_account_referral: Option<&AccountInfo<'a>>,
        amm_seed: &[u8],
        nonce: u8,

        side: serum_dex::matching::Side,
        limit_price: NonZeroU64,
        max_coin_qty: NonZeroU64,
        max_native_pc_qty_including_fees: NonZeroU64,
        order_type: serum_dex::matching::OrderType,
        client_order_id: u64,
        limit: u16,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];

        let mut srm_account_referral_key = None;
        if let Some(srm_account_referral_account) = srm_account_referral {
            srm_account_referral_key = Some(srm_account_referral_account.key);
        }

        let ix = serum_dex::instruction::new_order(
            market.key,
            open_orders.key,
            req_q.key,
            event_q.key,
            bids.key,
            asks.key,
            payer.key,
            open_orders_owner.key,
            coin_vault.key,
            pc_vault.key,
            token_program.key,
            rent_account.key,
            srm_account_referral_key,
            dex_program.key,
            side,
            limit_price,
            max_coin_qty,
            order_type,
            client_order_id,
            serum_dex::instruction::SelfTradeBehavior::CancelProvide,
            limit,
            max_native_pc_qty_including_fees,
            i64::MAX,
        )?;

        let mut accounts = vec![
            dex_program,
            market,
            open_orders,
            req_q,
            event_q,
            bids,
            asks,
            payer,
            open_orders_owner,
            coin_vault,
            pc_vault,
            token_program,
            rent_account,
        ];
        if let Some(srm_account) = srm_account_referral {
            accounts.push(srm_account.clone());
        }

        solana_program::program::invoke_signed(&ix, &accounts, signers)
    }

    /// Issue a dex `CancelOrder` instruction.
    pub fn invoke_dex_cancel_order_v2<'a>(
        dex_program: AccountInfo<'a>,
        market: AccountInfo<'a>,
        bids: AccountInfo<'a>,
        asks: AccountInfo<'a>,
        open_orders: AccountInfo<'a>,
        open_orders_owner: AccountInfo<'a>,
        event_q: AccountInfo<'a>,
        amm_seed: &[u8],
        nonce: u8,

        side: serum_dex::matching::Side,
        order_id: u128,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];

        let ix = serum_dex::instruction::cancel_order(
            dex_program.key,
            market.key,
            bids.key,
            asks.key,
            open_orders.key,
            open_orders_owner.key,
            event_q.key,
            side,
            order_id,
        )?;
        let accounts = [
            dex_program,
            market,
            bids,
            asks,
            open_orders,
            open_orders_owner,
            event_q,
        ];
        solana_program::program::invoke_signed(&ix, &accounts, signers)
    }

    /// Issue a dex `CancelOrdersByClientIds` instruction.
    pub fn invoke_dex_cancel_orders_by_client_order_ids<'a>(
        dex_program: AccountInfo<'a>,
        market: AccountInfo<'a>,
        bids: AccountInfo<'a>,
        asks: AccountInfo<'a>,
        open_orders: AccountInfo<'a>,
        open_orders_owner: AccountInfo<'a>,
        event_q: AccountInfo<'a>,
        amm_seed: &[u8],
        nonce: u8,

        client_order_ids: [u64; 8],
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];

        let ix = serum_dex::instruction::cancel_orders_by_client_order_ids(
            dex_program.key,
            market.key,
            bids.key,
            asks.key,
            open_orders.key,
            open_orders_owner.key,
            event_q.key,
            client_order_ids,
        )?;
        let accounts = [
            dex_program,
            market,
            bids,
            asks,
            open_orders,
            open_orders_owner,
            event_q,
        ];
        solana_program::program::invoke_signed(&ix, &accounts, signers)
    }

    /// Issue a dex `SettleFunds` instruction.
    pub fn invoke_dex_settle_funds<'a>(
        dex_program: AccountInfo<'a>,
        market: AccountInfo<'a>,
        open_orders: AccountInfo<'a>,
        owner: AccountInfo<'a>, //open_orders.owner
        coin_vault: AccountInfo<'a>,
        pc_vault: AccountInfo<'a>,
        coin_wallet: AccountInfo<'a>,
        pc_wallet: AccountInfo<'a>,
        vault_signer: AccountInfo<'a>,
        spl_token_program: AccountInfo<'a>,
        referrer_pc_wallet: Option<&AccountInfo<'a>>,
        amm_seed: &[u8],
        nonce: u8,
    ) -> Result<(), ProgramError> {
        let authority_signature_seeds = [amm_seed, &[nonce]];
        let signers = &[&authority_signature_seeds[..]];

        let mut referrer_pc_wallet_key = None;
        if let Some(referrer_pc_wallet_account) = referrer_pc_wallet {
            referrer_pc_wallet_key = Some(referrer_pc_wallet_account.key);
        }

        let ix = serum_dex::instruction::settle_funds(
            dex_program.key,
            market.key,
            spl_token_program.key,
            open_orders.key,
            owner.key,
            coin_vault.key,
            coin_wallet.key,
            pc_vault.key,
            pc_wallet.key,
            referrer_pc_wallet_key,
            vault_signer.key,
        )?;

        let mut accounts = vec![
            dex_program,
            market,
            open_orders,
            owner,
            coin_vault,
            pc_vault,
            coin_wallet,
            pc_wallet,
            vault_signer,
            spl_token_program,
        ];
        if let Some(referrer_pc_account) = referrer_pc_wallet {
            accounts.push(referrer_pc_account.clone());
        }
        solana_program::program::invoke_signed(&ix, &accounts, signers)
    }
}
