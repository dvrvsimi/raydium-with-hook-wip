#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::{
        account_info::AccountInfo,
        program_error::ProgramError,
        pubkey::Pubkey,
    };
    use crate::instruction::{
        AmmInstruction,
        CreateToken2022MintInstruction,
        CreateTransferHookInstruction,
        UpdateHookWhitelistInstruction,
        TokenTransferInstruction,
        HookWhitelistAction,
    };
    use crate::state::{HookWhitelist, find_whitelist_pda};

    #[test]
    fn test_hook_whitelist_operations() {
        let mut whitelist = HookWhitelist::default();
        let hook_program_id = Pubkey::new_unique();
        
        // Test adding hook
        whitelist.add_hook(hook_program_id).unwrap();
        assert!(whitelist.is_hook_whitelisted(&hook_program_id));
        assert_eq!(whitelist.hooks.len(), 1);
        
        // Test adding duplicate hook
        whitelist.add_hook(hook_program_id).unwrap();
        assert_eq!(whitelist.hooks.len(), 1); // Should not add duplicate
        
        // Test removing hook
        whitelist.remove_hook(&hook_program_id).unwrap();
        assert!(!whitelist.is_hook_whitelisted(&hook_program_id));
        assert_eq!(whitelist.hooks.len(), 0);
    }

    #[test]
    fn test_whitelist_pda_derivation() {
        let program_id = Pubkey::new_unique();
        let (pda, bump) = find_whitelist_pda(&program_id);
        
        // Verify PDA derivation
        let expected_pda = Pubkey::find_program_address(
            &[b"hook_whitelist"],
            &program_id,
        );
        
        assert_eq!(pda, expected_pda.0);
        assert_eq!(bump, expected_pda.1);
    }

    #[test]
    fn test_instruction_serialization() {
        // Test CreateToken2022MintInstruction
        let instruction = CreateToken2022MintInstruction {
            decimals: 9,
            mint_authority: Pubkey::new_unique(),
            freeze_authority: None,
            transfer_hook_program_id: Some(Pubkey::new_unique()),
            name: "Test Token".to_string(),
            symbol: "TEST".to_string(),
            uri: "https://example.com/metadata.json".to_string(),
        };
        
        let amm_instruction = AmmInstruction::CreateToken2022Mint(instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::CreateToken2022Mint(_)));
    }

    #[test]
    fn test_whitelist_action_serialization() {
        let hook_program_id = Pubkey::new_unique();
        
        // Test Add action
        let add_instruction = UpdateHookWhitelistInstruction {
            hook_program_id,
            action: HookWhitelistAction::Add,
        };
        
        let amm_instruction = AmmInstruction::UpdateHookWhitelist(add_instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::UpdateHookWhitelist(_)));
        
        // Test Remove action
        let remove_instruction = UpdateHookWhitelistInstruction {
            hook_program_id,
            action: HookWhitelistAction::Remove,
        };
        
        let amm_instruction = AmmInstruction::UpdateHookWhitelist(remove_instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::UpdateHookWhitelist(_)));
    }

    #[test]
    fn test_token_transfer_instruction() {
        let transfer_instruction = TokenTransferInstruction {
            amount: 1000,
        };
        
        let amm_instruction = AmmInstruction::TokenTransfer(transfer_instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::TokenTransfer(_)));
    }

    #[test]
    fn test_hook_whitelist_pack_unpack() {
        let mut whitelist = HookWhitelist::default();
        whitelist.authority = Pubkey::new_unique();
        whitelist.add_hook(Pubkey::new_unique()).unwrap();
        whitelist.add_hook(Pubkey::new_unique()).unwrap();
        
        // Pack
        let mut data = vec![0u8; HookWhitelist::LEN];
        whitelist.pack_into_slice(&mut data);
        
        // Unpack
        let unpacked = HookWhitelist::unpack_from_slice(&data).unwrap();
        
        assert_eq!(whitelist.authority, unpacked.authority);
        assert_eq!(whitelist.hooks.len(), unpacked.hooks.len());
        assert_eq!(whitelist.hooks, unpacked.hooks);
    }
} 