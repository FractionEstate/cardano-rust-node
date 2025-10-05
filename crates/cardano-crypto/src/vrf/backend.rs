//! VRF backend implementation using cardano-crypto-class
//!
//! This module provides C-style interface functions that wrap the production-ready
//! VRF implementation from cardano-crypto-class (PraosBatchCompatVRF with Draft-13).
//!
//! Uses batch-compatible VRF with 128-byte proofs for optimal performance and
//! compatibility with Cardano mainnet/testnet.

use cardano_crypto_class::seed::mk_seed_from_bytes;
use cardano_crypto_class::vrf::praos_batch::PraosBatchCompatVRF;
use cardano_crypto_class::vrf::VRFAlgorithm;
use rand_core::{OsRng, RngCore};
use zeroize::Zeroize;

const VRF_PUBLIC_KEY_LENGTH: usize = 32;
const VRF_SECRET_KEY_LENGTH: usize = 64;
const VRF_SEED_LENGTH: usize = 32;
const VRF_PROOF_LENGTH: usize = 128; // Draft-13 batch-compatible uses 128-byte proofs
const VRF_OUTPUT_LENGTH: usize = 64;

pub fn vrf_public_key_bytes() -> usize {
    VRF_PUBLIC_KEY_LENGTH
}

pub fn vrf_secret_key_bytes() -> usize {
    VRF_SECRET_KEY_LENGTH
}

pub fn vrf_seed_bytes() -> usize {
    VRF_SEED_LENGTH
}

pub fn vrf_proof_bytes() -> usize {
    VRF_PROOF_LENGTH
}

pub fn vrf_output_bytes() -> usize {
    VRF_OUTPUT_LENGTH
}

pub fn ensure_backend_init() -> Result<(), &'static str> {
    // Pure Rust backend does not require runtime initialisation.
    Ok(())
}

pub fn random_keypair(pk: &mut [u8], sk: &mut [u8]) -> i32 {
    if pk.len() != VRF_PUBLIC_KEY_LENGTH || sk.len() != VRF_SECRET_KEY_LENGTH {
        return -1;
    }

    let mut seed = [0u8; VRF_SEED_LENGTH];
    OsRng.fill_bytes(&mut seed);
    let result = seed_keypair(pk, sk, &seed);
    seed.zeroize();
    result
}

pub fn seed_keypair(pk: &mut [u8], sk: &mut [u8], seed: &[u8]) -> i32 {
    if pk.len() != VRF_PUBLIC_KEY_LENGTH
        || sk.len() != VRF_SECRET_KEY_LENGTH
        || seed.len() != VRF_SEED_LENGTH
    {
        return -1;
    }

    let mut seed_array = [0u8; VRF_SEED_LENGTH];
    seed_array.copy_from_slice(seed);

    // Use PraosBatchCompatVRF::gen_keypair for production-ready batch-compatible VRF
    let seed_obj = mk_seed_from_bytes(&seed_array);
    let (signing_key, verification_key) = PraosBatchCompatVRF::gen_keypair(&seed_obj);

    // Serialize keys using VRFAlgorithm trait methods
    let sk_bytes = PraosBatchCompatVRF::raw_serialize_signing_key(&signing_key);
    let vk_bytes = PraosBatchCompatVRF::raw_serialize_verification_key(&verification_key);

    if sk_bytes.len() != VRF_SECRET_KEY_LENGTH || vk_bytes.len() != VRF_PUBLIC_KEY_LENGTH {
        seed_array.zeroize();
        return -1;
    }

    pk.copy_from_slice(&vk_bytes);
    sk.copy_from_slice(&sk_bytes);

    seed_array.zeroize();

    0
}

pub fn sk_to_pk(pk: &mut [u8], sk: &[u8]) {
    if pk.len() == VRF_PUBLIC_KEY_LENGTH && sk.len() == VRF_SECRET_KEY_LENGTH {
        pk.copy_from_slice(&sk[VRF_SEED_LENGTH..]);
    }
}

pub fn sk_to_seed(seed: &mut [u8], sk: &[u8]) {
    if seed.len() == VRF_SEED_LENGTH && sk.len() == VRF_SECRET_KEY_LENGTH {
        seed.copy_from_slice(&sk[..VRF_SEED_LENGTH]);
    }
}

pub fn prove(proof: &mut [u8], sk: &[u8], msg: &[u8]) -> i32 {
    if proof.len() != VRF_PROOF_LENGTH || sk.len() != VRF_SECRET_KEY_LENGTH {
        return -1;
    }

    // Deserialize signing key
    let signing_key = match PraosBatchCompatVRF::raw_deserialize_signing_key(sk) {
        Some(key) => key,
        None => return -1,
    };

    // Use VRFAlgorithm::evaluate_bytes for batch-compatible proof generation
    let (_output, vrf_proof) = PraosBatchCompatVRF::evaluate_bytes(&(), msg, &signing_key);

    // Serialize proof
    let proof_bytes = PraosBatchCompatVRF::raw_serialize_proof(&vrf_proof);

    if proof_bytes.len() != VRF_PROOF_LENGTH {
        return -1;
    }

    proof.copy_from_slice(&proof_bytes);
    0
}

pub fn verify(output: &mut [u8], pk: &[u8], proof: &[u8], msg: &[u8]) -> i32 {
    if output.len() != VRF_OUTPUT_LENGTH
        || pk.len() != VRF_PUBLIC_KEY_LENGTH
        || proof.len() != VRF_PROOF_LENGTH
    {
        return -1;
    }

    // Deserialize verification key
    let verification_key = match PraosBatchCompatVRF::raw_deserialize_verification_key(pk) {
        Some(key) => key,
        None => return -1,
    };

    // Deserialize proof
    let vrf_proof = match PraosBatchCompatVRF::raw_deserialize_proof(proof) {
        Some(p) => p,
        None => return -1,
    };

    // Use VRFAlgorithm::verify_bytes for batch-compatible verification
    match PraosBatchCompatVRF::verify_bytes(&(), &verification_key, msg, &vrf_proof) {
        Some(vrf_output) => {
            // Extract output bytes
            let output_bytes = vrf_output.as_bytes();
            if output_bytes.len() != VRF_OUTPUT_LENGTH {
                return -1;
            }
            output.copy_from_slice(output_bytes);
            0
        }
        None => -1,
    }
}

pub fn proof_to_hash(hash: &mut [u8], proof: &[u8]) -> i32 {
    if hash.len() != VRF_OUTPUT_LENGTH || proof.len() != VRF_PROOF_LENGTH {
        return -1;
    }

    // Deserialize proof
    let vrf_proof = match PraosBatchCompatVRF::raw_deserialize_proof(proof) {
        Some(p) => p,
        None => return -1,
    };

    // Extract output from proof using cardano-crypto-class helper
    use cardano_crypto_class::vrf::praos_batch::output_from_proof;
    match output_from_proof(&vrf_proof) {
        Ok(Some(vrf_output)) => {
            let output_bytes = vrf_output.as_bytes();
            if output_bytes.len() != VRF_OUTPUT_LENGTH {
                return -1;
            }
            hash.copy_from_slice(output_bytes);
            0
        }
        Ok(None) => -1,
        Err(_) => -1,
    }
}
