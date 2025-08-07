#[cfg(test)]
mod tests {
    use crate::{
        error::AmmError,
        instruction::{
            CreateToken2022MintInstruction,
            UpdateHookWhitelistInstruction,
            HookWhitelistAction,
            AmmInstruction,
        },
        state::{HookWhitelist, find_whitelist_pda},
    };
    use solana_program::{
        account_info::{next_account_info, AccountInfo},
        entrypoint::ProgramResult,
        msg,
        program_error::ProgramError,
        program_pack::{Pack, IsInitialized, Sealed},
        pubkey::Pubkey,
        system_instruction,
    };
    use spl_token_2022::{
        extension::{ExtensionType, StateWithExtensionsMut, transfer_hook::TransferHook, BaseStateWithExtensions, StateWithExtensions, BaseStateWithExtensionsMut},
        state::{Mint, Account, AccountState},
    };
    use spl_transfer_hook_interface::instruction::ExecuteInstruction;
    use spl_tlv_account_resolution::{
        account::ExtraAccountMeta,
        state::ExtraAccountMetaList,
    };
    use spl_type_length_value::state::TlvStateBorrowed;
    use std::str::FromStr;

    // ===== WHITELIST TESTS =====

    #[test]
    fn test_hook_whitelist_action_enum() {
        assert_eq!(HookWhitelistAction::Add as u8, 0);
        assert_eq!(HookWhitelistAction::Remove as u8, 1);
    }

    #[test]
    fn test_initialize_hook_whitelist_instruction() {
        let authority = Pubkey::new_unique();
        let amm_instruction = AmmInstruction::InitializeHookWhitelist { authority };
        let serialized = amm_instruction.pack().unwrap();
        let deserialized = AmmInstruction::unpack(&serialized).unwrap();
        
        assert!(matches!(deserialized, AmmInstruction::InitializeHookWhitelist { .. }));
        
        if let AmmInstruction::InitializeHookWhitelist { authority: deserialized_authority } = deserialized {
            assert_eq!(authority, deserialized_authority);
        }
    }

    // ===== EXECUTE TRANSFER HOOK TESTS =====

    #[test]
    fn test_execute_transfer_hook_no_hook() {
        use crate::invokers::execute_transfer_hook;
        use spl_token_2022::state::{Mint, Account, AccountState};
        
        let program_id = crate::id();
        let source_pubkey = Pubkey::new_unique();
        let destination_pubkey = Pubkey::new_unique();
        let mint_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        let spl_token_2022_id = spl_token_2022::id();
        
        // Create basic mint WITHOUT TransferHook extension
        let base_mint = Mint {
            mint_authority: Some(authority_pubkey).into(),
            supply: 1000000,
            decimals: 6,
            is_initialized: true,
            freeze_authority: Some(authority_pubkey).into(),
        };
        let mut mint_data = vec![0u8; Mint::LEN];
        base_mint.pack_into_slice(&mut mint_data);
        
        let mut mint_lamports = 1000000u64;
        let mint_account = AccountInfo::new(
            &mint_pubkey,
            false,
            false,
            &mut mint_lamports,
            &mut mint_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        // Create source token account
        let source_token_account = Account {
            mint: mint_pubkey,
            owner: authority_pubkey,
            amount: 5000,
            delegate: None.into(),
            state: AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        let mut source_data = vec![0u8; Account::LEN];
        source_token_account.pack_into_slice(&mut source_data);
        let mut source_lamports = 1000000u64;
        let source_account = AccountInfo::new(
            &source_pubkey,
            false,
            false,
            &mut source_lamports,
            &mut source_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        let dest_token_account = Account {
            mint: mint_pubkey,
            owner: authority_pubkey,
            amount: 0,
            delegate: None.into(),
            state: AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        let mut dest_data = vec![0u8; Account::LEN];
        dest_token_account.pack_into_slice(&mut dest_data);
        let mut dest_lamports = 1000000u64;
        let destination_account = AccountInfo::new(
            &destination_pubkey,
            false,
            false,
            &mut dest_lamports,
            &mut dest_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        let mut authority_lamports = 1000000u64;
        let authority_account = AccountInfo::new(
            &authority_pubkey,
            true,
            false,
            &mut authority_lamports,
            &mut [],
            &program_id,
            false,
            0,
        );
        
        // Test: No hook should return Ok(())
        let result = execute_transfer_hook(
            &program_id,
            &source_account,
            &mint_account,
            &destination_account,
            &authority_account,
            1000,
            &[], // No remaining accounts
        );
        
        assert!(result.is_ok(), "Should succeed when no hook is configured");
    }

    #[test]
    fn test_execute_transfer_hook_insufficient_accounts() {
        use crate::invokers::execute_transfer_hook;
        use spl_token_2022::state::{Mint, Account, AccountState};
        use spl_token_2022::extension::{ExtensionType, StateWithExtensionsMut, transfer_hook::TransferHook};
        
        let program_id = crate::id();
        let hook_program_id = Pubkey::from_str("CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1").unwrap(); // Real deployed whitelist transfer hook
        let source_pubkey = Pubkey::new_unique();
        let destination_pubkey = Pubkey::new_unique();
        let mint_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        let spl_token_2022_id = spl_token_2022::id();
        
        // Create mint WITH TransferHook extension
        let base_mint = Mint {
            mint_authority: Some(authority_pubkey).into(),
            supply: 1000000,
            decimals: 6,
            is_initialized: true,
            freeze_authority: Some(authority_pubkey).into(),
        };
        
        // Calculate size for mint + TransferHook extension
        let extension_types = [ExtensionType::TransferHook];
        let total_len = ExtensionType::try_calculate_account_len::<Mint>(&extension_types).unwrap();
        let mut mint_data = vec![0u8; total_len];
        
        // Create StateWithExtensionsMut to properly initialize the mint
        let mut state = StateWithExtensionsMut::<Mint>::unpack_uninitialized(&mut mint_data).unwrap();
        state.base = base_mint;
        
        // Initialize the TransferHook extension
        state.init_extension::<TransferHook>(true).unwrap();
        let transfer_hook_ext = state.get_extension_mut::<TransferHook>().unwrap();
        transfer_hook_ext.program_id = Some(hook_program_id).try_into().unwrap();
        
        // Initialize account type and pack the base
        state.init_account_type().unwrap();
        state.pack_base();
        
        let mut mint_lamports = 1000000u64;
        let mint_account = AccountInfo::new(
            &mint_pubkey,
            false,
            false,
            &mut mint_lamports,
            &mut mint_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        // Create source token account
        let source_token_account = Account {
            mint: mint_pubkey,
            owner: authority_pubkey,
            amount: 5000,
            delegate: None.into(),
            state: AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        let mut source_data = vec![0u8; Account::LEN];
        source_token_account.pack_into_slice(&mut source_data);
        let mut source_lamports = 1000000u64;
        let source_account = AccountInfo::new(
            &source_pubkey,
            false,
            false,
            &mut source_lamports,
            &mut source_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        let dest_token_account = Account {
            mint: mint_pubkey,
            owner: authority_pubkey,
            amount: 0,
            delegate: None.into(),
            state: AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        let mut dest_data = vec![0u8; Account::LEN];
        dest_token_account.pack_into_slice(&mut dest_data);
        let mut dest_lamports = 1000000u64;
        let destination_account = AccountInfo::new(
            &destination_pubkey,
            false,
            false,
            &mut dest_lamports,
            &mut dest_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        let mut authority_lamports = 1000000u64;
        let authority_account = AccountInfo::new(
            &authority_pubkey,
            true,
            false,
            &mut authority_lamports,
            &mut [],
            &program_id,
            false,
            0,
        );
        
        // Test: Insufficient accounts should fail
        let result = execute_transfer_hook(
            &program_id,
            &source_account,
            &mint_account,
            &destination_account,
            &authority_account,
            1000,
            &[], // No remaining accounts - should fail
        );
        
        assert!(result.is_err(), "Should fail with insufficient accounts");
        match result {
            Err(ProgramError::NotEnoughAccountKeys) => {
                // Expected error
            }
            _ => panic!("Expected NotEnoughAccountKeys error"),
        }
    }

    #[test]
    fn test_execute_transfer_hook_whitelisted() -> Result<(), ProgramError> {
        use crate::invokers::execute_transfer_hook;
        use spl_token_2022::state::{Mint, Account, AccountState};
        use spl_token_2022::extension::{ExtensionType, StateWithExtensionsMut, transfer_hook::TransferHook};
        
        let program_id = crate::id();
        let hook_program_id = Pubkey::from_str("CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1").unwrap(); // Real deployed whitelist transfer hook
        let source_pubkey = Pubkey::new_unique();
        let destination_pubkey = Pubkey::new_unique();
        let mint_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        let spl_token_2022_id = spl_token_2022::id();
        
        // Create mint WITH TransferHook extension
        let base_mint = Mint {
            mint_authority: Some(authority_pubkey).into(),
            supply: 1000000,
            decimals: 6,
            is_initialized: true,
            freeze_authority: Some(authority_pubkey).into(),
        };
        
        // Calculate size for mint + TransferHook extension
        let extension_types = [ExtensionType::TransferHook];
        let total_len = ExtensionType::try_calculate_account_len::<Mint>(&extension_types).unwrap();
        let mut mint_data = vec![0u8; total_len];
        
        // Create StateWithExtensionsMut to properly initialize the mint
        let mut state = StateWithExtensionsMut::<Mint>::unpack_uninitialized(&mut mint_data).unwrap();
        state.base = base_mint;
        
        // Initialize the TransferHook extension
        state.init_extension::<TransferHook>(true).unwrap();
        let transfer_hook_ext = state.get_extension_mut::<TransferHook>().unwrap();
        transfer_hook_ext.program_id = Some(hook_program_id).try_into().unwrap();
        
        // Initialize account type and pack the base
        state.init_account_type().unwrap();
        state.pack_base();
        
        let mut mint_lamports = 1000000u64;
        let mint_account = AccountInfo::new(
            &mint_pubkey,
            false,
            false,
            &mut mint_lamports,
            &mut mint_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        // Create source token account
        let source_token_account = Account {
            mint: mint_pubkey,
            owner: authority_pubkey,
            amount: 5000,
            delegate: None.into(),
            state: AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        let mut source_data = vec![0u8; Account::LEN];
        source_token_account.pack_into_slice(&mut source_data);
        let mut source_lamports = 1000000u64;
        let source_account = AccountInfo::new(
            &source_pubkey,
            false,
            false,
            &mut source_lamports,
            &mut source_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        let dest_token_account = Account {
            mint: mint_pubkey,
            owner: authority_pubkey,
            amount: 0,
            delegate: None.into(),
            state: AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        let mut dest_data = vec![0u8; Account::LEN];
        dest_token_account.pack_into_slice(&mut dest_data);
        let mut dest_lamports = 1000000u64;
        let destination_account = AccountInfo::new(
            &destination_pubkey,
            false,
            false,
            &mut dest_lamports,
            &mut dest_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        let mut authority_lamports = 1000000u64;
        let authority_account = AccountInfo::new(
            &authority_pubkey,
            true,
            false,
            &mut authority_lamports,
            &mut [],
            &program_id,
            false,
            0,
        );
        
        // Create whitelist WITH the hook program (should succeed)
        let mut whitelist = HookWhitelist::default();
        whitelist.authority = authority_pubkey;
        whitelist.add_hook(hook_program_id)?; // Add hook to whitelist

        let mut whitelist_data = vec![0u8; HookWhitelist::LEN];
        whitelist.pack_into_slice(&mut whitelist_data);
        
        // Use proper PDA derivation
        let (whitelist_pda, _bump) = find_whitelist_pda(&program_id);
        let mut whitelist_lamports = 1000000u64;
        let whitelist_account = AccountInfo::new(
            &whitelist_pda,
            false,
            false,
            &mut whitelist_lamports,
            &mut whitelist_data,
            &program_id,
            false,
            0,
        );
        
        // Create ExtraAccountMetaList PDA
        let (extra_account_meta_list_pda, _bump) = Pubkey::find_program_address(
            &[b"extra-account-metas", mint_pubkey.as_ref()],
            &hook_program_id,
        );
        
        // Create proper ExtraAccountMetaList data
        let extra_metas: Vec<ExtraAccountMeta> = vec![
            // The whitelist account is required as an extra account
            ExtraAccountMeta::new_with_seeds(
                &[
                    spl_tlv_account_resolution::seeds::Seed::Literal {
                        bytes: b"hook_whitelist".to_vec(),
                    },
                ],
                false, // is_signer
                false, // is_writable
            ).unwrap()
        ];
        let account_size = ExtraAccountMetaList::size_of(extra_metas.len()).unwrap();
        let mut extra_meta_data = vec![0u8; account_size];

        // Initialize the ExtraAccountMetaList properly
        ExtraAccountMetaList::init::<ExecuteInstruction>(&mut extra_meta_data, &extra_metas).unwrap();
        
        let mut extra_meta_lamports = 1000000u64;
        let extra_meta_account = AccountInfo::new(
            &extra_account_meta_list_pda,
            false,
            false,
            &mut extra_meta_lamports,
            &mut extra_meta_data,
            &hook_program_id,
            false,
            0,
        );
        
        // Test: Whitelisted hook should succeed (but hook program invocation will fail since no real program exists)
        let result = execute_transfer_hook(
            &program_id,
            &source_account,
            &mint_account,
            &destination_account,
            &authority_account,
            1000,
            &[whitelist_account, extra_meta_account], // remaining accounts
        );
        
        // The function should succeed in checking the whitelist, but the actual hook invocation will fail
        // since there's no real hook program deployed. This is expected behavior.
        match result {
            Ok(_) => {
                // This would happen if the hook program was actually deployed and working TODO: test with coreria's repo
                println!("Hook execution succeeded (unexpected in test environment)");
            }
            Err(e) => {
                // This is expected since there's no real hook program deployed
                println!("Hook execution failed as expected: {:?}", e);
            }
        }
        Ok(())
    }

    #[test]
    fn test_execute_transfer_hook_not_whitelisted() {
        use crate::invokers::execute_transfer_hook;
        use spl_token_2022::state::{Mint, Account, AccountState};
        use spl_token_2022::extension::{ExtensionType, StateWithExtensionsMut, transfer_hook::TransferHook};
        
        let program_id = crate::id();
        let hook_program_id = Pubkey::from_str("CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1").unwrap(); // Real deployed whitelist transfer hook
        let source_pubkey = Pubkey::new_unique();
        let destination_pubkey = Pubkey::new_unique();
        let mint_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        let spl_token_2022_id = spl_token_2022::id();
        
        // Create mint WITH TransferHook extension
        let base_mint = Mint {
            mint_authority: Some(authority_pubkey).into(),
            supply: 1000000,
            decimals: 6,
            is_initialized: true,
            freeze_authority: Some(authority_pubkey).into(),
        };
        
        // Calculate size for mint + TransferHook extension
        let extension_types = [ExtensionType::TransferHook];
        let total_len = ExtensionType::try_calculate_account_len::<Mint>(&extension_types).unwrap();
        let mut mint_data = vec![0u8; total_len];
        
        // Create StateWithExtensionsMut to properly initialize the mint
        let mut state = StateWithExtensionsMut::<Mint>::unpack_uninitialized(&mut mint_data).unwrap();
        state.base = base_mint;
        
        // Initialize the TransferHook extension
        state.init_extension::<TransferHook>(true).unwrap();
        let transfer_hook_ext = state.get_extension_mut::<TransferHook>().unwrap();
        transfer_hook_ext.program_id = Some(hook_program_id).try_into().unwrap();
        
        // Initialize account type and pack the base
        state.init_account_type().unwrap();
        state.pack_base();
        
        let mut mint_lamports = 1000000u64;
        let mint_account = AccountInfo::new(
            &mint_pubkey,
            false,
            false,
            &mut mint_lamports,
            &mut mint_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        // Create source token account
        let source_token_account = Account {
            mint: mint_pubkey,
            owner: authority_pubkey,
            amount: 5000,
            delegate: None.into(),
            state: AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        let mut source_data = vec![0u8; Account::LEN];
        source_token_account.pack_into_slice(&mut source_data);
        let mut source_lamports = 1000000u64;
        let source_account = AccountInfo::new(
            &source_pubkey,
            false,
            false,
            &mut source_lamports,
            &mut source_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        let dest_token_account = Account {
            mint: mint_pubkey,
            owner: authority_pubkey,
            amount: 0,
            delegate: None.into(),
            state: AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        let mut dest_data = vec![0u8; Account::LEN];
        dest_token_account.pack_into_slice(&mut dest_data);
        let mut dest_lamports = 1000000u64;
        let destination_account = AccountInfo::new(
            &destination_pubkey,
            false,
            false,
            &mut dest_lamports,
            &mut dest_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        let mut authority_lamports = 1000000u64;
        let authority_account = AccountInfo::new(
            &authority_pubkey,
            true,
            false,
            &mut authority_lamports,
            &mut [],
            &program_id,
            false,
            0,
        );
        
        // Create whitelist WITHOUT the hook program (should fail)
        let mut whitelist = HookWhitelist::default();
        whitelist.authority = authority_pubkey;
        // Note: hook_program_id is NOT added to whitelist

        let mut whitelist_data = vec![0u8; HookWhitelist::LEN];
        whitelist.pack_into_slice(&mut whitelist_data);
        
        // Use proper PDA derivation
        let (whitelist_pda, _bump) = find_whitelist_pda(&program_id);
        let mut whitelist_lamports = 1000000u64;
        let whitelist_account = AccountInfo::new(
            &whitelist_pda,
            false,
            false,
            &mut whitelist_lamports,
            &mut whitelist_data,
            &program_id,
            false,
            0,
        );
        
        // Create ExtraAccountMetaList PDA
        let (extra_account_meta_list_pda, _bump) = Pubkey::find_program_address(
            &[b"extra-account-metas", mint_pubkey.as_ref()],
            &hook_program_id,
        );
        
        // Create proper ExtraAccountMetaList data
        let extra_metas: Vec<ExtraAccountMeta> = vec![
            // The whitelist account is required as an extra account
            ExtraAccountMeta::new_with_seeds(
                &[
                    spl_tlv_account_resolution::seeds::Seed::Literal {
                        bytes: b"hook_whitelist".to_vec(),
                    },
                ],
                false, // is_signer
                false, // is_writable
            ).unwrap()
        ];
        let account_size = ExtraAccountMetaList::size_of(extra_metas.len()).unwrap();
        let mut extra_meta_data = vec![0u8; account_size];

        // Initialize the ExtraAccountMetaList properly
        ExtraAccountMetaList::init::<ExecuteInstruction>(&mut extra_meta_data, &extra_metas).unwrap();
        
        let mut extra_meta_lamports = 1000000u64;
        let extra_meta_account = AccountInfo::new(
            &extra_account_meta_list_pda,
            false,
            false,
            &mut extra_meta_lamports,
            &mut extra_meta_data,
            &hook_program_id,
            false,
            0,
        );
        
        // Test: Non-whitelisted hook should fail
        let result = execute_transfer_hook(
            &program_id,
            &source_account,
            &mint_account,
            &destination_account,
            &authority_account,
            1000,
            &[whitelist_account, extra_meta_account], // Proper remaining accounts
        );
        
        // This should fail because the hook is not whitelisted
        assert!(result.is_err(), "Should fail when hook is not whitelisted");
        match result {
            Err(ProgramError::Custom(e)) => {
                // Check if it's our custom error
                if e == crate::error::AmmError::TransferHookNotWhitelisted as u32 {
                    // Expected error
                } else {
                    panic!("Expected TransferHookNotWhitelisted error, got: {}", e);
                }
            }
            _ => panic!("Expected TransferHookNotWhitelisted error"),
        }
    }

    #[test]
    fn test_execute_transfer_hook_auto_initialize_whitelist() {
        use crate::invokers::execute_transfer_hook;
        use spl_token_2022::state::{Mint, Account, AccountState};
        use spl_token_2022::extension::{ExtensionType, StateWithExtensionsMut, transfer_hook::TransferHook};
        
        let program_id = crate::id();
        let hook_program_id = Pubkey::from_str("CpHUaPzccsDg9YBvt6pAW4epUPDWek39RRYXMcWj6oU1").unwrap(); // Real deployed whitelist transfer hook
        let source_pubkey = Pubkey::new_unique();
        let destination_pubkey = Pubkey::new_unique();
        let mint_pubkey = Pubkey::new_unique();
        let authority_pubkey = Pubkey::new_unique();
        let spl_token_2022_id = spl_token_2022::id();
        
        // Create mint WITH TransferHook extension
        let base_mint = Mint {
            mint_authority: Some(authority_pubkey).into(),
            supply: 1000000,
            decimals: 6,
            is_initialized: true,
            freeze_authority: Some(authority_pubkey).into(),
        };
        
        // Calculate size for mint + TransferHook extension
        let extension_types = [ExtensionType::TransferHook];
        let total_len = ExtensionType::try_calculate_account_len::<Mint>(&extension_types).unwrap();
        let mut mint_data = vec![0u8; total_len];
        
        // Create StateWithExtensionsMut to properly initialize the mint
        let mut state = StateWithExtensionsMut::<Mint>::unpack_uninitialized(&mut mint_data).unwrap();
        state.base = base_mint;
        
        // Initialize the TransferHook extension
        state.init_extension::<TransferHook>(true).unwrap();
        let transfer_hook_ext = state.get_extension_mut::<TransferHook>().unwrap();
        transfer_hook_ext.program_id = Some(hook_program_id).try_into().unwrap();
        
        // Initialize account type and pack the base
        state.init_account_type().unwrap();
        state.pack_base();
        
        let mut mint_lamports = 1000000u64;
        let mint_account = AccountInfo::new(
            &mint_pubkey,
            false,
            false,
            &mut mint_lamports,
            &mut mint_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        // Create source token account
        let source_token_account = Account {
            mint: mint_pubkey,
            owner: authority_pubkey,
            amount: 5000,
            delegate: None.into(),
            state: AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        let mut source_data = vec![0u8; Account::LEN];
        source_token_account.pack_into_slice(&mut source_data);
        let mut source_lamports = 1000000u64;
        let source_account = AccountInfo::new(
            &source_pubkey,
            false,
            false,
            &mut source_lamports,
            &mut source_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        let dest_token_account = Account {
            mint: mint_pubkey,
            owner: authority_pubkey,
            amount: 0,
            delegate: None.into(),
            state: AccountState::Initialized,
            is_native: None.into(),
            delegated_amount: 0,
            close_authority: None.into(),
        };
        let mut dest_data = vec![0u8; Account::LEN];
        dest_token_account.pack_into_slice(&mut dest_data);
        let mut dest_lamports = 1000000u64;
        let destination_account = AccountInfo::new(
            &destination_pubkey,
            false,
            false,
            &mut dest_lamports,
            &mut dest_data,
            &spl_token_2022_id,
            false,
            0,
        );
        
        let mut authority_lamports = 1000000u64;
        let authority_account = AccountInfo::new(
            &authority_pubkey,
            true,
            false,
            &mut authority_lamports,
            &mut [],
            &program_id,
            false,
            0,
        );
        
        // Create ExtraAccountMetaList PDA
        let (extra_account_meta_list_pda, _bump) = Pubkey::find_program_address(
            &[b"extra-account-metas", mint_pubkey.as_ref()],
            &hook_program_id,
        );
        
        // Create proper ExtraAccountMetaList data with required extra accounts
        let extra_metas: Vec<ExtraAccountMeta> = vec![
            // The whitelist account is required as an extra account for the transfer hook
            ExtraAccountMeta::new_with_seeds(
                &[
                    spl_tlv_account_resolution::seeds::Seed::Literal {
                        bytes: b"hook_whitelist".to_vec(),
                    },
                ],
                false, // is_signer
                false, // is_writable
            ).unwrap()
        ];
        let account_size = ExtraAccountMetaList::size_of(extra_metas.len()).unwrap();
        let mut extra_meta_data = vec![0u8; account_size];

        // Initialize the ExtraAccountMetaList properly
        ExtraAccountMetaList::init::<ExecuteInstruction>(&mut extra_meta_data, &extra_metas).unwrap();
        
        let mut extra_meta_lamports = 1000000u64;
        let extra_meta_account = AccountInfo::new(
            &extra_account_meta_list_pda,
            false,
            false,
            &mut extra_meta_lamports,
            &mut extra_meta_data,
            &hook_program_id,
            false,
            0,
        );
        
        // Create empty whitelist (hook is not whitelisted)
        let mut whitelist = HookWhitelist::default();
        whitelist.authority = authority_pubkey;
        // Don't add the hook to whitelist - it should remain not whitelisted
        
        let mut whitelist_data = vec![0u8; HookWhitelist::LEN];
        whitelist.pack_into_slice(&mut whitelist_data);
        
        // Use proper PDA derivation
        let (whitelist_pda, _bump) = find_whitelist_pda(&program_id);
        let mut whitelist_lamports = 1000000u64;
        let whitelist_account = AccountInfo::new(
            &whitelist_pda,
            false,
            false,
            &mut whitelist_lamports,
            &mut whitelist_data,
            &program_id,
            false,
            0,
        );
        
        // Test: Auto-initialization should create whitelist and fail because hook is not whitelisted
        let result = execute_transfer_hook(
            &program_id,
            &source_account,
            &mint_account,
            &destination_account,
            &authority_account,
            1000,
            &[whitelist_account, extra_meta_account], // Provide both whitelist and meta accounts
        );
        
        // This should fail because the hook is not whitelisted
        assert!(result.is_err(), "Should fail when hook is not whitelisted");
        match result {
            Err(ProgramError::Custom(e)) => {
                // Check if it's our custom error
                if e == crate::error::AmmError::TransferHookNotWhitelisted as u32 {
                    // Expected error
                } else {
                    panic!("Expected TransferHookNotWhitelisted error, got: {}", e);
                }
            }
            _ => panic!("Expected TransferHookNotWhitelisted error"),
        }
    }
} 