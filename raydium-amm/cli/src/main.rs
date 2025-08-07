use clap::{Parser, Subcommand};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    instruction::Instruction,
};

use spl_associated_token_account::instruction as ata_instruction;
use std::str::FromStr;
use std::fs;
use anyhow::{Result, Context};
use raydium_amm::instruction::{self, AmmInstruction};
use spl_token_2022::{
    extension::StateWithExtensions,
    state::Mint,
};
use spl_token::solana_program::program_option::COption;
use spl_tlv_account_resolution::{
    account::ExtraAccountMeta,
};
use bytemuck;



#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a Raydium AMM pool with Token-2022 support
    InitPool {
        /// AMM program ID
        #[arg(long, default_value = "3bTCD4MnbUsi6Ad1dqotiBQtiPbzKJbFmzkqQz8A1kag")] // devnet
        amm_program_id: String,
        /// Coin mint address (Token-2022 supported)
        #[arg(long)]
        coin_mint: String,
        /// PC mint address (usually USDC)
        #[arg(long)]
        pc_mint: String,
        /// Initial coin amount
        #[arg(long)]
        init_coin_amount: u64,
        /// Initial PC amount
        #[arg(long)]
        init_pc_amount: u64,
        /// Nonce for AMM authority
        #[arg(long, default_value = "0")]
        nonce: u8,
        /// Open time (Unix timestamp)
        #[arg(long)]
        open_time: u64,
        /// Path to payer keypair file
        #[arg(long, default_value = "~/.config/solana/id.json")]
        payer: String,
    },
    /// Initialize whitelist for transfer hook
    InitWhitelist {
        /// AMM program ID
        #[arg(long, default_value = "3bTCD4MnbUsi6Ad1dqotiBQtiPbzKJbFmzkqQz8A1kag")] // devnet
        amm_program_id: String,
        /// Path to payer keypair file
        #[arg(long, default_value = "~/.config/solana/id.json")]
        payer: String,
    },
    /// Add hook to whitelist
    AddHookToWhitelist {
        /// AMM program ID
        #[arg(long, default_value = "3bTCD4MnbUsi6Ad1dqotiBQtiPbzKJbFmzkqQz8A1kag")]
        amm_program_id: String,
        /// Hook program ID to add
        #[arg(long)]
        hook_program_id: String,
        /// Path to payer keypair file
        #[arg(long, default_value = "~/.config/solana/id.json")]
        payer: String,
    },
    /// Remove hook from whitelist
    RemoveHookFromWhitelist {
        /// AMM program ID
        #[arg(long, default_value = "3bTCD4MnbUsi6Ad1dqotiBQtiPbzKJbFmzkqQz8A1kag")]
        amm_program_id: String,
        /// Hook program ID to remove
        #[arg(long)]
        hook_program_id: String,
        /// Path to payer keypair file
        #[arg(long, default_value = "~/.config/solana/id.json")]
        payer: String,
    },
    /// Get whitelist info
    GetWhitelistInfo {
        /// AMM program ID
        #[arg(long, default_value = "3bTCD4MnbUsi6Ad1dqotiBQtiPbzKJbFmzkqQz8A1kag")]
        amm_program_id: String,
    },
    /// Create a Token-2022 mint with transfer hook
    CreateHookMint {
        /// Transfer hook program ID
        #[arg(long)]
        hook_program_id: String,
        /// Mint decimals
        #[arg(long, default_value = "9")]
        decimals: u8,
        /// Initial supply to mint to payer
        #[arg(long, default_value = "1000000000")]
        initial_supply: u64,
        /// Path to payer keypair file
        #[arg(long, default_value = "~/.config/solana/id.json")]
        payer: String,
    },
    /// Initialize the transfer hook's extra account meta list
    InitHookMetaList {
        /// Transfer hook program ID
        #[arg(long)]
        hook_program_id: String,
        /// Mint address with transfer hook
        #[arg(long)]
        mint: String,
        /// Path to payer keypair file
        #[arg(long, default_value = "~/.config/solana/id.json")]
        payer: String,
    },
    /// Initialize whitelist for a transfer hook
    InitHookWhitelist {
        /// Transfer hook program ID
        #[arg(long)]
        hook_program_id: String,
        /// Path to payer keypair file
        #[arg(long, default_value = "~/.config/solana/id.json")]
        payer: String,
    },
    /// Add user to transfer hook whitelist
    AddToHookWhitelist {
        /// Transfer hook program ID
        #[arg(long)]
        hook_program_id: String,
        /// User pubkey to add
        #[arg(long)]
        user: String,
        /// Path to payer keypair file
        #[arg(long, default_value = "~/.config/solana/id.json")]
        payer: String,
    },
    /// Test transfer with hook (should succeed if whitelisted)
    TestHookTransfer {
        /// Mint address with transfer hook
        #[arg(long)]
        mint: String,
        /// Source token account
        #[arg(long)]
        source: String,
        /// Destination token account  
        #[arg(long)]
        destination: String,
        /// Transfer amount
        #[arg(long)]
        amount: u64,
        /// Path to owner keypair file
        #[arg(long, default_value = "~/.config/solana/id.json")]
        owner: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let rpc_url = "https://api.devnet.solana.com".to_string();
    let rpc_client = RpcClient::new_with_commitment(
        rpc_url,
        CommitmentConfig::confirmed(),
    );

    match &cli.command {
        Commands::InitPool { 
            amm_program_id,
            coin_mint, 
            pc_mint, 
            init_coin_amount, 
            init_pc_amount,
            nonce,
            open_time,
            payer
        } => {
            let payer_keypair = load_keypair(payer)?;
            init_amm_pool(
                &rpc_client, 
                amm_program_id.clone(),
                coin_mint.clone(), 
                pc_mint.clone(), 
                *init_coin_amount, 
                *init_pc_amount,
                *nonce,
                *open_time,
                &payer_keypair
            ).await?;
        }
        Commands::InitWhitelist { amm_program_id, payer } => {
            let payer_keypair = load_keypair(payer)?;
            init_whitelist(
                &rpc_client,
                amm_program_id.clone(),
                &payer_keypair,
            ).await?;
        }
        Commands::AddHookToWhitelist { amm_program_id, hook_program_id, payer } => {
            let payer_keypair = load_keypair(payer)?;
            add_hook_to_whitelist(
                &rpc_client,
                amm_program_id.clone(),
                hook_program_id.clone(),
                &payer_keypair,
            ).await?;
        }
        Commands::RemoveHookFromWhitelist { amm_program_id, hook_program_id, payer } => {
            let payer_keypair = load_keypair(payer)?;
            remove_hook_from_whitelist(
                &rpc_client,
                amm_program_id.clone(),
                hook_program_id.clone(),
                &payer_keypair,
            ).await?;
        }
        Commands::GetWhitelistInfo { amm_program_id } => {
            get_whitelist_info(
                &rpc_client,
                amm_program_id.clone(),
            ).await?;
        }
        Commands::CreateHookMint { hook_program_id, decimals, initial_supply, payer } => {
            let payer_keypair = load_keypair(payer)?;
            create_hook_mint(
                &rpc_client,
                hook_program_id.clone(),
                *decimals,
                *initial_supply,
                &payer_keypair,
            ).await?;
        }
        Commands::InitHookMetaList { hook_program_id, mint, payer } => {
            let payer_keypair = load_keypair(payer)?;
            init_hook_meta_list(
                &rpc_client,
                hook_program_id.clone(),
                mint.clone(),
                &payer_keypair,
            ).await?;
        }
        Commands::InitHookWhitelist { hook_program_id, payer } => {
            let payer_keypair = load_keypair(payer)?;
            init_hook_whitelist(
                &rpc_client,
                hook_program_id.clone(),
                &payer_keypair,
            ).await?;
        }
        Commands::AddToHookWhitelist { hook_program_id, user, payer } => {
            let payer_keypair = load_keypair(payer)?;
            add_to_hook_whitelist(
                &rpc_client,
                hook_program_id.clone(),
                user.clone(),
                &payer_keypair,
            ).await?;
        }
        Commands::TestHookTransfer { mint, source, destination, amount, owner } => {
            let owner_keypair = load_keypair(owner)?;
            test_hook_transfer(
                &rpc_client,
                mint.clone(),
                source.clone(),
                destination.clone(),
                *amount,
                &owner_keypair,
            ).await?;
        }
    }

    Ok(())
}

// Helper function to load keypair from file with improved error handling and format support
fn load_keypair(path: &str) -> Result<Keypair> {
    let expanded_path = shellexpand::tilde(path);
    let keypair_data = fs::read_to_string(expanded_path.as_ref())
        .with_context(|| format!("Failed to read keypair file: {}", path))?;
    
    // Try different keypair formats
    let keypair = if keypair_data.trim().starts_with('[') {
        // JSON array format (most common)
        let keypair_bytes: Vec<u8> = serde_json::from_str(&keypair_data)
            .with_context(|| "Failed to parse keypair JSON array")?;
        
        Keypair::try_from(keypair_bytes.as_slice())
            .with_context(|| "Invalid keypair bytes")?
    } else if keypair_data.trim().starts_with('"') {
        // Base58 encoded string format
        let decoded = bs58::decode(keypair_data.trim_matches('"'))
            .into_vec()
            .with_context(|| "Failed to decode base58 keypair")?;
        
        Keypair::try_from(decoded.as_slice())
            .with_context(|| "Invalid keypair bytes from base58")?
    } else {
        // Try to parse as JSON object with "privateKey" field
        #[derive(serde::Deserialize)]
        struct KeypairFile {
            private_key: Vec<u8>,
        }
        
        let keypair_file: KeypairFile = serde_json::from_str(&keypair_data)
            .with_context(|| "Failed to parse keypair JSON object")?;
        
        Keypair::try_from(keypair_file.private_key.as_slice())
            .with_context(|| "Invalid keypair bytes from JSON object")?
    };
    
    // Validate the keypair
    if keypair.pubkey() == Pubkey::default() {
        return Err(anyhow::anyhow!("Invalid keypair: public key is zero"));
    }
    
    Ok(keypair)
}

async fn init_amm_pool(
    rpc_client: &RpcClient,
    amm_program_id: String,
    coin_mint: String,
    pc_mint: String,
    init_coin_amount: u64,
    init_pc_amount: u64,
    nonce: u8,
    open_time: u64,
    payer: &Keypair,
) -> Result<()> {
    println!("Initializing Raydium AMM pool...");
    println!("  AMM Program ID: {}", amm_program_id);
    println!("  Coin Mint: {}", coin_mint);
    println!("  PC Mint: {}", pc_mint);
    println!("  Initial Coin Amount: {}", init_coin_amount);
    println!("  Initial PC Amount: {}", init_pc_amount);
    println!("  Nonce: {}", nonce);
    println!("  Open Time: {}", open_time);
    println!("  Payer: {}", payer.pubkey());
    
    // Generate keypairs for the pool
    let amm_pool_keypair = Keypair::new();
    let amm_authority_keypair = Keypair::new();
    let amm_open_orders_keypair = Keypair::new();
    let amm_target_orders_keypair = Keypair::new();
    let amm_coin_vault_keypair = Keypair::new();
    let amm_pc_vault_keypair = Keypair::new();
    let amm_lp_mint_keypair = Keypair::new();
    let pool_withdraw_queue_keypair = Keypair::new();
    let lp_withdraw_queue_keypair = Keypair::new();
    
    // Parse pubkeys
    let amm_program_pubkey = Pubkey::from_str(&amm_program_id)?;
    let coin_mint_pubkey = Pubkey::from_str(&coin_mint)?;
    let pc_mint_pubkey = Pubkey::from_str(&pc_mint)?;
    
    // Check if payer has enough SOL
    let payer_balance = rpc_client.get_balance(&payer.pubkey())?;
    if payer_balance < 2_000_000_000 {
        println!("Airdropping SOL to payer...");
        let signature = rpc_client.request_airdrop(&payer.pubkey(), 2_000_000_000)?;
        rpc_client.confirm_transaction(&signature)?;
        println!("Airdrop confirmed: {}", signature);
    }
    
    // Create associated token accounts for payer
    let payer_coin_ata = spl_associated_token_account::get_associated_token_address(
        &payer.pubkey(),
        &coin_mint_pubkey,
    );
    let payer_pc_ata = spl_associated_token_account::get_associated_token_address(
        &payer.pubkey(),
        &pc_mint_pubkey,
    );
    let payer_lp_ata = spl_associated_token_account::get_associated_token_address(
        &payer.pubkey(),
        &amm_lp_mint_keypair.pubkey(),
    );
    
    // Create AMM vault ATAs
    let amm_coin_vault_ata = spl_associated_token_account::get_associated_token_address(
        &amm_coin_vault_keypair.pubkey(),
        &coin_mint_pubkey,
    );
    let amm_pc_vault_ata = spl_associated_token_account::get_associated_token_address(
        &amm_pc_vault_keypair.pubkey(),
        &pc_mint_pubkey,
    );
    
    let mut instructions = vec![];
    
    // Create ATAs for payer
    instructions.push(ata_instruction::create_associated_token_account(
        &payer.pubkey(),
        &payer.pubkey(),
        &coin_mint_pubkey,
        &spl_token::id(),
    ));
    
    instructions.push(ata_instruction::create_associated_token_account(
        &payer.pubkey(),
        &payer.pubkey(),
        &pc_mint_pubkey,
        &spl_token::id(),
    ));
    
    instructions.push(ata_instruction::create_associated_token_account(
        &payer.pubkey(),
        &payer.pubkey(),
        &amm_lp_mint_keypair.pubkey(),
        &spl_token::id(),
    ));
    
    // Create AMM accounts
    let amm_account_size = 752; // Size of AmmInfo struct
    let amm_lamports = rpc_client.get_minimum_balance_for_rent_exemption(amm_account_size)?;
    
    instructions.push(solana_system_interface::instruction::create_account(
        &payer.pubkey(),
        &amm_pool_keypair.pubkey(),
        amm_lamports,
        amm_account_size as u64,
        &amm_program_pubkey,
    ));
    
    // Create initialize instruction using actual instruction builder
    let init_instruction = instruction::initialize2(
        &amm_program_pubkey,
        &amm_pool_keypair.pubkey(),
        &amm_authority_keypair.pubkey(),
        &amm_open_orders_keypair.pubkey(),
        &amm_lp_mint_keypair.pubkey(),
        &coin_mint_pubkey,
        &pc_mint_pubkey,
        &amm_coin_vault_ata,
        &amm_pc_vault_ata,
        &amm_target_orders_keypair.pubkey(),
        &Pubkey::default(), // config
        &Pubkey::default(), // create_fee_destination
        &Pubkey::default(), // market_program
        &Pubkey::default(), // market
        &payer.pubkey(),
        &payer_coin_ata,
        &payer_pc_ata,
        &payer_lp_ata,
        nonce,
        open_time,
        init_pc_amount,
        init_coin_amount,
    )?;
    
    instructions.push(init_instruction);
    
    // Create and send transaction
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let mut transaction = Transaction::new_with_payer(
        &instructions,
        Some(&payer.pubkey()),
    );
    
    transaction.sign(
        &[payer, &amm_pool_keypair, &amm_authority_keypair, 
          &amm_open_orders_keypair, &amm_target_orders_keypair, &amm_coin_vault_keypair,
          &amm_pc_vault_keypair, &amm_lp_mint_keypair, &pool_withdraw_queue_keypair,
          &lp_withdraw_queue_keypair],
        recent_blockhash
    );
    
    println!("Sending initialization transaction...");
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
    println!("Pool initialized successfully!");
    println!("  Pool Address: {}", amm_pool_keypair.pubkey());
    println!("  Transaction: {}", signature);
    println!("  Explorer: https://explorer.solana.com/tx/{}?cluster=devnet", signature);
    
    Ok(())
}

async fn init_whitelist(
    rpc_client: &RpcClient,
    amm_program_id: String,
    payer: &Keypair,
) -> Result<()> {
    println!("Initializing whitelist...");
    println!("  AMM Program ID: {}", amm_program_id);
    println!("  Payer: {}", payer.pubkey());

    let amm_program_pubkey = Pubkey::from_str(&amm_program_id)?;
    
    // Create whitelist PDA
    let (whitelist_pda, _bump) = Pubkey::find_program_address(
        &[b"hook_whitelist"],
        &amm_program_pubkey,
    );
    println!("  Whitelist PDA: {}", whitelist_pda);

    // Create initialize whitelist instruction using actual instruction builder
    let init_instruction = create_initialize_whitelist_instruction(
        &amm_program_pubkey,
        &payer.pubkey(),
        &whitelist_pda,
    )?;

    let transaction = Transaction::new_signed_with_payer(
        &[init_instruction],
        Some(&payer.pubkey()),
        &[payer],
        rpc_client.get_latest_blockhash()?,
    );

    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
    println!("Whitelist initialized successfully!");
    println!("  Transaction Signature: {}", signature);
    println!("  Explorer: https://explorer.solana.com/tx/{}?cluster=devnet", signature);

    Ok(())
}

async fn add_hook_to_whitelist(
    rpc_client: &RpcClient,
    amm_program_id: String,
    hook_program_id: String,
    payer: &Keypair,
) -> Result<()> {
    println!("Adding hook to whitelist: {}", hook_program_id);
    println!("  AMM Program ID: {}", amm_program_id);
    println!("  Payer: {}", payer.pubkey());

    let amm_program_pubkey = Pubkey::from_str(&amm_program_id)?;
    let hook_program_pubkey = Pubkey::from_str(&hook_program_id)?;
    let (whitelist_pda, _bump) = Pubkey::find_program_address(
        &[b"hook_whitelist"],
        &amm_program_pubkey,
    );

    let instruction = create_update_whitelist_instruction(
        &amm_program_pubkey,
        &payer.pubkey(),
        &whitelist_pda,
        &hook_program_pubkey,
        true, // Add hook
    )?;

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[payer],
        rpc_client.get_latest_blockhash()?,
    );

    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
    println!("Hook added to whitelist successfully!");
    println!("  Transaction Signature: {}", signature);
    println!("  Explorer: https://explorer.solana.com/tx/{}?cluster=devnet", signature);

    Ok(())
}

async fn remove_hook_from_whitelist(
    rpc_client: &RpcClient,
    amm_program_id: String,
    hook_program_id: String,
    payer: &Keypair,
) -> Result<()> {
    println!("Removing hook from whitelist: {}", hook_program_id);
    println!("  AMM Program ID: {}", amm_program_id);
    println!("  Payer: {}", payer.pubkey());

    let amm_program_pubkey = Pubkey::from_str(&amm_program_id)?;
    let hook_program_pubkey = Pubkey::from_str(&hook_program_id)?;
    let (whitelist_pda, _bump) = Pubkey::find_program_address(
        &[b"hook_whitelist"],
        &amm_program_pubkey,
    );

    let instruction = create_update_whitelist_instruction(
        &amm_program_pubkey,
        &payer.pubkey(),
        &whitelist_pda,
        &hook_program_pubkey,
        false, // Remove hook
    )?;

    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[payer],
        rpc_client.get_latest_blockhash()?,
    );

    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
    println!("Hook removed from whitelist successfully!");
    println!("  Transaction Signature: {}", signature);
    println!("  Explorer: https://explorer.solana.com/tx/{}?cluster=devnet", signature);

    Ok(())
}

async fn get_whitelist_info(
    rpc_client: &RpcClient,
    amm_program_id: String,
) -> Result<()> {
    println!("Getting whitelist info...");
    println!("  AMM Program ID: {}", amm_program_id);

    let amm_program_pubkey = Pubkey::from_str(&amm_program_id)?;
    let (whitelist_pda, _bump) = Pubkey::find_program_address(
        &[b"hook_whitelist"],
        &amm_program_pubkey,
    );
    println!("  Whitelist PDA: {}", whitelist_pda);

    match rpc_client.get_account(&whitelist_pda) {
        Ok(account) => {
            println!("Whitelist account exists");
            println!("  Account size: {} bytes", account.data.len());
            if !account.data.is_empty() {
                println!("  Whitelist is initialized");
                // You could deserialize the data here to show whitelist contents
            } else {
                println!("  Whitelist is empty");
            }
        }
        Err(_) => {
            println!("Whitelist account does not exist");
        }
    }

    Ok(())
}

// Transfer hook testing functions

async fn create_hook_mint(
    rpc_client: &RpcClient,
    hook_program_id: String,
    decimals: u8,
    initial_supply: u64,
    payer: &Keypair,
) -> Result<()> {
    println!("Creating Token-2022 mint with transfer hook...");
    
    let hook_program_pubkey = Pubkey::from_str(&hook_program_id)?;
    let mint_keypair = Keypair::new();
    
    // Create mint account with transfer hook extension
    let mint_size = 82; // Standard mint size with transfer hook extension
    
    let mint_lamports = rpc_client.get_minimum_balance_for_rent_exemption(mint_size)?;
    
    let mut instructions = vec![];
    
    // Create mint account
    instructions.push(solana_system_interface::instruction::create_account(
        &payer.pubkey(),
        &mint_keypair.pubkey(),
        mint_lamports,
        mint_size as u64,
        &spl_token_2022::id(),
    ));
    
    // Initialize transfer hook extension
    instructions.push(
        spl_token_2022::extension::transfer_hook::instruction::initialize(
            &spl_token_2022::id(),
            &mint_keypair.pubkey(),
            Some(payer.pubkey()),
            Some(hook_program_pubkey),
        )?
    );
    
    // Initialize mint
    instructions.push(
        spl_token_2022::instruction::initialize_mint2(
            &spl_token_2022::id(),
            &mint_keypair.pubkey(),
            &payer.pubkey(),
            Some(&payer.pubkey()),
            decimals,
        )?
    );
    
    // Create ATA for payer and mint initial supply
    let payer_ata = spl_associated_token_account::get_associated_token_address_with_program_id(
        &payer.pubkey(),
        &mint_keypair.pubkey(),
        &spl_token_2022::id(),
    );
    
    instructions.push(
        spl_associated_token_account::instruction::create_associated_token_account(
            &payer.pubkey(),
            &payer.pubkey(),
            &mint_keypair.pubkey(),
            &spl_token_2022::id(),
        )
    );
    
    instructions.push(
        spl_token_2022::instruction::mint_to(
            &spl_token_2022::id(),
            &mint_keypair.pubkey(),
            &payer_ata,
            &payer.pubkey(),
            &[],
            initial_supply,
        )?
    );
    
    // Send transaction
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let mut transaction = Transaction::new_with_payer(&instructions, Some(&payer.pubkey()));
    transaction.sign(&[payer, &mint_keypair], recent_blockhash);
    
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
    
    println!("Token-2022 mint with transfer hook created successfully!");
    println!("  Mint Address: {}", mint_keypair.pubkey());
    println!("  Hook Program: {}", hook_program_pubkey);
    println!("  Payer ATA: {}", payer_ata);
    println!("  Transaction: {}", signature);
    println!("  Explorer: https://explorer.solana.com/tx/{}?cluster=devnet", signature);
    
    Ok(())
}

async fn init_hook_meta_list(
    rpc_client: &RpcClient,
    hook_program_id: String,
    mint: String,
    payer: &Keypair,
) -> Result<()> {
    println!("Initializing transfer hook meta list...");
    println!("  Hook Program ID: {}", hook_program_id);
    println!("  Mint: {}", mint);
    
    let hook_program_pubkey = Pubkey::from_str(&hook_program_id)?;
    let mint_pubkey = Pubkey::from_str(&mint)?;
    
    // Create ExtraAccountMetaList PDA for the hook program
    let (meta_list_pda, _bump) = Pubkey::find_program_address(
        &[b"extra-account-metas", mint_pubkey.as_ref()],
        &hook_program_pubkey,
    );
    
    // Create whitelist PDA for the hook program
    let (whitelist_pda, _whitelist_bump) = Pubkey::find_program_address(
        &[b"whitelist"],
        &hook_program_pubkey,
    );
    
    // Define the extra account metas that the hook program needs
    let extra_account_metas = vec![
        spl_tlv_account_resolution::account::ExtraAccountMeta::new_with_seeds(
            &[
                spl_tlv_account_resolution::seeds::Seed::Literal {
                    bytes: b"whitelist".to_vec(),
                },
            ],
            false, // is_signer
            false, // is_writable
        )?
    ];
    
    // Create instruction to call the hook program's InitializeExtraAccountMetaList instruction
    let instruction = create_hook_initialize_meta_list_instruction(
        &hook_program_pubkey,
        &meta_list_pda,
        &mint_pubkey,
        &payer.pubkey(),
        extra_account_metas,
    )?;
    
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[payer],
        rpc_client.get_latest_blockhash()?,
    );
    
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
    println!("Transfer hook meta list initialized successfully!");
    println!("  Meta List PDA: {}", meta_list_pda);
    println!("  Whitelist PDA: {}", whitelist_pda);
    println!("  Transaction: {}", signature);
    
    Ok(())
}

async fn init_hook_whitelist(
    rpc_client: &RpcClient,
    hook_program_id: String,
    payer: &Keypair,
) -> Result<()> {
    println!("Initializing transfer hook whitelist...");
    println!("  Hook Program ID: {}", hook_program_id);
    
    let hook_program_pubkey = Pubkey::from_str(&hook_program_id)?;
    
    // Create whitelist PDA
    let (whitelist_pda, _bump) = Pubkey::find_program_address(
        &[b"whitelist"],
        &hook_program_pubkey,
    );
    
    // Create instruction to call the hook program's initialize whitelist instruction
    let instruction = create_hook_initialize_whitelist_instruction(
        &hook_program_pubkey,
        &payer.pubkey(),
        &whitelist_pda,
    )?;
    
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[payer],
        rpc_client.get_latest_blockhash()?,
    );
    
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
    println!("Transfer hook whitelist initialized successfully!");
    println!("  Transaction: {}", signature);
    
    Ok(())
}

async fn add_to_hook_whitelist(
    rpc_client: &RpcClient,
    hook_program_id: String,
    user: String,
    payer: &Keypair,
) -> Result<()> {
    println!("Adding user to transfer hook whitelist: {}", user);
    
    let hook_program_pubkey = Pubkey::from_str(&hook_program_id)?;
    let user_pubkey = Pubkey::from_str(&user)?;
    
    let (whitelist_pda, _bump) = Pubkey::find_program_address(
        &[b"whitelist"],
        &hook_program_pubkey,
    );
    
    // Create instruction to call the hook program's add to whitelist instruction
    let instruction = create_hook_add_to_whitelist_instruction(
        &hook_program_pubkey,
        &payer.pubkey(),
        &whitelist_pda,
        &user_pubkey,
    )?;
    
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&payer.pubkey()),
        &[payer],
        rpc_client.get_latest_blockhash()?,
    );
    
    let signature = rpc_client.send_and_confirm_transaction(&transaction)?;
    println!("User added to transfer hook whitelist successfully!");
    println!("  Transaction: {}", signature);
    
    Ok(())
}

async fn test_hook_transfer(
    rpc_client: &RpcClient,
    mint: String,
    source: String,
    destination: String,
    amount: u64,
    owner: &Keypair,
) -> Result<()> {
    println!("Testing transfer with hook validation...");
    println!("  Mint: {}", mint);
    println!("  Source: {}", source);
    println!("  Destination: {}", destination);
    println!("  Amount: {}", amount);
    
    let mint_pubkey = Pubkey::from_str(&mint)?;
    let source_pubkey = Pubkey::from_str(&source)?;
    let destination_pubkey = Pubkey::from_str(&destination)?;
    
    // Get mint info to find transfer hook program
    let mint_account = rpc_client.get_account(&mint_pubkey)?;
    let mint_info = StateWithExtensions::<Mint>::unpack(&mint_account.data).map(|state| state.base).unwrap_or_else(|_| {
        // Fallback to default mint info if unpack fails
        Mint {
            mint_authority: COption::Some(owner.pubkey()),
            supply: 0,
            decimals: 9,
            is_initialized: true,
            freeze_authority: COption::None,
        }
    });
    
    // 
    let transfer_instruction = spl_token_2022::instruction::transfer_checked(
        &spl_token_2022::id(),
        &source_pubkey,
        &mint_pubkey,
        &destination_pubkey,
        &owner.pubkey(),
        &[],
        amount,
        mint_info.decimals,
    )?;
    
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let mut transaction = Transaction::new_with_payer(
        &[transfer_instruction],
        Some(&owner.pubkey()),
    );
    transaction.sign(&[owner], recent_blockhash);
    
    match rpc_client.send_and_confirm_transaction(&transaction) {
        Ok(signature) => {
            println!("✅ Transfer succeeded - user is whitelisted!");
            println!("  Transaction: {}", signature);
            println!("  Explorer: https://explorer.solana.com/tx/{}?cluster=devnet", signature);
        }
        Err(e) => {
            println!("❌ Transfer failed - user may not be whitelisted or other error:");
            println!("  Error: {}", e);
        }
    }
    
    Ok(())
}

// Helper functions for transfer hook instructions

fn create_hook_initialize_meta_list_instruction(
    hook_program_id: &Pubkey,
    meta_list_pda: &Pubkey,
    mint: &Pubkey,
    authority: &Pubkey,
    extra_account_metas: Vec<ExtraAccountMeta>,
) -> Result<Instruction> {
    // Create instruction to call the hook program's InitializeExtraAccountMetaList instruction
    // This uses the SPL Transfer Hook Interface discriminator
    let mut data = vec![
        // SPL Transfer Hook Interface discriminator for InitializeExtraAccountMetaList
        43, 34, 13, 49, 167, 88, 235, 235
    ];
    
    // Add the extra account metas using proper serialization
    for meta in extra_account_metas {
        // Use bytemuck to serialize ExtraAccountMeta (which implements Pod)
        let meta_bytes = bytemuck::bytes_of(&meta);
        data.extend_from_slice(meta_bytes);
    }
    
    let accounts = vec![
        solana_sdk::instruction::AccountMeta::new(*meta_list_pda, false),
        solana_sdk::instruction::AccountMeta::new_readonly(*mint, false),
        solana_sdk::instruction::AccountMeta::new_readonly(*authority, true),
        solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];

    Ok(Instruction {
        program_id: *hook_program_id,
        accounts,
        data,
    })
}

fn create_hook_initialize_whitelist_instruction(
    hook_program_id: &Pubkey,
    authority: &Pubkey,
    whitelist_pda: &Pubkey,
) -> Result<Instruction> {
    // Create instruction to call the hook program's initialize whitelist instruction
    // This uses the hook program's custom instruction format
    let mut data = vec![0]; // Initialize whitelist instruction discriminator
    data.extend_from_slice(&authority.to_bytes());
    
    let accounts = vec![
        solana_sdk::instruction::AccountMeta::new(*whitelist_pda, false),
        solana_sdk::instruction::AccountMeta::new_readonly(*authority, true),
        solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];

    Ok(Instruction {
        program_id: *hook_program_id,
        accounts,
        data,
    })
}

fn create_hook_add_to_whitelist_instruction(
    hook_program_id: &Pubkey,
    authority: &Pubkey,
    whitelist_pda: &Pubkey,
    user: &Pubkey,
) -> Result<Instruction> {
    // Create instruction to call the hook program's add to whitelist instruction
    // This uses the hook program's custom instruction format
    let mut data = vec![1]; // Add to whitelist instruction discriminator
    data.extend_from_slice(&user.to_bytes());
    
    let accounts = vec![
        solana_sdk::instruction::AccountMeta::new(*whitelist_pda, false),
        solana_sdk::instruction::AccountMeta::new_readonly(*authority, true),
    ];

    Ok(Instruction {
        program_id: *hook_program_id,
        accounts,
        data,
    })
}

// Helper functions using actual instruction builders

fn create_initialize_whitelist_instruction(
    program_id: &Pubkey,
    authority: &Pubkey,
    whitelist_pda: &Pubkey,
) -> Result<Instruction> {
    // Use instruction enum from crate
    let instruction_data = AmmInstruction::InitializeHookWhitelist { 
        authority: *authority 
    };
    
    let accounts = vec![
        solana_sdk::instruction::AccountMeta::new(*whitelist_pda, false),
        solana_sdk::instruction::AccountMeta::new_readonly(*authority, true),
        solana_sdk::instruction::AccountMeta::new_readonly(solana_sdk::system_program::id(), false),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: instruction_data.pack()?,
    })
}

fn create_update_whitelist_instruction(
    program_id: &Pubkey,
    authority: &Pubkey,
    whitelist_pda: &Pubkey,
    hook_program_id: &Pubkey,
    add: bool,
) -> Result<Instruction> {
    // Use the actual instruction enum from your crate
    let action = if add { 
        raydium_amm::instruction::HookWhitelistAction::Add 
    } else { 
        raydium_amm::instruction::HookWhitelistAction::Remove 
    };
    
    let instruction_data = AmmInstruction::UpdateHookWhitelist(
        raydium_amm::instruction::UpdateHookWhitelistInstruction {
            hook_program_id: *hook_program_id,
            action,
        }
    );
    
    let accounts = vec![
        solana_sdk::instruction::AccountMeta::new(*whitelist_pda, false),
        solana_sdk::instruction::AccountMeta::new_readonly(*authority, true),
    ];

    Ok(Instruction {
        program_id: *program_id,
        accounts,
        data: instruction_data.pack()?,
    })
}