use raydium_amm_client::{RaydiumClient, RaydiumConfig, keypair_utils};
use solana_sdk::{pubkey::Pubkey, commitment_config::CommitmentConfig, signature::Signer};
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example 1: Create client with default devnet configuration
    let client = RaydiumClient::new();
    println!("Created client with default devnet config");

    // Example 2: Create client with custom configuration
    let config = RaydiumConfig {
        rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
        commitment: CommitmentConfig::confirmed(),
        amm_program_id: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")?, // mainnet
    };
    let mainnet_client = RaydiumClient::with_config(config);
    println!("Created client with mainnet config");

    // Example 3: Generate and manage keypairs
    let payer_keypair = keypair_utils::generate_keypair();
    println!("Generated keypair: {}", payer_keypair.pubkey());

    // Convert keypair to different formats
    let json_array = keypair_utils::keypair_to_json_array(&payer_keypair);
    let base58 = keypair_utils::keypair_to_base58(&payer_keypair);
    println!("JSON array format: {}", json_array);
    println!("Base58 format: {}", base58);

    // Load keypair from string (useful for web applications)
    let loaded_keypair = keypair_utils::load_keypair_from_string(&json_array)?;
    println!("Loaded keypair matches: {}", loaded_keypair.pubkey() == payer_keypair.pubkey());

    // Example 4: Check if a mint is Token-2022
    let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;
    match client.is_token2022_mint(&usdc_mint).await {
        Ok(is_token2022) => println!("USDC is Token-2022: {}", is_token2022),
        Err(e) => println!("Error checking mint: {}", e),
    }

    // Example 5: Get whitelist information
    match client.get_whitelist_info().await {
        Ok(Some(data)) => println!("Whitelist exists with {} bytes", data.len()),
        Ok(None) => println!("Whitelist does not exist"),
        Err(e) => println!("Error getting whitelist: {}", e),
    }

    // Example 6: Initialize AMM pool (commented out to avoid actual transaction)
    /*
    let coin_mint = Pubkey::from_str("So11111111111111111111111111111111111111112")?; // WSOL
    let pc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;   // USDC

    match client.init_amm_pool(
        coin_mint,
        pc_mint,
        1000000000, // 1 SOL
        1000000,    // 1 USDC
        0,          // nonce
        1673234400, // open time
        &payer_keypair,
    ).await {
        Ok(signature) => println!("Pool initialized: {}", signature),
        Err(e) => println!("Error initializing pool: {}", e),
    }
    */

    // Example 7: Create Token-2022 mint with transfer hook (commented out)
    /*
    let hook_program_id = Pubkey::from_str("HookProgramId11111111111111111111111111111")?;
    
    match client.create_hook_mint(
        hook_program_id,
        9,          // decimals
        1000000000, // initial supply
        &payer_keypair,
    ).await {
        Ok((mint_pubkey, signature)) => {
            println!("Created hook mint: {}", mint_pubkey);
            println!("Transaction: {}", signature);
        },
        Err(e) => println!("Error creating hook mint: {}", e),
    }
    */

    println!("Client usage examples completed!");
    Ok(())
}