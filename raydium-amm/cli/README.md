# Raydium AMM Client Library

A Rust library for interacting with Raydium AMM pools with full Token-2022 and transfer hook support.

## Features

- ✅ **Full Token-2022 Support** - Automatic detection and handling of Token-2022 mints
- ✅ **Transfer Hook Integration** - Create and manage transfer hooks with whitelist functionality
- ✅ **AMM Pool Management** - Initialize and manage Raydium AMM pools
- ✅ **Keypair Utilities** - Support for multiple keypair formats (JSON array, base58, JSON object)
- ✅ **Production Ready** - Comprehensive error handling and validation
- ✅ **Async/Await** - Modern async Rust API

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
raydium-amm-cli = { path = "path/to/raydium-amm/cli" }
tokio = { version = "1.0", features = ["full"] }
```

## Quick Start

```rust
use raydium_amm_client::{RaydiumClient, keypair_utils};
use solana_sdk::{pubkey::Pubkey, signature::Signer};
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create client with default devnet configuration
    let client = RaydiumClient::new();
    
    // Generate or load a keypair
    let payer = keypair_utils::generate_keypair();
    println!("Payer: {}", payer.pubkey());
    
    // Check if a mint is Token-2022
    let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;
    let is_token2022 = client.is_token2022_mint(&usdc_mint).await?;
    println!("USDC is Token-2022: {}", is_token2022);
    
    Ok(())
}
```

## API Reference

### RaydiumClient

Main client for interacting with Raydium AMM.

#### Creation

```rust
// Default devnet client
let client = RaydiumClient::new();

// Custom configuration
let config = RaydiumConfig {
    rpc_url: "https://api.mainnet-beta.solana.com".to_string(),
    commitment: CommitmentConfig::confirmed(),
    amm_program_id: Pubkey::from_str("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8")?,
};
let client = RaydiumClient::with_config(config);
```

#### AMM Pool Operations

```rust
// Initialize AMM pool
let signature = client.init_amm_pool(
    coin_mint,          // Token mint
    pc_mint,            // Quote currency mint  
    1000000000,         // Initial coin amount
    1000000,            // Initial PC amount
    0,                  // Nonce
    1673234400,         // Open time
    &payer_keypair,     // Payer
).await?;

// Initialize whitelist
let signature = client.init_whitelist(&payer).await?;

// Add hook to whitelist
let signature = client.add_hook_to_whitelist(hook_program_id, &payer).await?;

// Get whitelist info
let whitelist_data = client.get_whitelist_info().await?;
```

#### Token-2022 & Hook Operations

```rust
// Create Token-2022 mint with transfer hook
let (mint_pubkey, signature) = client.create_hook_mint(
    hook_program_id,
    9,                  // Decimals
    1000000000,         // Initial supply
    &payer,
).await?;

// Initialize hook meta list
let signature = client.init_hook_meta_list(
    hook_program_id,
    mint_pubkey,
    &payer,
).await?;

// Initialize hook whitelist  
let signature = client.init_hook_whitelist(hook_program_id, &payer).await?;

// Add user to hook whitelist
let signature = client.add_to_hook_whitelist(
    hook_program_id,
    user_pubkey,
    &payer,
).await?;

// Test hook transfer
let signature = client.test_hook_transfer(
    mint_pubkey,
    source_ata,
    destination_ata,
    amount,
    &owner,
).await?;
```

#### Utility Functions

```rust
// Check if mint is Token-2022
let is_token2022 = client.is_token2022_mint(&mint_pubkey).await?;
```

### Keypair Utilities

```rust
use raydium_amm_client::keypair_utils;

// Generate new keypair
let keypair = keypair_utils::generate_keypair();

// Load from file
let keypair = keypair_utils::load_keypair_from_file("~/.config/solana/id.json")?;

// Load from string (useful for web apps)
let json_array = "[1,2,3,...]"; // Your keypair bytes
let keypair = keypair_utils::load_keypair_from_string(json_array)?;

// Convert keypair to different formats
let json_array = keypair_utils::keypair_to_json_array(&keypair);
let base58 = keypair_utils::keypair_to_base58(&keypair);
```

## Supported Keypair Formats

1. **JSON Array** (most common):
   ```json
   [1,2,3,4,5,...]
   ```

2. **Base58 String** (with quotes):
   ```json
   "5J6xqhBs8q..."
   ```

3. **JSON Object**:
   ```json
   {"private_key": [1,2,3,4,5,...]}
   ```

## Web Integration Recommendations

For client-side UI integration, consider these approaches:

### Option 1: WebAssembly (WASM)
- Compile this library to WASM for direct browser use
- Requires WASM-compatible HTTP client

### Option 2: TypeScript Port
- Port core functionality to TypeScript using `@solana/web3.js`
- Better browser compatibility and ecosystem integration

### Option 3: HTTP Service
- Wrap this library in a REST API service
- Call from your UI via HTTP requests

### Option 4: Library Structure (Current)
- Use this library in a Rust backend
- Expose endpoints for your frontend to consume

## Error Handling

All methods return `Result<T, anyhow::Error>` for comprehensive error handling:

```rust
match client.init_amm_pool(params...).await {
    Ok(signature) => println!("Success: {}", signature),
    Err(e) => {
        eprintln!("Error: {}", e);
        // Handle specific error cases
        if e.to_string().contains("insufficient funds") {
            // Handle insufficient funds
        }
    }
}
```

## Examples

See `examples/client_usage.rs` for comprehensive usage examples.

Run the example:
```bash
cargo run --example client_usage
```

## CLI Tool

This library also provides a CLI tool for testing and administration:

```bash
# Build CLI
cargo build --release

# Initialize whitelist
cargo run -- init-whitelist

# Create Token-2022 mint with hook
cargo run -- create-hook-mint --hook-program-id <PROGRAM_ID>

# Initialize AMM pool
cargo run -- init-pool --coin-mint <MINT> --pc-mint <MINT> --init-coin-amount 1000000000 --init-pc-amount 1000000 --open-time 1673234400
```

## Development

```bash
# Run tests
cargo test

# Check library builds
cargo check --lib

# Check CLI builds  
cargo check --bin raydium-amm-cli

# Run example
cargo run --example client_usage
```

## License

This project follows the same license as the parent Raydium AMM project.

## Contributing

1. Fork the repository
2. Create your feature branch
3. Add tests for new functionality  
4. Run `cargo test` and `cargo check`
5. Submit a pull request

## Architecture Notes

- **Async Design**: All network operations are async for better performance
- **Error Context**: Uses `anyhow` for rich error context and chaining  
- **Token-2022 Auto-Detection**: Automatically detects and handles both SPL and Token-2022 mints
- **Keypair Flexibility**: Supports multiple keypair formats for different use cases
- **Production Ready**: Comprehensive validation and error handling