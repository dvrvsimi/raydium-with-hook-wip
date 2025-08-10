//! Market utilities for OpenBook DEX integration
//! 
//! This module provides functionality to create and manage OpenBook DEX markets
//! for use with Raydium AMM pools.

use anyhow::{format_err, Result};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    instruction::Instruction,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use solana_program::program_pack::Pack;
use spl_token::instruction as token_instruction;
use serum_dex::state::gen_vault_signer_key;

/// Market account public keys returned after market creation
#[derive(Debug, Clone)]
pub struct MarketPubkeys {
    pub market: Pubkey,
    pub req_q: Pubkey,
    pub event_q: Pubkey,
    pub bids: Pubkey,
    pub asks: Pubkey,
    pub coin_vault: Pubkey,
    pub pc_vault: Pubkey,
    pub vault_signer_key: Pubkey,
}

/// Internal struct to hold keypairs during market creation
struct ListingKeys {
    market_key: Keypair,
    req_q_key: Keypair,
    event_q_key: Keypair,
    bids_key: Keypair,
    asks_key: Keypair,
    vault_signer_pk: Pubkey,
    vault_signer_nonce: u64,
}

/// Detect if a mint is Token-2022
fn is_token2022_mint(client: &RpcClient, mint_pubkey: &Pubkey) -> Result<bool> {
    match client.get_account(mint_pubkey) {
        Ok(account) => Ok(account.owner == spl_token_2022::id()),
        Err(_) => Ok(false),
    }
}

/// Create a token account with proper token program detection
fn create_token_account(
    client: &RpcClient,
    mint_pubkey: &Pubkey,
    owner_pubkey: &Pubkey,
    payer: &Keypair,
) -> Result<Keypair> {
    let spl_account = Keypair::new();
    let signers = vec![payer, &spl_account];

    // Detect which token program to use
    let is_token2022 = is_token2022_mint(client, mint_pubkey)?;
    let token_program_id = if is_token2022 {
        spl_token_2022::id()
    } else {
        spl_token::id()
    };

    // Use appropriate account size - Token-2022 might need more space for extensions
    let account_len = if is_token2022 {
        spl_token_2022::state::Account::LEN
    } else {
        spl_token::state::Account::LEN
    };

    let lamports = client.get_minimum_balance_for_rent_exemption(account_len)?;

    let create_account_instr = solana_sdk::system_instruction::create_account(
        &payer.pubkey(),
        &spl_account.pubkey(),
        lamports,
        account_len as u64,
        &token_program_id, // Use detected token program
    );

    let init_account_instr = if is_token2022 {
        // Use Token-2022 instruction
        spl_token_2022::instruction::initialize_account(
            &token_program_id,
            &spl_account.pubkey(),
            mint_pubkey,
            owner_pubkey,
        )?
    } else {
        // Use regular SPL Token instruction
        token_instruction::initialize_account(
            &token_program_id,
            &spl_account.pubkey(),
            mint_pubkey,
            owner_pubkey,
        )?
    };

    let instructions = vec![create_account_instr, init_account_instr];

    let recent_hash = client.get_latest_blockhash()?;

    let txn = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &signers,
        recent_hash,
    );

    client.send_and_confirm_transaction(&txn)?;
    Ok(spl_account)
}

/// Create a DEX account with proper padding
fn create_dex_account(
    client: &RpcClient,
    program_id: &Pubkey,
    payer: &Pubkey,
    unpadded_len: usize,
) -> Result<(Keypair, Instruction)> {
    let len = unpadded_len + 12; // Add DEX account padding
    let key = Keypair::new();
    let create_account_instr = solana_sdk::system_instruction::create_account(
        payer,
        &key.pubkey(),
        client.get_minimum_balance_for_rent_exemption(len)?,
        len as u64,
        program_id,
    );
    Ok((key, create_account_instr))
}

/// Generate listing parameters and create account instructions
fn gen_listing_params(
    client: &RpcClient,
    program_id: &Pubkey,
    payer: &Pubkey,
    _coin_mint: &Pubkey,
    _pc_mint: &Pubkey,
) -> Result<(ListingKeys, Vec<Instruction>)> {
    // Create all the market component accounts
    let (market_key, create_market) = create_dex_account(client, program_id, payer, 376)?;
    let (req_q_key, create_req_q) = create_dex_account(client, program_id, payer, 640)?;
    let (event_q_key, create_event_q) = create_dex_account(client, program_id, payer, 1 << 20)?;
    let (bids_key, create_bids) = create_dex_account(client, program_id, payer, 1 << 16)?;
    let (asks_key, create_asks) = create_dex_account(client, program_id, payer, 1 << 16)?;
    
    // Generate vault signer PDA
    let (vault_signer_nonce, vault_signer_pk) = {
        let mut i = 0;
        loop {
            assert!(i < 100);
            if let Ok(pk) = gen_vault_signer_key(i, &market_key.pubkey(), program_id) {
                break (i, pk);
            }
            i += 1;
        }
    };
    
    let listing_keys = ListingKeys {
        market_key,
        req_q_key,
        event_q_key,
        bids_key,
        asks_key,
        vault_signer_pk,
        vault_signer_nonce,
    };
    
    let instructions = vec![
        create_market,
        create_req_q,
        create_event_q,
        create_bids,
        create_asks,
    ];
    
    Ok((listing_keys, instructions))
}

/// Create a new OpenBook DEX market for the given token pair
/// 
/// # Arguments
/// * `client` - RPC client for Solana
/// * `program_id` - OpenBook DEX program ID
/// * `payer` - Account that will pay for market creation
/// * `coin_mint` - Base token mint
/// * `pc_mint` - Quote token mint (usually USDC)
/// * `coin_lot_size` - Minimum order size for base token
/// * `pc_lot_size` - Minimum order size for quote token
/// 
/// # Returns
/// Market public keys that can be used for AMM initialization
pub fn create_openbook_market(
    client: &RpcClient,
    program_id: &Pubkey,
    payer: &Keypair,
    coin_mint: &Pubkey,
    pc_mint: &Pubkey,
    coin_lot_size: u64,
    pc_lot_size: u64,
) -> Result<MarketPubkeys> {
    // Generate all the market accounts and instructions
    let (listing_keys, mut instructions) =
        gen_listing_params(client, program_id, &payer.pubkey(), coin_mint, pc_mint)?;
    
    let ListingKeys {
        market_key,
        req_q_key,
        event_q_key,
        bids_key,
        asks_key,
        vault_signer_pk,
        vault_signer_nonce,
    } = listing_keys;

    // Create token vaults for the market - these will now use the correct token programs
    println!("Creating coin vault for mint: {}", coin_mint);
    let coin_vault = create_token_account(client, coin_mint, &vault_signer_pk, payer)?;
    
    println!("Creating PC vault for mint: {}", pc_mint);
    let pc_vault = create_token_account(client, pc_mint, &vault_signer_pk, payer)?;

    println!("DEBUG: Market initialization parameters:");
    println!("  Market Key: {}", market_key.pubkey());
    println!("  Program ID: {}", program_id);
    println!("  Coin Mint: {}", coin_mint);
    println!("  PC Mint: {}", pc_mint);
    println!("  Coin Vault: {}", coin_vault.pubkey());
    println!("  PC Vault: {}", pc_vault.pubkey());
    println!("  Bids: {}", bids_key.pubkey());
    println!("  Asks: {}", asks_key.pubkey());
    println!("  Req Q: {}", req_q_key.pubkey());
    println!("  Event Q: {}", event_q_key.pubkey());
    println!("  Coin Lot Size: {}", coin_lot_size);
    println!("  PC Lot Size: {}", pc_lot_size);
    println!("  Vault Signer Nonce: {}", vault_signer_nonce);
    println!("  Vault Signer PK: {}", vault_signer_pk);
    
    // Initialize the market
    let init_market_instruction = serum_dex::instruction::initialize_market(
        &market_key.pubkey(),
        program_id,
        coin_mint,
        pc_mint,
        &coin_vault.pubkey(),
        &pc_vault.pubkey(),
        None, // no msrm mint
        None, // no bids authority
        None, // no asks authority  
        &bids_key.pubkey(),
        &asks_key.pubkey(),
        &req_q_key.pubkey(),
        &event_q_key.pubkey(),
        coin_lot_size,
        pc_lot_size,
        vault_signer_nonce,
        100, // fees (bps)
    )?;

    instructions.push(init_market_instruction);

    // Create and send transaction
    let recent_hash = client.get_latest_blockhash()?;
    let signers = vec![
        payer,
        &market_key,
        &req_q_key,
        &event_q_key,
        &bids_key,
        &asks_key,
    ];
    
    let txn = Transaction::new_signed_with_payer(
        &instructions,
        Some(&payer.pubkey()),
        &signers,
        recent_hash,
    );

    // Send the transaction
    client.send_and_confirm_transaction(&txn)?;

    Ok(MarketPubkeys {
        market: market_key.pubkey(),
        req_q: req_q_key.pubkey(),
        event_q: event_q_key.pubkey(),
        bids: bids_key.pubkey(),
        asks: asks_key.pubkey(),
        coin_vault: coin_vault.pubkey(),
        pc_vault: pc_vault.pubkey(),
        vault_signer_key: vault_signer_pk,
    })
}