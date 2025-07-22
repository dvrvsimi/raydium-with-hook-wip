pub const AUTHORITY_AMM: &[u8] = b"amm authority";
pub const AMM_ASSOCIATED_SEED: &[u8] = b"amm_associated_seed";
pub const TARGET_ASSOCIATED_SEED: &[u8] = b"target_associated_seed";
pub const OPEN_ORDER_ASSOCIATED_SEED: &[u8] = b"open_order_associated_seed";
pub const COIN_VAULT_ASSOCIATED_SEED: &[u8] = b"coin_vault_associated_seed";
pub const PC_VAULT_ASSOCIATED_SEED: &[u8] = b"pc_vault_associated_seed";
pub const LP_MINT_ASSOCIATED_SEED: &[u8] = b"lp_mint_associated_seed";
pub const AMM_CONFIG_SEED: &[u8] = b"amm_config_account_seed";

use solana_program::pubkey::Pubkey;
use std::str::FromStr;
use crate::error::AmmError;

// Referrer wallet IDs for different networks
// These are the same IDs used in the original config_feature module
#[cfg(feature = "testnet")]
pub const REFERRER_PC_WALLET_ID: &str = "75KWb5XcqPTgacQyNw9P5QU2HL3xpezEVcgsFCiJgTT";
#[cfg(feature = "devnet")]
pub const REFERRER_PC_WALLET_ID: &str = "4NpMfWThvJQsV9VLjUXXpn3tPv1zoQpib8wCBDc1EBzD";
#[cfg(not(any(feature = "testnet", feature = "devnet")))]
pub const REFERRER_PC_WALLET_ID: &str = "FCxGKqGSVeV1d3WsmAXt45A5iQdCS6kKCeJy3EUBigMG";

// AMM owner IDs for different networks
#[cfg(feature = "testnet")]
pub const AMM_OWNER_ID: &str = "75KWb5XcqPTgacQyNw9P5QU2HL3xpezEVcgsFCiJgTT";
#[cfg(feature = "devnet")]
pub const AMM_OWNER_ID: &str = "Adm29NctkKwJGaaiU8CXqdV6WDTwR81JbxV8zoxn745Y";
#[cfg(not(any(feature = "testnet", feature = "devnet")))]
pub const AMM_OWNER_ID: &str = "GThUX1Atko4tqhN2NaiTazWSeFWMuiUvfFnyJyUghFMJ";

// OpenBook program IDs for different networks
#[cfg(feature = "testnet")]
pub const OPENBOOK_PROGRAM_ID: &str = "6ccSma8mmmmQXcSFpheSKrTnwsCe5pBuEpzDLFjrAsCF";
#[cfg(feature = "devnet")]
pub const OPENBOOK_PROGRAM_ID: &str = "EoTcMgcDRTJVZDMZWBoU6rhYHZfkNTVEAfz3uUJRcYGj";
#[cfg(not(any(feature = "testnet", feature = "devnet")))]
pub const OPENBOOK_PROGRAM_ID: &str = "srmqPvymJeFKQ4zGQed1GFppgkRHL9kaELCbyksJtPX";

// Create pool fee address IDs for different networks
#[cfg(feature = "testnet")]
pub const CREATE_POOL_FEE_ADDRESS_ID: &str = "3TRTX4dXUpp2eqxi3tvQDFYUV7SdDJjcPE3Y4mbtftaX";
#[cfg(feature = "devnet")]
pub const CREATE_POOL_FEE_ADDRESS_ID: &str = "3XMrhbv989VxAMi3DErLV9eJht1pHppW5LbKxe9fkEFR";
#[cfg(not(any(feature = "testnet", feature = "devnet")))]
pub const CREATE_POOL_FEE_ADDRESS_ID: &str = "7YttLkHDoNj9wyDur5pM1ejNaAvT9X4eqaYcHQqtj2G5";

pub fn get_referrer_pc_wallet_id() -> Result<Pubkey, AmmError> {
    Pubkey::from_str(REFERRER_PC_WALLET_ID)
        .map_err(|_| AmmError::InvalidProgramAddress)
}

pub fn get_amm_owner_id() -> Result<Pubkey, AmmError> {
    Pubkey::from_str(AMM_OWNER_ID)
        .map_err(|_| AmmError::InvalidProgramAddress)
}

pub fn get_openbook_program_id() -> Result<Pubkey, AmmError> {
    Pubkey::from_str(OPENBOOK_PROGRAM_ID)
        .map_err(|_| AmmError::InvalidProgramAddress)
}

pub fn get_create_pool_fee_address_id() -> Result<Pubkey, AmmError> {
    Pubkey::from_str(CREATE_POOL_FEE_ADDRESS_ID)
        .map_err(|_| AmmError::InvalidProgramAddress)
} 