//! Program state invoker

use solana_program::{
    account_info::AccountInfo,
    instruction::{AccountMeta, Instruction},
    program_error::ProgramError,
    pubkey::Pubkey,
    msg,
};
use std::num::NonZeroU64;
use spl_token_2022::extension::{
    StateWithExtensions, transfer_hook::TransferHook, StateWithExtensionsMut,
};
use spl_token_2022::extension::BaseStateWithExtensions;
use spl_token_2022::extension::BaseStateWithExtensionsMut;
use spl_token_2022::state::Mint;
use spl_transfer_hook_interface::instruction::ExecuteInstruction;
use spl_tlv_account_resolution::{
    state::ExtraAccountMetaList,
    account::ExtraAccountMeta,
    seeds::Seed,
};
use spl_type_length_value::state::TlvStateBorrowed;
use spl_token_2022::extension::transfer_hook::TransferHookAccount;
use spl_transfer_hook_interface::instruction::TransferHookInstruction;
use spl_discriminator::SplDiscriminate;

use crate::process::whitelist::is_hook_whitelisted;

/// Helper to get the transfer hook program id from a mint's TLV extension
fn get_transfer_hook_program_id(mint_account: &AccountInfo) -> Option<Pubkey> {
    let data = mint_account.data.borrow();
    let state = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&data).ok()?;
    let ext = state.get_extension::<TransferHook>().ok()?;
    ext.program_id.into()
}

/// Set the transferring flag for a token account (if extension exists)
fn try_set_transferring(account: &AccountInfo) -> Result<(), ProgramError> {
    let mut data = account.data.borrow_mut();
    let mut state = StateWithExtensionsMut::<spl_token_2022::state::Account>::unpack(&mut data)?;
    
    // Only set flag if the TransferHookAccount extension exists
    if let Ok(ext) = state.get_extension_mut::<TransferHookAccount>() {
        ext.transferring = true.into();
    }
    // If extension doesn't exist, that's fine - just continue
    Ok(())
}

/// Unset the transferring flag for a token account (if extension exists)
fn try_unset_transferring(account: &AccountInfo) -> Result<(), ProgramError> {
    let mut data = account.data.borrow_mut();
    let mut state = StateWithExtensionsMut::<spl_token_2022::state::Account>::unpack(&mut data)?;
    
    // Only unset flag if the TransferHookAccount extension exists
    if let Ok(ext) = state.get_extension_mut::<TransferHookAccount>() {
        ext.transferring = false.into();
    }
    // If extension doesn't exist, continue
    Ok(())
}

/// Check if a hook program is supported for auto-initialization
fn is_hook_supported_for_auto_init(hook_program_id: &Pubkey) -> bool {
    // Add known hook program IDs that support auto-initialization
    // For now, we'll support the whitelist-transfer-hook program
    let supported_hooks = [
        // Add your whitelist-transfer-hook program ID here
        // "YourWhitelistTransferHookProgramID",
    ];
    
    supported_hooks.contains(hook_program_id)
}

/// Get default extra account metas for a supported hook program
fn get_default_extra_account_metas(hook_program_id: &Pubkey) -> Result<Vec<ExtraAccountMeta>, ProgramError> {
    // For whitelist-transfer-hook, the default extra account is the whitelist PDA
    let whitelist_meta = ExtraAccountMeta::new_with_seeds(
        &[
            Seed::Literal {
                bytes: b"whitelist".to_vec(),
            },
        ],
        false, // is_signer
        false, // is_writable
    )?;
    
    Ok(vec![whitelist_meta])
}

/// Auto-initialize the extra account meta list for a hook program
fn auto_initialize_meta_list<'a>(
    hook_program_id: &Pubkey,
    mint: &AccountInfo<'a>,
    meta_list_account: &AccountInfo<'a>,
) -> Result<(), ProgramError> {
    // Check if hook program is supported for auto-initialization
    if !is_hook_supported_for_auto_init(hook_program_id) {
        return Err(crate::error::AmmError::HookProgramNotSupportedForAutoInit.into());
    }
    
    // Get default extra account metas for this hook program
    let extra_account_metas = get_default_extra_account_metas(hook_program_id)?;
    
    // Calculate required account size
    let account_size = ExtraAccountMetaList::size_of(extra_account_metas.len())?;
    
    // Check if account has enough space
    let mut data = meta_list_account.try_borrow_mut_data()?;
    if data.len() < account_size {
        return Err(crate::error::AmmError::HookMetaListAutoInitFailed.into());
    }
    
    // Initialize the meta list
    ExtraAccountMetaList::init::<ExecuteInstruction>(
        &mut data,
        &extra_account_metas,
    )?;
    
    msg!("Auto-initialized meta list for hook program: {}", hook_program_id);
    Ok(())
}

/// Check if meta list account is valid and initialized
fn is_meta_list_valid<'a>(
    meta_list_account: &AccountInfo<'a>,
    hook_program_id: &Pubkey,
) -> Result<bool, ProgramError> {
    let data = meta_list_account.try_borrow_data()?;
    
    // Check if account is empty (not initialized)
    if data.is_empty() {
        return Ok(false);
    }
    
    // Try to parse the TLV state
    let tlv_state = match TlvStateBorrowed::unpack(&data) {
        Ok(state) => state,
        Err(_) => return Ok(false), // Invalid TLV state
    };
    
    // Try to unpack the extra account meta list
    match ExtraAccountMetaList::unpack_with_tlv_state::<ExecuteInstruction>(&tlv_state) {
        Ok(_) => Ok(true),
        Err(_) => Ok(false), // Invalid meta list
    }
}

/// Execute transfer hook for Token 2022 mints with on-chain whitelist validation
/// 
/// Expected remaining_accounts order:
/// 0. TransferHookWhitelist PDA account
/// 1. ExtraAccountMetaList PDA
/// 2..N. Additional accounts required by the hook (in order specified by the meta list)
pub fn execute_transfer_hook<'a>(
    program_id: &Pubkey, // Your AMM program ID (for whitelist PDA derivation)
    source: &AccountInfo<'a>,
    mint: &AccountInfo<'a>,
    destination: &AccountInfo<'a>,
    authority: &AccountInfo<'a>,
    amount: u64,
    remaining_accounts: &[AccountInfo<'a>],
) -> Result<(), ProgramError> {
    // Get the hook program ID from the mint
    let hook_program_id = match get_transfer_hook_program_id(mint) {
        Some(id) => id,
        None => return Ok(()), // No hook, continue with normal transfer
    };

    // Need at least whitelist and meta list accounts
    if remaining_accounts.len() < 2 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let whitelist_account = &remaining_accounts[0];
    let extra_account_meta_list_info = &remaining_accounts[1];

    // Security check: ensure hook program is whitelisted using on-chain whitelist
    match is_hook_whitelisted(program_id, whitelist_account, &hook_program_id) {
        Ok(true) => {
            msg!("Transfer hook program is whitelisted: {}", hook_program_id);
        }
        Ok(false) => {
            msg!("Transfer hook program not whitelisted: {}", hook_program_id);
            return Err(crate::error::AmmError::TransferHookNotWhitelisted.into());
        }
        Err(e) => {
            msg!("Error checking whitelist: {:?}", e);
            return Err(e);
        }
    }

    // Set transferring flag (ignore error if extension doesn't exist)
    let _ = try_set_transferring(source);

    // Create hook instruction data
    let hook_ix_data = TransferHookInstruction::Execute { amount }.pack();

    // Derive the ExtraAccountMetaList PDA
    let (extra_account_meta_list_pda, _bump) = Pubkey::find_program_address(
        &[b"extra-account-metas", mint.key.as_ref()],
        &hook_program_id,
    );

    // Verify the ExtraAccountMetaList PDA
    if extra_account_meta_list_info.key != &extra_account_meta_list_pda {
        let _ = try_unset_transferring(source);
        return Err(ProgramError::InvalidAccountData);
    }

    // Check if meta list is valid and auto-initialize if needed
    let is_valid = is_meta_list_valid(extra_account_meta_list_info, &hook_program_id)?;
    if !is_valid {
        msg!("Meta list is invalid or not initialized, attempting auto-initialization...");
        match auto_initialize_meta_list(&hook_program_id, mint, extra_account_meta_list_info) {
            Ok(_) => {
                msg!("Auto-initialization successful");
            }
            Err(e) => {
                let _ = try_unset_transferring(source);
                return Err(e);
            }
        }
    }

    // Build initial account metas and infos for the hook instruction
    let mut account_metas = vec![
        AccountMeta::new(*source.key, false),
        AccountMeta::new_readonly(*mint.key, false),
        AccountMeta::new(*destination.key, false),
        AccountMeta::new_readonly(*authority.key, false), // authority is readonly for hook
        AccountMeta::new_readonly(extra_account_meta_list_pda, false),
    ];

    let mut account_infos = vec![
        source.clone(),
        mint.clone(),
        destination.clone(),
        authority.clone(),
        extra_account_meta_list_info.clone(),
    ];

    // Parse the ExtraAccountMetaList
    let meta_list_data = match extra_account_meta_list_info.try_borrow_data() {
        Ok(data) => data,
        Err(_) => {
            let _ = try_unset_transferring(source);
            return Err(ProgramError::InvalidAccountData);
        }
    };

    let tlv_state = match TlvStateBorrowed::unpack(&meta_list_data) {
        Ok(state) => state,
        Err(_) => {
            let _ = try_unset_transferring(source);
            return Err(ProgramError::InvalidAccountData);
        }
    };

    // Skip if no extra accounts needed
    let extra_account_meta_list = match ExtraAccountMetaList::unpack_with_tlv_state::<ExecuteInstruction>(&tlv_state) {
        Ok(list) => list,
        Err(_) => {
            // If parsing fails, maybe there are no extra accounts - that's okay
            msg!("No extra account metas found, proceeding with basic accounts");
            
            // Create and execute the hook instruction with basic accounts
            let hook_ix = Instruction {
                program_id: hook_program_id,
                accounts: account_metas,
                data: hook_ix_data,
            };

            let hook_result = solana_program::program::invoke(&hook_ix, &account_infos);
            let _ = try_unset_transferring(source);
            return hook_result;
        }
    };

    // Add extra accounts (starting from index 2, after whitelist and meta list)
    for (i, meta) in extra_account_meta_list.data().iter().enumerate() {
        let acc_info = match remaining_accounts.get(i + 2) {
            Some(info) => info,
            None => {
                let _ = try_unset_transferring(source);
                return Err(ProgramError::NotEnoughAccountKeys);
            }
        };

        // Resolve the account meta using the hook interface
        let resolved_meta = match meta.resolve(&hook_ix_data, &hook_program_id, |index| {
            account_infos.get(index).map(|info| {
                // Determine writability based on the account info
                static WRITABLE_FLAG: [u8; 1] = [1u8];
                let is_writable = if info.is_writable { Some(&WRITABLE_FLAG[..]) } else { None };
                (info.key, is_writable)
            })
        }) {
            Ok(meta) => meta,
            Err(_) => {
                let _ = try_unset_transferring(source);
                return Err(ProgramError::InvalidAccountData);
            }
        };

        account_metas.push(resolved_meta);
        account_infos.push(acc_info.clone());
    }

    // Create and execute the hook instruction
    let hook_ix = Instruction {
        program_id: hook_program_id,
        accounts: account_metas,
        data: hook_ix_data,
    };

    // Execute the hook and handle errors properly
    let hook_result = solana_program::program::invoke(&hook_ix, &account_infos);
    
    // Unset the transferring flag, regardless of hook result
    let _ = try_unset_transferring(source);
    
    // Propagate any hook execution errors
    hook_result?;

    msg!("Transfer hook executed successfully for program: {}", hook_program_id);
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

    /// Issue a spl_token `Transfer` instruction with Token 2022 transfer hook support.
    /// 
    /// For Token 2022 mints with transfer hooks, remaining_accounts should contain:
    /// 0. TransferHookWhitelist PDA account
    /// 1. ExtraAccountMetaList PDA
    /// 2..N. Additional accounts required by the hook (in order)
    pub fn token_transfer<'a>(
        program_id: &Pubkey, // Your AMM program ID
        token_program: AccountInfo<'a>,
        source: AccountInfo<'a>,
        destination: AccountInfo<'a>,
        owner: AccountInfo<'a>,
        deposit_amount: u64,
        mint: AccountInfo<'a>,
        remaining_accounts: &[AccountInfo<'a>],
    ) -> Result<(), ProgramError> {
        // Only execute transfer hook for Token-2022
        if *token_program.key == spl_token_2022::id() {
            execute_transfer_hook(
                program_id,
                &source,
                &mint,
                &destination,
                &owner,
                deposit_amount,
                remaining_accounts,
            )?;
            
            // Get mint decimals
            let mint_data = mint.try_borrow_data()?;
            let mint_info = StateWithExtensions::<Mint>::unpack(&mint_data)?;
            let decimals = mint_info.base.decimals;
            
            // Use transfer_checked for Token-2022
            let transfer_ix = spl_token_2022::instruction::transfer_checked(
                token_program.key,
                source.key,
                mint.key,
                destination.key,
                owner.key,
                &[],
                deposit_amount,
                decimals,
            )?;
            
            solana_program::program::invoke_signed(
                &transfer_ix,
                &[source.clone(), mint.clone(), destination, owner, token_program],
                &[],
            )
        } else {
            // Regular SPL Token transfer (no hooks)
            let transfer_ix = spl_token::instruction::transfer(
                token_program.key,
                source.key,
                destination.key,
                owner.key,
                &[],
                deposit_amount,
            )?;
            
            solana_program::program::invoke_signed(
                &transfer_ix,
                &[source, destination, owner, token_program],
                &[],
            )
        }
    }

    /// Issue a spl_token `Transfer` instruction with authority and Token 2022 transfer hook support.
    /// 
    /// For Token 2022 mints with transfer hooks, remaining_accounts should contain:
    /// 0. TransferHookWhitelist PDA account
    /// 1. ExtraAccountMetaList PDA
    /// 2..N. Additional accounts required by the hook (in order)
    pub fn token_transfer_with_authority<'a>(
        program_id: &Pubkey, // Your AMM program ID
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

        // Only execute transfer hook for Token-2022
        if *token_program.key == spl_token_2022::id() {
            execute_transfer_hook(
                program_id,
                &source,
                &mint,
                &destination,
                &authority,
                amount,
                remaining_accounts,
            )?;
            
            // Get mint decimals
            let mint_data = mint.try_borrow_data()?;
            let mint_info = StateWithExtensions::<Mint>::unpack(&mint_data)?;
            let decimals = mint_info.base.decimals;
            
            // Use transfer_checked for Token-2022
            let transfer_ix = spl_token_2022::instruction::transfer_checked(
                token_program.key,
                source.key,
                mint.key,
                destination.key,
                authority.key,
                &[],
                amount,
                decimals,
            )?;
            
            solana_program::program::invoke_signed(
                &transfer_ix,
                &[source.clone(), mint.clone(), destination, authority, token_program],
                signers,
            )
        } else {
            // Regular SPL Token transfer (no hooks)
            let transfer_ix = spl_token::instruction::transfer(
                token_program.key,
                source.key,
                destination.key,
                authority.key,
                &[],
                amount,
            )?;
            
            solana_program::program::invoke_signed(
                &transfer_ix,
                &[source, destination, authority, token_program],
                signers,
            )
        }
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