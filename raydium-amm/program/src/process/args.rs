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

#[derive(Default, Serialize, Deserialize)]
pub struct WithdrawLog {
    pub log_type: u8,
    pub withdraw_lp: u64,
    pub user_lp: u64,
    pub pool_coin: u64,
    pub pool_pc: u64,
    pub pool_lp: u64,
    pub calc_pnl_x: u128,
    pub calc_pnl_y: u128,
    pub out_coin: u64,
    pub out_pc: u64,
}

#[derive(Default, Serialize, Deserialize)]
pub struct SwapBaseInLog {
    pub log_type: u8,
    pub amount_in: u64,
    pub minimum_out: u64,
    pub direction: u64,
    pub user_source: u64,
    pub pool_coin: u64,
    pub pool_pc: u64,
    pub out_amount: u64,
}

#[derive(Default, Serialize, Deserialize)]
pub struct SwapBaseOutLog {
    pub log_type: u8,
    pub max_in: u64,
    pub amount_out: u64,
    pub direction: u64,
    pub user_source: u64,
    pub pool_coin: u64,
    pub pool_pc: u64,
    pub deduct_in: u64,
}

#[derive(Serialize, Deserialize)]
pub enum LogType {
    Deposit,
    Withdraw,
    SwapBaseIn,
    SwapBaseOut,
}

impl LogType {
    pub fn into_u8(self) -> u8 {
        match self {
            LogType::Deposit => 0,
            LogType::Withdraw => 1,
            LogType::SwapBaseIn => 2,
            LogType::SwapBaseOut => 3,
        }
    }
} 