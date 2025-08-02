#[cfg(test)]
mod tests {
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
        // TransferHookInstruction removed - use SPL Transfer Hook Interface instead
        HookWhitelistAction,
    };
    use crate::state::{HookWhitelist, find_whitelist_pda};
    use solana_program::program_pack::Pack;

    // ===== WHITELIST TESTS =====

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
    fn test_whitelist_capacity_limits() {
        let mut whitelist = HookWhitelist::default();
        
        // Add many hooks
        for _ in 0..100 {
            let hook_id = Pubkey::new_unique();
            whitelist.add_hook(hook_id).unwrap();
        }
        
        assert_eq!(whitelist.hooks.len(), 100);
        
        // Try to add one more - should fail
        let extra_hook = Pubkey::new_unique();
        assert!(whitelist.add_hook(extra_hook).is_err());
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

    // ===== TOKEN-2022 MINT CREATION TESTS =====

    #[test]
    fn test_create_token2022_mint_instruction_serialization() {
        let instruction = CreateToken2022MintInstruction {
            decimals: 9,
            mint_authority: Pubkey::new_unique(),
            freeze_authority: Some(Pubkey::new_unique()),
            transfer_hook_program_id: Some(Pubkey::new_unique()),
            name: "Test Token".to_string(),
            symbol: "TEST".to_string(),
            uri: "https://example.com/metadata.json".to_string(),
        };
        
        let amm_instruction = AmmInstruction::CreateToken2022Mint(instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::CreateToken2022Mint(_)));
        
        if let AmmInstruction::CreateToken2022Mint(deserialized_instruction) = deserialized {
            assert_eq!(deserialized_instruction.decimals, 9);
            assert_eq!(deserialized_instruction.name, "Test Token");
            assert_eq!(deserialized_instruction.symbol, "TEST");
            assert_eq!(deserialized_instruction.uri, "https://example.com/metadata.json");
        }
    }

    #[test]
    fn test_create_token2022_mint_without_hook() {
        let instruction = CreateToken2022MintInstruction {
            decimals: 6,
            mint_authority: Pubkey::new_unique(),
            freeze_authority: None,
            transfer_hook_program_id: None,
            name: "Simple Token".to_string(),
            symbol: "SIMPLE".to_string(),
            uri: "".to_string(),
        };
        
        let amm_instruction = AmmInstruction::CreateToken2022Mint(instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::CreateToken2022Mint(_)));
    }

    // ===== TRANSFER HOOK TESTS =====

    #[test]
    fn test_create_transfer_hook_instruction() {
        let instruction = CreateTransferHookInstruction {
            hook_program_id: Pubkey::new_unique(),
            hook_name: "KYC Hook".to_string(),
            hook_description: "Verifies KYC before transfer".to_string(),
        };
        
        let amm_instruction = AmmInstruction::CreateTransferHook(instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::CreateTransferHook(_)));
        
        if let AmmInstruction::CreateTransferHook(deserialized_instruction) = deserialized {
            assert_eq!(deserialized_instruction.hook_name, "KYC Hook");
            assert_eq!(deserialized_instruction.hook_description, "Verifies KYC before transfer");
        }
    }

    // TransferHook instruction test removed - use SPL Transfer Hook Interface instead

    // ===== WHITELIST ACTION TESTS =====

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
    fn test_hook_whitelist_action_enum() {
        assert_eq!(HookWhitelistAction::Add as u8, 0);
        assert_eq!(HookWhitelistAction::Remove as u8, 1);
    }

    // ===== TOKEN TRANSFER TESTS =====

    #[test]
    fn test_token_transfer_instruction() {
        let instruction = TokenTransferInstruction {
            amount: 1000000,
        };
        
        let amm_instruction = AmmInstruction::TokenTransfer(instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::TokenTransfer(_)));
        
        if let AmmInstruction::TokenTransfer(deserialized_instruction) = deserialized {
            assert_eq!(deserialized_instruction.amount, 1000000);
        }
    }

    #[test]
    fn test_token_transfer_edge_cases() {
        // Test zero amount
        let instruction = TokenTransferInstruction { amount: 0 };
        let amm_instruction = AmmInstruction::TokenTransfer(instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        assert!(matches!(deserialized, AmmInstruction::TokenTransfer(_)));
        
        // Test large amount
        let instruction = TokenTransferInstruction { amount: u64::MAX };
        let amm_instruction = AmmInstruction::TokenTransfer(instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        assert!(matches!(deserialized, AmmInstruction::TokenTransfer(_)));
    }

    // ===== WHITELIST INITIALIZATION TESTS =====

    #[test]
    fn test_initialize_hook_whitelist_instruction() {
        let authority = Pubkey::new_unique();
        let amm_instruction = AmmInstruction::InitializeHookWhitelist { authority };
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::InitializeHookWhitelist { .. }));
    }

    #[test]
    fn test_update_whitelist_authority_instruction() {
        let new_authority = Pubkey::new_unique();
        let amm_instruction = AmmInstruction::UpdateWhitelistAuthority { new_authority };
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::UpdateWhitelistAuthority { .. }));
    }

    // ===== ERROR HANDLING TESTS =====

    #[test]
    fn test_invalid_instruction_data() {
        // Test empty data
        assert!(AmmInstruction::unpack(&[]).is_err());
        
        // Test invalid discriminator
        let invalid_data = vec![255]; // Invalid discriminator
        assert!(AmmInstruction::unpack(&invalid_data).is_err());
    }

    // ===== WHITELIST OPERATION TESTS =====

    #[test]
    fn test_whitelist_remove_nonexistent_hook() {
        let mut whitelist = HookWhitelist::default();
        let hook_id = Pubkey::new_unique();
        
        // Try to remove a hook that doesn't exist
        assert!(whitelist.remove_hook(&hook_id).is_err());
    }

    #[test]
    fn test_whitelist_duplicate_operations() {
        let mut whitelist = HookWhitelist::default();
        let hook_id = Pubkey::new_unique();
        
        // Add hook
        whitelist.add_hook(hook_id).unwrap();
        assert_eq!(whitelist.hooks.len(), 1);
        
        // Try to add the same hook again
        whitelist.add_hook(hook_id).unwrap(); // Should not add duplicate
        assert_eq!(whitelist.hooks.len(), 1);
        
        // Remove hook
        whitelist.remove_hook(&hook_id).unwrap();
        assert_eq!(whitelist.hooks.len(), 0);
        
        // Try to remove the same hook again
        assert!(whitelist.remove_hook(&hook_id).is_err());
    }

    // ===== INTEGRATION TESTS =====

    #[test]
    fn test_token2022_with_transfer_hook_integration() {
        // Test creating a token with transfer hook
        let hook_program_id = Pubkey::new_unique();
        let create_hook_instruction = CreateTransferHookInstruction {
            hook_program_id,
            hook_name: "Test Hook".to_string(),
            hook_description: "Test hook for integration".to_string(),
        };
        
        let amm_instruction = AmmInstruction::CreateTransferHook(create_hook_instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::CreateTransferHook(_)));
        
        // Test creating a token with the hook
        let create_token_instruction = CreateToken2022MintInstruction {
            decimals: 9,
            mint_authority: Pubkey::new_unique(),
            freeze_authority: Some(Pubkey::new_unique()),
            transfer_hook_program_id: Some(hook_program_id),
            name: "Test Token".to_string(),
            symbol: "TEST".to_string(),
            uri: "https://example.com/metadata.json".to_string(),
        };
        
        let amm_instruction = AmmInstruction::CreateToken2022Mint(create_token_instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::CreateToken2022Mint(_)));
    }

    #[test]
    fn test_complete_rwa_workflow() {
        // Test complete RWA (Real World Asset) workflow
        let hook_program_id = Pubkey::new_unique();
        
        // 1. Create transfer hook
        let create_hook_instruction = CreateTransferHookInstruction {
            hook_program_id,
            hook_name: "RWA Hook".to_string(),
            hook_description: "Real World Asset transfer hook".to_string(),
        };
        
        let amm_instruction = AmmInstruction::CreateTransferHook(create_hook_instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        assert!(matches!(deserialized, AmmInstruction::CreateTransferHook(_)));
        
        // 2. Create token with transfer hook
        let create_token_instruction = CreateToken2022MintInstruction {
            decimals: 6,
            mint_authority: Pubkey::new_unique(),
            freeze_authority: Some(Pubkey::new_unique()),
            transfer_hook_program_id: Some(hook_program_id),
            name: "RWA Token".to_string(),
            symbol: "RWA".to_string(),
            uri: "https://example.com/rwa-metadata.json".to_string(),
        };
        
        let amm_instruction = AmmInstruction::CreateToken2022Mint(create_token_instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        assert!(matches!(deserialized, AmmInstruction::CreateToken2022Mint(_)));
        
        // 3. Initialize whitelist
        let authority = Pubkey::new_unique();
        let amm_instruction = AmmInstruction::InitializeHookWhitelist { authority };
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        assert!(matches!(deserialized, AmmInstruction::InitializeHookWhitelist { .. }));
        
        // 4. Add hook to whitelist
        let add_instruction = UpdateHookWhitelistInstruction {
            hook_program_id,
            action: HookWhitelistAction::Add,
        };
        
        let amm_instruction = AmmInstruction::UpdateHookWhitelist(add_instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        assert!(matches!(deserialized, AmmInstruction::UpdateHookWhitelist(_)));
    }

    // ===== PERFORMANCE TESTS =====

    #[test]
    fn test_whitelist_performance() {
        let mut whitelist = HookWhitelist::default();
        
        // Test adding many hooks quickly
        let start = std::time::Instant::now();
        for _ in 0..100 {
            let hook_id = Pubkey::new_unique();
            whitelist.add_hook(hook_id).unwrap();
        }
        let duration = start.elapsed();
        
        // Should complete within reasonable time (less than 1 second)
        assert!(duration.as_millis() < 1000);
        assert_eq!(whitelist.hooks.len(), 100);
    }

    #[test]
    fn test_instruction_serialization_performance() {
        let instruction = CreateToken2022MintInstruction {
            decimals: 9,
            mint_authority: Pubkey::new_unique(),
            freeze_authority: Some(Pubkey::new_unique()),
            transfer_hook_program_id: Some(Pubkey::new_unique()),
            name: "Performance Test Token".to_string(),
            symbol: "PERF".to_string(),
            uri: "https://example.com/performance-metadata.json".to_string(),
        };
        
        let amm_instruction = AmmInstruction::CreateToken2022Mint(instruction);
        
        // Test serialization performance
        let start = std::time::Instant::now();
        for _ in 0..1000 {
            let _serialized = amm_instruction.pack().unwrap();
        }
        let duration = start.elapsed();
        
        // Should complete within reasonable time (less than 1 second)
        assert!(duration.as_millis() < 1000);
    }
} 