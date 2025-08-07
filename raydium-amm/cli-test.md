# CLI Testing Guide for Raydium AMM with Token-2022 Support

## 🎯 Overview

This guide provides step-by-step instructions to test all CLI commands implemented in the Raydium AMM with Token-2022 support. Each command will be tested on Solana devnet.

## 📋 Prerequisites

### 1. Environment Setup
```bash
# Ensure you're in the raydium-amm directory
cd raydium-amm

# Build the CLI
cargo build --bin raydium-amm-cli

# Check if CLI is built successfully
./target/debug/raydium-amm-cli --help
```

### 2. Solana Configuration
```bash
# Set to devnet
solana config set --url devnet

# Check your keypair
solana address

# Check balance (should have some SOL)
solana balance

# If low on SOL, airdrop
solana airdrop 2
```

### 3. Keypair Setup
```bash
# Ensure you have a keypair file
ls ~/.config/solana/id.json

# If not, create one
solana-keygen new --outfile ~/.config/solana/id.json
```

## 🧪 Test Commands

### **Phase 1: Basic Setup and Information**

#### Test 1: Get Whitelist Info (Read-only)
```bash
# Test getting whitelist information
./target/debug/raydium-amm-cli get-whitelist-info

# Expected output:
# Getting whitelist info...
#   AMM Program ID: 3bTCD4MnbUsi6Ad1dqotiBQtiPbzKJbFmzkqQz8A1kag
#   Whitelist PDA: [some PDA address]
#   Whitelist account does not exist
```

**✅ Success Criteria**: Command runs without errors and shows whitelist PDA

---

### **Phase 2: Whitelist Management**

#### Test 2: Initialize Whitelist
```bash
# Initialize the whitelist for transfer hooks
./target/debug/raydium-amm-cli init-whitelist

# Expected output:
# Initializing whitelist...
#   AMM Program ID: 3bTCD4MnbUsi6Ad1dqotiBQtiPbzKJbFmzkqQz8A1kag
#   Payer: [your pubkey]
#   Whitelist PDA: [PDA address]
# Whitelist initialized successfully!
#   Transaction Signature: [signature]
#   Explorer: https://explorer.solana.com/tx/[signature]?cluster=devnet
```

**✅ Success Criteria**: Transaction succeeds, whitelist account is created

#### Test 3: Add Hook to Whitelist
```bash
# Add the real whitelist transfer hook program to whitelist
./target/debug/raydium-amm-cli add-hook-to-whitelist \
  --hook-program-id CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1

# Expected output:
# Adding hook to whitelist: CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1
#   AMM Program ID: 3bTCD4MnbUsi6Ad1dqotiBQtiPbzKJbFmzkqQz8A1kag
#   Payer: [your pubkey]
# Hook added to whitelist successfully!
#   Transaction Signature: [signature]
```

**✅ Success Criteria**: Hook is successfully added to whitelist

#### Test 4: Verify Whitelist Info
```bash
# Check whitelist info again
./target/debug/raydium-amm-cli get-whitelist-info

# Expected output:
# Getting whitelist info...
#   AMM Program ID: 3bTCD4MnbUsi6Ad1dqotiBQtiPbzKJbFmzkqQz8A1kag
#   Whitelist PDA: [PDA address]
# Whitelist account exists
#   Account size: [size] bytes
#   Whitelist is initialized
```

**✅ Success Criteria**: Shows whitelist is initialized and contains the hook

---

### **Phase 3: Token-2022 with Transfer Hook**

#### Test 5: Create Token-2022 Mint with Transfer Hook
```bash
# Create a Token-2022 mint with transfer hook
./target/debug/raydium-amm-cli create-hook-mint \
  --hook-program-id CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1 \
  --decimals 9 \
  --initial-supply 1000000000

# Expected output:
# Creating Token-2022 mint with transfer hook...
# Token-2022 mint with transfer hook created successfully!
#   Mint Address: [mint address]
#   Hook Program: CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1
#   Payer ATA: [ATA address]
#   Transaction: [signature]
```

**✅ Success Criteria**: Token-2022 mint is created with transfer hook extension

**📝 Note**: Save the mint address for next tests

#### Test 6: Initialize Transfer Hook Meta List
```bash
# Replace [MINT_ADDRESS] with the mint address from previous test
./target/debug/raydium-amm-cli init-hook-meta-list \
  --hook-program-id CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1 \
  --mint [MINT_ADDRESS]

# Expected output:
# Initializing transfer hook meta list...
#   Hook Program ID: CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1
#   Mint: [mint address]
# Transfer hook meta list initialized successfully!
#   Meta List PDA: [PDA address]
#   Whitelist PDA: [PDA address]
#   Transaction: [signature]
```

**✅ Success Criteria**: Meta list is initialized for the mint

#### Test 7: Initialize Transfer Hook Whitelist
```bash
# Initialize whitelist for the transfer hook program
./target/debug/raydium-amm-cli init-hook-whitelist \
  --hook-program-id CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1

# Expected output:
# Initializing transfer hook whitelist...
#   Hook Program ID: CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1
# Transfer hook whitelist initialized successfully!
#   Transaction: [signature]
```

**✅ Success Criteria**: Transfer hook whitelist is initialized

#### Test 8: Add User to Transfer Hook Whitelist
```bash
# Add your address to the transfer hook whitelist
./target/debug/raydium-amm-cli add-to-hook-whitelist \
  --hook-program-id CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1 \
  --user [YOUR_PUBKEY]

# Expected output:
# Adding user to transfer hook whitelist: [your pubkey]
# User added to transfer hook whitelist successfully!
#   Transaction: [signature]
```

**✅ Success Criteria**: User is added to transfer hook whitelist

---

### **Phase 4: AMM Pool Operations**

#### Test 9: Initialize AMM Pool
```bash
# Initialize AMM pool with Token-2022 token
./target/debug/raydium-amm-cli init-pool \
  --coin-mint [MINT_ADDRESS] \
  --pc-mint EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v \
  --init-coin-amount 1000000000 \
  --init-pc-amount 1000000000 \
  --open-time $(date +%s)

# Expected output:
# Initializing Raydium AMM pool...
#   AMM Program ID: 3bTCD4MnbUsi6Ad1dqotiBQtiPbzKJbFmzkqQz8A1kag
#   Coin Mint: [mint address]
#   PC Mint: EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v
#   Initial Coin Amount: 1000000000
#   Initial PC Amount: 1000000000
#   Nonce: 0
#   Open Time: [timestamp]
#   Payer: [your pubkey]
# Pool initialized successfully!
#   Pool Address: [pool address]
#   Transaction: [signature]
```

**✅ Success Criteria**: AMM pool is created with Token-2022 support

**📝 Note**: Save the pool address for future tests

---

### **Phase 5: Transfer Hook Testing**

#### Test 10: Test Hook Transfer
```bash
# Test transfer with hook validation
./target/debug/raydium-amm-cli test-hook-transfer \
  --mint [MINT_ADDRESS] \
  --source [PAYER_ATA_ADDRESS] \
  --destination [DESTINATION_ATA_ADDRESS] \
  --amount 1000000

# Expected output:
# Testing transfer with hook validation...
#   Mint: [mint address]
#   Source: [source ATA]
#   Destination: [destination ATA]
#   Amount: 1000000
# ✅ Transfer succeeded - user is whitelisted!
#   Transaction: [signature]
```

**✅ Success Criteria**: Transfer succeeds because user is whitelisted

---

### **Phase 6: Cleanup and Management**

#### Test 11: Remove Hook from Whitelist
```bash
# Remove hook from AMM whitelist
./target/debug/raydium-amm-cli remove-hook-from-whitelist \
  --hook-program-id CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1

# Expected output:
# Removing hook from whitelist: CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1
#   AMM Program ID: 3bTCD4MnbUsi6Ad1dqotiBQtiPbzKJbFmzkqQz8A1kag
#   Payer: [your pubkey]
# Hook removed from whitelist successfully!
#   Transaction Signature: [signature]
```

**✅ Success Criteria**: Hook is successfully removed from whitelist

---

## 🔍 Verification Steps

### Check All Created Accounts
```bash
# Check whitelist account
solana account [WHITELIST_PDA] --url devnet

# Check mint account
solana account [MINT_ADDRESS] --url devnet

# Check pool account
solana account [POOL_ADDRESS] --url devnet

# Check transfer hook whitelist
solana account [HOOK_WHITELIST_PDA] --url devnet
```

### Verify Token-2022 Extension
```bash
# Check if mint has transfer hook extension
spl-token display [MINT_ADDRESS] --url devnet
```

## 🚨 Troubleshooting

### Common Issues

1. **Insufficient SOL**
   ```bash
   solana airdrop 2
   ```

2. **Keypair Issues**
   ```bash
   # Check keypair format
   cat ~/.config/solana/id.json
   
   # Regenerate if needed
   solana-keygen new --outfile ~/.config/solana/id.json
   ```

3. **Transaction Failures**
   ```bash
   # Check transaction status
   solana confirm [SIGNATURE] --url devnet
   
   # Check account status
   solana account [ACCOUNT_ADDRESS] --url devnet
   ```

4. **Program Not Found**
   ```bash
   # Verify program is deployed
   solana program show 3bTCD4MnbUsi6Ad1dqotiBQtiPbzKJbFmzkqQz8A1kag --url devnet
   ```

## 📊 Expected Test Results

| Test | Command | Expected Result |
|------|---------|-----------------|
| 1 | `get-whitelist-info` | Shows whitelist PDA |
| 2 | `init-whitelist` | Creates whitelist account |
| 3 | `add-hook-to-whitelist` | Adds hook to whitelist |
| 4 | `get-whitelist-info` | Shows initialized whitelist |
| 5 | `create-hook-mint` | Creates Token-2022 mint |
| 6 | `init-hook-meta-list` | Initializes meta list |
| 7 | `init-hook-whitelist` | Creates hook whitelist |
| 8 | `add-to-hook-whitelist` | Adds user to whitelist |
| 9 | `init-pool` | Creates AMM pool |
| 10 | `test-hook-transfer` | Transfer succeeds |
| 11 | `remove-hook-from-whitelist` | Removes hook |

## 🎯 Success Criteria

✅ **All commands execute without errors**  
✅ **Transactions are confirmed on devnet**  
✅ **Accounts are created with correct data**  
✅ **Transfer hooks work as expected**  
✅ **Whitelist management functions properly**  
✅ **Token-2022 extensions are properly initialized**  

## 📝 Notes

- All tests use **devnet** for safety
- **Real SOL** is used for transaction fees
- **Real program IDs** are used (deployed programs)
- **Proper error handling** should be tested
- **Transaction confirmations** should be verified
- **Account states** should be checked after operations

---

**Ready to test! 🚀** 