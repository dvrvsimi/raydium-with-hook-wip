// #![deny(missing_docs)]

//! An Uniswap-like program for the Solana blockchain.
pub mod error;
pub mod instruction;
pub mod invokers;
pub mod log;
pub mod math;
pub mod process;
pub mod state;

#[cfg(test)]
mod tests;

use solana_program::{
    account_info::AccountInfo,
    entrypoint,
    entrypoint::ProgramResult,
    pubkey::Pubkey,
};

entrypoint!(process_instruction);

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    input: &[u8],
) -> ProgramResult {
    let instruction = crate::instruction::AmmInstruction::unpack(input)?;
    match instruction {
        crate::instruction::AmmInstruction::PreInitialize(_init_arg) => {
            unimplemented!("This instruction is not supported, please use Initialize2")
        }
        crate::instruction::AmmInstruction::Initialize(_init1) => {
            unimplemented!("This instruction is not supported, please use Initialize2")
        }
        crate::instruction::AmmInstruction::Initialize2(init2) => {
            crate::process::process_initialize2(program_id, accounts, init2)
        }
        crate::instruction::AmmInstruction::MonitorStep(monitor) => {
            crate::process::process_monitor_step(program_id, accounts, monitor)
        }
        crate::instruction::AmmInstruction::Deposit(deposit) => {
            crate::process::process_deposit(program_id, accounts, deposit)
        }
        crate::instruction::AmmInstruction::Withdraw(withdraw) => {
            crate::process::process_withdraw(program_id, accounts, withdraw)
        }
        crate::instruction::AmmInstruction::MigrateToOpenBook => {
            crate::process::process_migrate_to_openbook(program_id, accounts)
        }
        crate::instruction::AmmInstruction::SetParams(setparams) => {
            crate::process::process_set_params(program_id, accounts, setparams)
        }
        crate::instruction::AmmInstruction::WithdrawPnl => {
            crate::process::process_withdrawpnl(program_id, accounts)
        }
        crate::instruction::AmmInstruction::WithdrawSrm(withdrawsrm) => {
            crate::process::process_withdraw_srm(program_id, accounts, withdrawsrm)
        }
        crate::instruction::AmmInstruction::SwapBaseIn(swap) => {
            crate::process::process_swap_base_in(program_id, accounts, swap)
        }
        crate::instruction::AmmInstruction::SwapBaseOut(swap) => {
            crate::process::process_swap_base_out(program_id, accounts, swap)
        }
        crate::instruction::AmmInstruction::SimulateInfo(simulate) => {
            crate::process::process_simulate_info(program_id, accounts, simulate)
        }
        crate::instruction::AmmInstruction::AdminCancelOrders(cancel) => {
            crate::process::process_admin_cancel_orders(program_id, accounts, cancel)
        }
        crate::instruction::AmmInstruction::CreateConfigAccount => {
            crate::process::process_create_config(program_id, accounts)
        }
        crate::instruction::AmmInstruction::UpdateConfigAccount(config_args) => {
            crate::process::process_update_config(program_id, accounts, config_args)
        }
        crate::instruction::AmmInstruction::CreateToken2022Mint(create_mint) => {
            crate::process::token2022::process_create_token2022_mint(program_id, accounts, create_mint)
        }
        crate::instruction::AmmInstruction::CreateTransferHook(create_hook) => {
            crate::process::token2022::process_create_transfer_hook(program_id, accounts, create_hook)
        }
        crate::instruction::AmmInstruction::UpdateHookWhitelist(update_whitelist) => {
            crate::process::process_update_hook_whitelist(program_id, accounts, update_whitelist)
        }
        crate::instruction::AmmInstruction::TokenTransfer(transfer) => {
            crate::process::token2022::process_token_transfer(program_id, accounts, transfer)
        }
    }
}

// Export current solana-sdk types for downstream users who may also be building with a different solana-sdk version
pub use solana_program;

#[cfg(not(feature = "no-entrypoint"))]
solana_security_txt::security_txt! {
    name: "raydium-amm",
    project_url: "https://raydium.io",
    contacts: "link:https://immunefi.com/bounty/raydium",
    policy: "https://immunefi.com/bounty/raydium",
    source_code: "https://github.com/raydium-io/raydium-amm",
    preferred_languages: "en",
    auditors: "https://github.com/raydium-io/raydium-docs/blob/master/audit/MadSheild%20Q2%202023/Raydium%20updated%20orderbook%20AMM%20program%20%26%20OpenBook%20migration.pdf"
}

#[cfg(feature = "devnet")]
solana_program::declare_id!("HWy1jotHpo6UqeQxx49dpYYdQB8wj9Qk9MdxwjLvDHB8");
#[cfg(not(feature = "devnet"))]
solana_program::declare_id!("675kPX9MHTjS2zt1qfr1NYHuzeLXfQM9H24wFSUt1Mp8");
