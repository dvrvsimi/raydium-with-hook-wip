# Raydium AMM with Token-2022 Support

## 🏆 Hackathon Project: Token-2022 + Transfer Hooks on AMM

This project extends Raydium AMM to support Token-2022 with transfer hooks, enabling programmable token behavior in DeFi trading.

## 🎯 Problem Solved

**Current Issue**: No major AMMs support Token-2022 with active transfer hooks, limiting adoption of programmable tokens for real-world assets (RWA) and enterprise use cases.

**Our Solution**: Modified Raydium AMM to safely handle Token-2022 transfers with transfer hooks through:
- Whitelisted hook program validation
- Transfer hook execution during swaps
- TLV account resolution for extra accounts
- Transferring flag management

## 🚀 Features

### ✅ Core Functionality
- **Token-2022 Mint Creation**: Create tokens with transfer hooks
- **AMM Pool Initialization**: Initialize liquidity pools with Token-2022 tokens
- **Safe Trading**: Execute swaps while respecting transfer hook logic
- **Hook Whitelisting**: Permissioned but safe hook approval system

### ✅ Security Features
- **Transfer Hook Validation**: Only whitelisted hook programs can execute
- **Account Resolution**: Proper TLV account handling for hook programs
- **Transferring Flags**: Prevents reentrancy attacks
- **Error Handling**: Graceful fallbacks for hook failures

### ✅ Developer Experience
- **CLI Tool**: Command-line interface for token and pool operations
- **React UI**: User-friendly web interface for demos
- **Comprehensive Documentation**: Clear setup and usage instructions

## 🛠️ Tech Stack

- **Solana Program**: Rust + Anchor framework
- **Token-2022**: SPL Token-2022 with transfer hooks
- **Frontend**: React + TypeScript
- **CLI**: Rust + Clap
- **Dependencies**: 
  - `spl-token-2022`
  - `spl-transfer-hook-interface`
  - `spl-tlv-account-resolution`

## 📁 Project Structure

```
raydium-amm/
├── program/                 # Solana program (Rust)
│   ├── src/
│   │   ├── process/
│   │   │   ├── token2022.rs    # Token-2022 operations
│   │   │   ├── initialize.rs   # Pool initialization
│   │   │   └── swap.rs         # Trading logic
│   │   ├── invokers.rs         # Transfer hook integration
│   │   └── lib.rs              # Main program entry
├── cli/                    # Command-line interface
│   └── src/main.rs
├── ui/                     # React web interface
│   └── src/App.tsx
└── HACKATHON_README.md     # This file
```

## 🚀 Quick Start

### 1. Build the Program
```bash
cd raydium-amm/program
cargo build-bpf
```

### 2. Deploy to Devnet
```bash
solana program deploy target/deploy/raydium_amm.so --url devnet
```

### 3. Run CLI Commands
```bash
cd raydium-amm/cli
cargo run -- create-token --name "My Token" --symbol "MTK" --decimals 9
cargo run -- init-pool --token-mint <MINT_ADDRESS> --pc-mint <USDC_MINT>
cargo run -- swap --pool-address <POOL_ADDRESS> --amount-in 1000000
```

### 4. Start UI
```bash
cd raydium-amm/ui
npm install
npm start
```

## 🎥 Demo Flow

### Step 1: Create Token-2022 with Transfer Hook
1. Navigate to the web UI
2. Enter token details (name, symbol, decimals)
3. Optionally add transfer hook program ID
4. Click "Create Token-2022"
5. Token is created with transfer hook extension

### Step 2: Initialize AMM Pool
1. Enter the created token mint address
2. Enter USDC mint address as PC token
3. Set initial liquidity amounts
4. Click "Initialize Pool"
5. AMM pool is created with Token-2022 support

### Step 3: Trade Tokens
1. Enter pool address
2. Set swap amount
3. Click "Swap Tokens"
4. Transfer hook executes during swap
5. Trade completes successfully

## 🔒 Security Architecture

### Transfer Hook Integration
```rust
// Check if token has transfer hook
if let Some(hook_program_id) = get_transfer_hook_program_id(&mint) {
    // Validate against whitelist
    if !HOOK_WHITELIST.contains(&hook_program_id) {
        return Err(ProgramError::Custom(1234));
    }
    
    // Execute transfer hook
    let hook_ix = TransferHookInstruction::Execute { amount };
    invoke(&hook_ix, &account_infos)?;
}
```

### Whitelist Management
- Hardcoded whitelist for demo (replace with on-chain registry)
- Only approved hook programs can execute
- Prevents malicious hook programs from being used

## 🎯 Hackathon Requirements Met

### ✅ Functionality
- ✅ Create Token-2022 with Transfer Hook
- ✅ Create LP pool (SOL-token pair)
- ✅ Enable trading with hook validation

### ✅ UI Components
- ✅ Video demo (walkthrough of the flow)
- ✅ Live demo (deployed to devnet)
- ✅ Source code (complete implementation)

### ✅ Bonus Points
- ✅ Whitelisted hook approval system
- ✅ Multiple hook support architecture
- ✅ Direct integration with existing AMM protocol

## 🔮 Future Enhancements

### Production Ready
- On-chain hook registry instead of hardcoded whitelist
- Governance system for hook approval
- Advanced hook validation logic
- Multi-signature security

### Additional Features
- Support for multiple transfer hook types
- Cross-chain hook execution
- Advanced RWA token features
- Enterprise compliance hooks

## 🐛 Known Limitations

1. **Demo Hook Programs**: Currently uses placeholder hook programs
2. **Whitelist**: Hardcoded for demo (should be on-chain)
3. **Error Handling**: Basic error handling (needs improvement)
4. **Testing**: Limited test coverage (needs comprehensive tests)

## 📞 Contact

- **Team**: Hackathon Team
- **GitHub**: [Repository Link]
- **Demo**: [Live Demo Link]
- **Video**: [Demo Video Link]

## 📄 License

Apache 2.0 - Same as original Raydium AMM

---

**Built for the Token-2022 Hackathon** 🚀
*Making real-world asset trading on-chain a reality* 

new raydium program id: 3bTCD4MnbUsi6Ad1dqotiBQtiPbzKJbFmzkqQz8A1kag
hook program id: CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1