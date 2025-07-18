use serde::{Serialize, Deserialize};

#[derive(Default, Serialize, Deserialize)]
pub struct DepositLog {
    pub log_type: u8,
    pub max_coin: u64,
    pub max_pc: u64,
    pub base: u64,
    pub pool_coin: u64,
    pub pool_pc: u64,
    pub pool_lp: u64,
    pub calc_pnl_x: u128,
    pub calc_pnl_y: u128,
    pub deduct_coin: u64,
    pub deduct_pc: u64,
    pub mint_lp: u64,
}

#[derive(Serialize, Deserialize)]
pub enum LogType {
    Deposit,
    // TODO: Add other log types as needed
}

impl LogType {
    pub fn into_u8(self) -> u8 {
        match self {
            LogType::Deposit => 0,
            // TODO: Map other log types
        }
    }
} 