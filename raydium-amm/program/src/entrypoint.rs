//! Program entrypoint definitions

#![cfg(not(feature = "no-entrypoint"))]

use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, msg,
    program_error::PrintProgramError, pubkey::Pubkey,
};

use crate::{error::AmmError, process::process_deposit};

entrypoint!(process_instruction);

fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = crate::instruction::AmmInstruction::unpack(instruction_data)?;

    match instruction {
        crate::instruction::AmmInstruction::Deposit(deposit) => {
            msg!("Instruction: Deposit");
            process_deposit(program_id, accounts, deposit)
        }
        // Add other instruction handlers as needed
        _ => Err(AmmError::InvalidInstruction.into()),
    }
}
