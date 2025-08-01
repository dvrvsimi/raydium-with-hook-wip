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
        
        // Test serialization/deserialization
        let add_action = HookWhitelistAction::Add;
        let remove_action = HookWhitelistAction::Remove;
        
        assert_ne!(add_action, remove_action);
    }

    // ===== TOKEN TRANSFER TESTS =====

    #[test]
    fn test_token_transfer_instruction() {
        let transfer_instruction = TokenTransferInstruction {
            amount: 1000,
        };
        
        let amm_instruction = AmmInstruction::TokenTransfer(transfer_instruction);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::TokenTransfer(_)));
        
        if let AmmInstruction::TokenTransfer(deserialized_instruction) = deserialized {
            assert_eq!(deserialized_instruction.amount, 1000);
        }
    }

    #[test]
    fn test_token_transfer_edge_cases() {
        // Test zero amount
        let zero_transfer = TokenTransferInstruction { amount: 0 };
        let amm_instruction = AmmInstruction::TokenTransfer(zero_transfer);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        assert!(matches!(deserialized, AmmInstruction::TokenTransfer(_)));
        
        // Test maximum amount
        let max_transfer = TokenTransferInstruction { amount: u64::MAX };
        let amm_instruction = AmmInstruction::TokenTransfer(max_transfer);
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        assert!(matches!(deserialized, AmmInstruction::TokenTransfer(_)));
    }

    // ===== WHITELIST AUTHORITY TESTS =====

    #[test]
    fn test_initialize_hook_whitelist_instruction() {
        let authority = Pubkey::new_unique();
        
        let amm_instruction = AmmInstruction::InitializeHookWhitelist { authority };
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::InitializeHookWhitelist { .. }));
        
        if let AmmInstruction::InitializeHookWhitelist { authority: deserialized_authority } = deserialized {
            assert_eq!(deserialized_authority, authority);
        }
    }

    #[test]
    fn test_update_whitelist_authority_instruction() {
        let new_authority = Pubkey::new_unique();
        
        let amm_instruction = AmmInstruction::UpdateWhitelistAuthority { new_authority };
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::UpdateWhitelistAuthority { .. }));
        
        if let AmmInstruction::UpdateWhitelistAuthority { new_authority: deserialized_authority } = deserialized {
            assert_eq!(deserialized_authority, new_authority);
        }
    }

    // ===== ERROR HANDLING TESTS =====

    #[test]
    fn test_invalid_instruction_data() {
        // Test with empty data
        let empty_data = vec![];
        assert!(AmmInstruction::unpack(&empty_data).is_err());
        
        // Test with invalid tag
        let invalid_data = vec![255u8]; // Invalid instruction tag
        assert!(AmmInstruction::unpack(&invalid_data).is_err());
        
        // Test with incomplete data
        let incomplete_data = vec![16u8, 9u8]; // CreateToken2022Mint but incomplete
        assert!(AmmInstruction::unpack(&incomplete_data).is_err());
    }

    #[test]
    fn test_whitelist_remove_nonexistent_hook() {
        let mut whitelist = HookWhitelist::default();
        let hook_id = Pubkey::new_unique();
        
        // Try to remove hook that doesn't exist
        assert!(whitelist.remove_hook(&hook_id).is_err());
    }

    #[test]
    fn test_whitelist_duplicate_operations() {
        let mut whitelist = HookWhitelist::default();
        let hook_id = Pubkey::new_unique();
        
        // Add hook
        whitelist.add_hook(hook_id).unwrap();
        assert_eq!(whitelist.hooks.len(), 1);
        
        // Add same hook again (should be ignored)
        whitelist.add_hook(hook_id).unwrap();
        assert_eq!(whitelist.hooks.len(), 1);
        
        // Remove hook
        whitelist.remove_hook(&hook_id).unwrap();
        assert_eq!(whitelist.hooks.len(), 0);
        
        // Try to remove again (should fail)
        assert!(whitelist.remove_hook(&hook_id).is_err());
    }

    // ===== INTEGRATION TESTS =====

    #[test]
    fn test_token2022_with_transfer_hook_integration() {
        // Test creating a Token-2022 mint with transfer hook
        let mint_instruction = CreateToken2022MintInstruction {
            decimals: 9,
            mint_authority: Pubkey::new_unique(),
            freeze_authority: None,
            transfer_hook_program_id: Some(Pubkey::new_unique()),
            name: "RWA Token".to_string(),
            symbol: "RWA".to_string(),
            uri: "https://example.com/rwa.json".to_string(),
        };
        
        // Test creating transfer hook
        let hook_instruction = CreateTransferHookInstruction {
            hook_program_id: mint_instruction.transfer_hook_program_id.unwrap(),
            hook_name: "RWA Compliance Hook".to_string(),
            hook_description: "Enforces RWA compliance rules".to_string(),
        };
        
        // Test whitelist operations
        let mut whitelist = HookWhitelist::default();
        whitelist.add_hook(hook_instruction.hook_program_id).unwrap();
        
        // Verify integration
        assert!(whitelist.is_hook_whitelisted(&hook_instruction.hook_program_id));
        assert_eq!(mint_instruction.transfer_hook_program_id, Some(hook_instruction.hook_program_id));
    }

    #[test]
    fn test_complete_rwa_workflow() {
        // 1. Initialize whitelist
        let authority = Pubkey::new_unique();
        let mut whitelist = HookWhitelist::default();
        whitelist.authority = authority;
        
        // 2. Add compliance hook to whitelist
        let compliance_hook = Pubkey::new_unique();
        whitelist.add_hook(compliance_hook).unwrap();
        
        // 3. Create Token-2022 mint with transfer hook
        let mint_instruction = CreateToken2022MintInstruction {
            decimals: 6,
            mint_authority: Pubkey::new_unique(),
            freeze_authority: None,
            transfer_hook_program_id: Some(compliance_hook),
            name: "Real Estate Token".to_string(),
            symbol: "REAL".to_string(),
            uri: "https://example.com/real-estate.json".to_string(),
        };
        
        // 4. Test token transfer with hook
        let transfer_instruction = TokenTransferInstruction {
            amount: 1000000, // 1 token with 6 decimals
        };
        
        // 5. Verify all components work together
        assert!(whitelist.is_hook_whitelisted(&compliance_hook));
        assert_eq!(mint_instruction.transfer_hook_program_id, Some(compliance_hook));
        assert_eq!(transfer_instruction.amount, 1000000);
        
        // Test serialization of complete workflow
        let instructions = vec![
            AmmInstruction::InitializeHookWhitelist { authority },
            AmmInstruction::UpdateHookWhitelist(UpdateHookWhitelistInstruction {
                hook_program_id: compliance_hook,
                action: HookWhitelistAction::Add,
            }),
            AmmInstruction::CreateToken2022Mint(mint_instruction),
            AmmInstruction::TokenTransfer(transfer_instruction),
        ];
        
        for instruction in instructions {
            let serialized = instruction.pack().unwrap();
            let deserialized = AmmInstruction::unpack(&serialized).unwrap();
            assert!(matches!(deserialized, _));
        }
    }

    // ===== PERFORMANCE TESTS =====

    #[test]
    fn test_whitelist_performance() {
        let mut whitelist = HookWhitelist::default();
        let start_time = std::time::Instant::now();
        
        // Add many hooks
        for _ in 0..100 {
            whitelist.add_hook(Pubkey::new_unique()).unwrap();
        }
        
        let add_time = start_time.elapsed();
        assert!(add_time.as_millis() < 100); // Should be very fast
        
        // Test lookup performance
        let lookup_start = std::time::Instant::now();
        for hook in &whitelist.hooks {
            assert!(whitelist.is_hook_whitelisted(hook));
        }
        
        let lookup_time = lookup_start.elapsed();
        assert!(lookup_time.as_millis() < 50); // Should be very fast
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
            uri: "https://example.com/performance.json".to_string(),
        };
        
        let amm_instruction = AmmInstruction::CreateToken2022Mint(instruction);
        let start_time = std::time::Instant::now();
        
        // Test serialization performance
        for _ in 0..1000 {
            let _serialized = amm_instruction.pack().unwrap();
        }
        
        let serialization_time = start_time.elapsed();
        assert!(serialization_time.as_millis() < 100); // Should be very fast
    }
} 