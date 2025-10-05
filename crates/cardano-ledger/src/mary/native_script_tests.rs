//! Native Script Validation Tests
//!
//! Comprehensive test suite for Mary era native script validation,
//! covering all script types and validation scenarios.

#[cfg(test)]
mod native_script_tests {
    use super::super::*;

    fn create_test_tx_with_validity(
        invalid_before: Option<Slot>,
        invalid_hereafter: Option<Slot>,
    ) -> MaryTransaction {
        MaryTransaction {
            inputs: vec![],
            outputs: vec![],
            fee: 1000,
            ttl: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            auxiliary_data: None,
            validity_interval: ValidityInterval {
                invalid_before,
                invalid_hereafter,
            },
            mint: None,
            witness_set: MaryWitnessSet {
                vkey_witnesses: vec![],
                native_scripts: vec![],
                bootstrap_witnesses: vec![],
            },
        }
    }

    fn create_vkey_witness(key_bytes: &[u8]) -> (Blake2b256Hash, VKeyWitness) {
        // Create key hash and witness pair
        // The key hash is the hash of the vkey bytes
        let key_hash = Blake2b256Hash::hash(key_bytes);
        let witness = VKeyWitness {
            vkey: key_bytes.to_vec(),
            signature: Ed25519Signature::from_bytes([0u8; 64]),
        };
        (key_hash, witness)
    }

    fn create_test_key() -> (Blake2b256Hash, VKeyWitness) {
        // Create a deterministic key and witness pair
        let vkey_bytes = b"test_public_key_bytes_32_chars!";
        create_vkey_witness(vkey_bytes)
    }

    #[test]
    fn test_script_pubkey_satisfied() {
        let (key_hash, witness) = create_test_key();
        let script = NativeScript::ScriptPubkey(key_hash);

        let mut tx = create_test_tx_with_validity(None, None);
        tx.witness_set.vkey_witnesses.push(witness);

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_ok(),
            "ScriptPubkey should be satisfied with matching signature"
        );
    }

    #[test]
    fn test_script_pubkey_unsatisfied() {
        let key_hash = Blake2b256Hash::hash(b"test_key");
        let script = NativeScript::ScriptPubkey(key_hash);

        let tx = create_test_tx_with_validity(None, None);
        // No witness added

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_err(),
            "ScriptPubkey should fail without matching signature"
        );
    }

    #[test]
    fn test_script_all_satisfied() {
        let (key1, witness1) = create_test_key();

        // Create second key with different bytes
        let vkey2_bytes = b"different_key_bytes_32_chars";
        let key2 = Blake2b256Hash::hash(vkey2_bytes);
        let witness2 = VKeyWitness {
            vkey: vkey2_bytes.to_vec(),
            signature: Ed25519Signature::from_bytes([0u8; 64]),
        };

        let script = NativeScript::ScriptAll(vec![
            NativeScript::ScriptPubkey(key1),
            NativeScript::ScriptPubkey(key2),
        ]);

        let mut tx = create_test_tx_with_validity(None, None);
        tx.witness_set.vkey_witnesses.push(witness1);
        tx.witness_set.vkey_witnesses.push(witness2);

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_ok(),
            "ScriptAll should be satisfied when all sub-scripts pass"
        );
    }

    #[test]
    fn test_script_all_partially_satisfied() {
        let (key1, witness1) = create_vkey_witness(b"key1_test_bytes_32_characters!!");
        let (key2, _witness2) = create_vkey_witness(b"key2_test_bytes_32_characters!!");

        let script = NativeScript::ScriptAll(vec![
            NativeScript::ScriptPubkey(key1),
            NativeScript::ScriptPubkey(key2),
        ]);

        let mut tx = create_test_tx_with_validity(None, None);
        tx.witness_set.vkey_witnesses.push(witness1);
        // Missing key2 signature

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_err(),
            "ScriptAll should fail if any sub-script fails"
        );
    }

    #[test]
    fn test_script_any_satisfied() {
        let (key1, witness1) = create_vkey_witness(b"key1_test_bytes_32_characters!!");
        let (key2, _witness2) = create_vkey_witness(b"key2_test_bytes_32_characters!!");

        let script = NativeScript::ScriptAny(vec![
            NativeScript::ScriptPubkey(key1),
            NativeScript::ScriptPubkey(key2),
        ]);

        let mut tx = create_test_tx_with_validity(None, None);
        tx.witness_set.vkey_witnesses.push(witness1);
        // Only one signature, but that's enough for ScriptAny

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_ok(),
            "ScriptAny should be satisfied when at least one sub-script passes"
        );
    }

    #[test]
    fn test_script_any_unsatisfied() {
        let key1 = Blake2b256Hash::hash(b"key1");
        let key2 = Blake2b256Hash::hash(b"key2");

        let script = NativeScript::ScriptAny(vec![
            NativeScript::ScriptPubkey(key1),
            NativeScript::ScriptPubkey(key2),
        ]);

        let tx = create_test_tx_with_validity(None, None);
        // No signatures

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_err(),
            "ScriptAny should fail when no sub-scripts pass"
        );
    }

    #[test]
    fn test_script_any_empty() {
        let script = NativeScript::ScriptAny(vec![]);
        let tx = create_test_tx_with_validity(None, None);

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(result.is_err(), "ScriptAny with empty list should fail");
    }

    #[test]
    fn test_script_n_of_k_satisfied() {
        let (key1, witness1) = create_vkey_witness(b"key1_test_bytes_32_characters!!");
        let (key2, witness2) = create_vkey_witness(b"key2_test_bytes_32_characters!!");
        let (key3, _witness3) = create_vkey_witness(b"key3_test_bytes_32_characters!!");

        // Require 2 of 3 signatures
        let script = NativeScript::ScriptNOfK(
            2,
            vec![
                NativeScript::ScriptPubkey(key1),
                NativeScript::ScriptPubkey(key2),
                NativeScript::ScriptPubkey(key3),
            ],
        );

        let mut tx = create_test_tx_with_validity(None, None);
        tx.witness_set.vkey_witnesses.push(witness1);
        tx.witness_set.vkey_witnesses.push(witness2);
        // key3 not signed, but we have 2 of 3

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_ok(),
            "ScriptNOfK(2,3) should pass with 2 signatures"
        );
    }

    #[test]
    fn test_script_n_of_k_insufficient() {
        let (key1, witness1) = create_vkey_witness(b"key1_test_bytes_32_characters!!");
        let (key2, _witness2) = create_vkey_witness(b"key2_test_bytes_32_characters!!");
        let (key3, _witness3) = create_vkey_witness(b"key3_test_bytes_32_characters!!");

        // Require 2 of 3 signatures
        let script = NativeScript::ScriptNOfK(
            2,
            vec![
                NativeScript::ScriptPubkey(key1),
                NativeScript::ScriptPubkey(key2),
                NativeScript::ScriptPubkey(key3),
            ],
        );

        let mut tx = create_test_tx_with_validity(None, None);
        tx.witness_set.vkey_witnesses.push(witness1);
        // Only 1 of 3 signatures

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_err(),
            "ScriptNOfK(2,3) should fail with only 1 signature"
        );
    }

    #[test]
    fn test_script_n_of_k_zero() {
        // Require 0 of 2 (always satisfied)
        let script = NativeScript::ScriptNOfK(
            0,
            vec![
                NativeScript::ScriptPubkey(Blake2b256Hash::hash(b"key1")),
                NativeScript::ScriptPubkey(Blake2b256Hash::hash(b"key2")),
            ],
        );

        let tx = create_test_tx_with_validity(None, None);
        // No signatures needed

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(result.is_ok(), "ScriptNOfK(0,_) should always pass");
    }

    #[test]
    fn test_script_n_of_k_invalid() {
        // Require 3 of 2 (impossible)
        let script = NativeScript::ScriptNOfK(
            3,
            vec![
                NativeScript::ScriptPubkey(Blake2b256Hash::hash(b"key1")),
                NativeScript::ScriptPubkey(Blake2b256Hash::hash(b"key2")),
            ],
        );

        let tx = create_test_tx_with_validity(None, None);

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_err(),
            "ScriptNOfK should fail when N > number of scripts"
        );
    }

    #[test]
    fn test_invalid_before_satisfied() {
        let script = NativeScript::InvalidBefore(100);
        let tx = create_test_tx_with_validity(Some(100), None);

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_ok(),
            "InvalidBefore should pass when tx.invalid_before >= script slot"
        );
    }

    #[test]
    fn test_invalid_before_unsatisfied() {
        let script = NativeScript::InvalidBefore(100);
        let tx = create_test_tx_with_validity(Some(50), None);

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_err(),
            "InvalidBefore should fail when tx.invalid_before < script slot"
        );
    }

    #[test]
    fn test_invalid_before_no_constraint() {
        let script = NativeScript::InvalidBefore(100);
        let tx = create_test_tx_with_validity(None, None);

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_err(),
            "InvalidBefore should fail when tx has no invalid_before"
        );
    }

    #[test]
    fn test_invalid_hereafter_satisfied() {
        let script = NativeScript::InvalidHereafter(200);
        let tx = create_test_tx_with_validity(None, Some(200));

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_ok(),
            "InvalidHereafter should pass when tx.invalid_hereafter <= script slot"
        );
    }

    #[test]
    fn test_invalid_hereafter_unsatisfied() {
        let script = NativeScript::InvalidHereafter(200);
        let tx = create_test_tx_with_validity(None, Some(300));

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_err(),
            "InvalidHereafter should fail when tx.invalid_hereafter > script slot"
        );
    }

    #[test]
    fn test_complex_nested_script() {
        // Complex script: (key1 AND key2) OR (2 of [key3, key4, key5])
        let (key1, _witness1) = create_vkey_witness(b"key1_test_bytes_32_characters!!");
        let (key2, _witness2) = create_vkey_witness(b"key2_test_bytes_32_characters!!");
        let (key3, witness3) = create_vkey_witness(b"key3_test_bytes_32_characters!!");
        let (key4, witness4) = create_vkey_witness(b"key4_test_bytes_32_characters!!");
        let (key5, _witness5) = create_vkey_witness(b"key5_test_bytes_32_characters!!");

        let script = NativeScript::ScriptAny(vec![
            NativeScript::ScriptAll(vec![
                NativeScript::ScriptPubkey(key1),
                NativeScript::ScriptPubkey(key2),
            ]),
            NativeScript::ScriptNOfK(
                2,
                vec![
                    NativeScript::ScriptPubkey(key3),
                    NativeScript::ScriptPubkey(key4),
                    NativeScript::ScriptPubkey(key5),
                ],
            ),
        ]);

        // Satisfy the second branch (2 of 3)
        let mut tx = create_test_tx_with_validity(None, None);
        tx.witness_set.vkey_witnesses.push(witness3);
        tx.witness_set.vkey_witnesses.push(witness4);

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_ok(),
            "Complex nested script should pass with valid branch"
        );
    }

    #[test]
    fn test_timelock_script() {
        // Script valid only between slots 100 and 200
        let (key, witness) = create_vkey_witness(b"key_test_bytes_32_characters!!!!");

        let script = NativeScript::ScriptAll(vec![
            NativeScript::ScriptPubkey(key),
            NativeScript::InvalidBefore(100),
            NativeScript::InvalidHereafter(200),
        ]);

        let mut tx = create_test_tx_with_validity(Some(100), Some(200));
        tx.witness_set.vkey_witnesses.push(witness);

        let result = MaryLedger::evaluate_native_script(&script, &tx);
        assert!(
            result.is_ok(),
            "Timelock script should pass within valid window"
        );
    }

    #[test]
    fn test_script_hashing_consistency() {
        let key = Blake2b256Hash::hash(b"test_key");
        let script = NativeScript::ScriptPubkey(key);

        let hash1 = MaryLedger::native_script_hash(&script);
        let hash2 = MaryLedger::native_script_hash(&script);

        assert_eq!(hash1, hash2, "Script hash should be deterministic");
    }

    #[test]
    fn test_script_hashing_different_scripts() {
        let key1 = Blake2b256Hash::hash(b"key1");
        let key2 = Blake2b256Hash::hash(b"key2");

        let script1 = NativeScript::ScriptPubkey(key1);
        let script2 = NativeScript::ScriptPubkey(key2);

        let hash1 = MaryLedger::native_script_hash(&script1);
        let hash2 = MaryLedger::native_script_hash(&script2);

        assert_ne!(
            hash1, hash2,
            "Different scripts should have different hashes"
        );
    }

    #[test]
    fn test_value_conservation_with_minting() {
        // Create a transaction that mints tokens
        let policy_id = PolicyId::new(Blake2b256Hash::hash(b"test_policy"));
        let asset_name = AssetName::from_string("TestToken").unwrap();

        let mut mint_assets = MultiAsset::new();
        let mut asset_map = HashMap::new();
        asset_map.insert(asset_name.clone(), 1000);
        mint_assets.assets.insert(policy_id.clone(), asset_map);

        // Create output with minted assets
        let mut output_value = MaryValue::new_ada_only(2_000_000);
        output_value.add_asset(policy_id, asset_name, 1000).unwrap();

        let tx = MaryTransaction {
            inputs: vec![],
            outputs: vec![MaryTransactionOutput {
                address: Address {
                    bytes: vec![1, 2, 3],
                },
                value: output_value,
                datum_hash: None,
            }],
            fee: 1000,
            ttl: None,
            certificates: vec![],
            withdrawals: HashMap::new(),
            auxiliary_data: None,
            validity_interval: ValidityInterval {
                invalid_before: None,
                invalid_hereafter: None,
            },
            mint: Some(Mint { mint_assets }),
            witness_set: MaryWitnessSet {
                vkey_witnesses: vec![],
                native_scripts: vec![],
                bootstrap_witnesses: vec![],
            },
        };

        let result = MaryLedger::validate_value_conservation(&tx);
        assert!(
            result.is_ok(),
            "Value conservation should pass for valid minting"
        );
    }
}
