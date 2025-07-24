use clap::{Parser, Subcommand};
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    commitment_config::CommitmentConfig,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
    instruction::Instruction,
    system_instruction,
};
use spl_token::instruction as token_instruction;
use spl_associated_token_account::instruction as ata_instruction;
use std::str::FromStr;
use anyhow::Result;
use raydium_amm::instruction::{self, AmmInstruction, InitializeInstruction2};

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
        #[arg(long, default_value = "675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")]
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
    },
    /// Deposit liquidity to an existing AMM pool
    Deposit {
        /// AMM pool address
        #[arg(long)]
        pool_address: String,
        /// Coin amount to deposit
        #[arg(long)]
        coin_amount: u64,
        /// PC amount to deposit
        #[arg(long)]
        pc_amount: u64,
        /// Minimum LP tokens to receive
        #[arg(long)]
        min_lp_amount: u64,
    },
    /// Withdraw liquidity from an AMM pool
    Withdraw {
        /// AMM pool address
        #[arg(long)]
        pool_address: String,
        /// LP token amount to burn
        #[arg(long)]
        lp_amount: u64,
        /// Minimum coin amount to receive
        #[arg(long)]
        min_coin_amount: Option<u64>,
        /// Minimum PC amount to receive
        #[arg(long)]
        min_pc_amount: Option<u64>,
    },
    /// Swap tokens on an AMM pool
    Swap {
        /// AMM pool address
        #[arg(long)]
        pool_address: String,
        /// Amount to swap (in base units)
        #[arg(long)]
        amount_in: u64,
        /// Minimum amount out (in base units)
        #[arg(long)]
        min_amount_out: u64,
        /// Swap base in (true) or base out (false)
        #[arg(long, default_value = "true")]
        base_in: bool,
    },
    /// Get pool information
    PoolInfo {
        /// AMM pool address
        #[arg(long)]
        pool_address: String,
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
            open_time
        } => {
            init_amm_pool(
                &rpc_client, 
                amm_program_id.clone(),
                coin_mint.clone(), 
                pc_mint.clone(), 
                *init_coin_amount, 
                *init_pc_amount,
                *nonce,
                *open_time
            ).await?;
        }
        Commands::Deposit { 
            pool_address, 
            coin_amount, 
            pc_amount, 
            min_lp_amount 
        } => {
            deposit_liquidity(
                &rpc_client, 
                pool_address.clone(), 
                *coin_amount, 
                *pc_amount, 
                *min_lp_amount
            ).await?;
        }
        Commands::Withdraw { 
            pool_address, 
            lp_amount, 
            min_coin_amount, 
            min_pc_amount 
        } => {
            withdraw_liquidity(
                &rpc_client, 
                pool_address.clone(), 
                *lp_amount, 
                *min_coin_amount, 
                *min_pc_amount
            ).await?;
        }
        Commands::Swap { 
            pool_address, 
            amount_in, 
            min_amount_out, 
            base_in 
        } => {
            swap_tokens(
                &rpc_client, 
                pool_address.clone(), 
                *amount_in, 
                *min_amount_out, 
                *base_in
            ).await?;
        }
        Commands::PoolInfo { 
            pool_address 
        } => {
            get_pool_info(
                &rpc_client, 
                pool_address.clone()
            ).await?;
        }
    }

    Ok(())
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
) -> Result<()> {
    println!("Initializing Raydium AMM pool...");
    println!("  AMM Program ID: {}", amm_program_id);
    println!("  Coin Mint: {}", coin_mint);
    println!("  PC Mint: {}", pc_mint);
    println!("  Initial Coin Amount: {}", init_coin_amount);
    println!("  Initial PC Amount: {}", init_pc_amount);
    println!("  Nonce: {}", nonce);
    println!("  Open Time: {}", open_time);
    
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
    let user_wallet_keypair = Keypair::new();
    
    // Parse pubkeys
    let amm_program_pubkey = Pubkey::from_str(&amm_program_id)?;
    let coin_mint_pubkey = Pubkey::from_str(&coin_mint)?;
    let pc_mint_pubkey = Pubkey::from_str(&pc_mint)?;
    
    // Airdrop SOL to user wallet
    println!("Airdropping SOL to user wallet...");
    let signature = rpc_client.request_airdrop(&user_wallet_keypair.pubkey(), 2_000_000_000)?;
    rpc_client.confirm_transaction(&signature)?;
    println!("Airdrop confirmed: {}", signature);
    
    // Create associated token accounts for user
    let user_coin_ata = spl_associated_token_account::get_associated_token_address(
        &user_wallet_keypair.pubkey(),
        &coin_mint_pubkey,
    );
    let user_pc_ata = spl_associated_token_account::get_associated_token_address(
        &user_wallet_keypair.pubkey(),
        &pc_mint_pubkey,
    );
    let user_lp_ata = spl_associated_token_account::get_associated_token_address(
        &user_wallet_keypair.pubkey(),
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
    
    // Create ATAs for user
    instructions.push(ata_instruction::create_associated_token_account(
        &user_wallet_keypair.pubkey(),
        &user_wallet_keypair.pubkey(),
        &coin_mint_pubkey,
        &spl_token::id(),
    ));
    
    instructions.push(ata_instruction::create_associated_token_account(
        &user_wallet_keypair.pubkey(),
        &user_wallet_keypair.pubkey(),
        &pc_mint_pubkey,
        &spl_token::id(),
    ));
    
    instructions.push(ata_instruction::create_associated_token_account(
        &user_wallet_keypair.pubkey(),
        &user_wallet_keypair.pubkey(),
        &amm_lp_mint_keypair.pubkey(),
        &spl_token::id(),
    ));
    
    // Create AMM accounts
    let amm_account_size = 752; // Size of AmmInfo struct
    let amm_lamports = rpc_client.get_minimum_balance_for_rent_exemption(amm_account_size)?;
    
    instructions.push(system_instruction::create_account(
        &user_wallet_keypair.pubkey(),
        &amm_pool_keypair.pubkey(),
        amm_lamports,
        amm_account_size as u64,
        &amm_program_pubkey,
    ));
    
    // Create other AMM accounts...
    let authority_size = 0; // PDA doesn't need space
    let open_orders_size = 0; // Placeholder
    let target_orders_size = 0; // Placeholder
    
    // Create initialize instruction
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
        &user_wallet_keypair.pubkey(),
        &user_coin_ata,
        &user_pc_ata,
        &user_lp_ata,
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
        Some(&user_wallet_keypair.pubkey()),
    );
    
    transaction.sign(
        &[&user_wallet_keypair, &amm_pool_keypair, &amm_authority_keypair, 
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

async fn deposit_liquidity(
    rpc_client: &RpcClient,
    pool_address: String,
    coin_amount: u64,
    pc_amount: u64,
    min_lp_amount: u64,
) -> Result<()> {
    println!("Depositing liquidity to pool: {}", pool_address);
    println!("  Coin Amount: {}", coin_amount);
    println!("  PC Amount: {}", pc_amount);
    println!("  Min LP Amount: {}", min_lp_amount);
    
    // This would create a deposit instruction
    // For now, just show the parameters
    println!("Note: Deposit functionality requires pool state analysis");
    println!("Pool address: {}", pool_address);
    
    Ok(())
}

async fn withdraw_liquidity(
    rpc_client: &RpcClient,
    pool_address: String,
    lp_amount: u64,
    min_coin_amount: Option<u64>,
    min_pc_amount: Option<u64>,
) -> Result<()> {
    println!("Withdrawing liquidity from pool: {}", pool_address);
    println!("  LP Amount: {}", lp_amount);
    if let Some(min_coin) = min_coin_amount {
        println!("  Min Coin Amount: {}", min_coin);
    }
    if let Some(min_pc) = min_pc_amount {
        println!("  Min PC Amount: {}", min_pc);
    }
    
    // This would create a withdraw instruction
    println!("Note: Withdraw functionality requires pool state analysis");
    
    Ok(())
}

async fn swap_tokens(
    rpc_client: &RpcClient,
    pool_address: String,
    amount_in: u64,
    min_amount_out: u64,
    base_in: bool,
) -> Result<()> {
    println!("Swapping tokens on pool: {}", pool_address);
    println!("  Amount In: {}", amount_in);
    println!("  Min Amount Out: {}", min_amount_out);
    println!("  Base In: {}", base_in);
    
    // This would create a swap instruction
    println!("Note: Swap functionality requires pool state analysis");
    
    Ok(())
}

async fn get_pool_info(
    rpc_client: &RpcClient,
    pool_address: String,
) -> Result<()> {
    println!("Getting pool information for: {}", pool_address);
    
    let pool_pubkey = Pubkey::from_str(&pool_address)?;
    
    // Get account info
    match rpc_client.get_account(&pool_pubkey) {
        Ok(account) => {
            println!("Pool account found!");
            println!("  Owner: {}", account.owner);
            println!("  Lamports: {}", account.lamports);
            println!("  Data length: {} bytes", account.data.len());
            
            // Try to deserialize as AmmInfo
            if account.owner == Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")? {
                println!("  This appears to be a Raydium AMM pool");
                // You could deserialize the data here to show pool details
            }
        }
        Err(e) => {
            println!("Error getting pool info: {}", e);
        }
    }
    
    Ok(())
}