//! VRF backend implementation using cardano-crypto-class
//!
//! This module provides C-style interface functions that wrap the production-ready
//! VRF implementation from cardano-crypto-class (PraosBatchCompatVRF with Draft-13).
//!
//! Uses batch-compatible VRF with 128-byte proofs for optimal performance and
//! compatibility with Cardano mainnet/testnet.

use crate::vendor::vrf_pure::common::secret_key_to_public;
use crate::vendor::vrf_pure::draft13::{
    VrfDraft13, OUTPUT_SIZE as DRAFT13_OUTPUT_LENGTH, PROOF_SIZE as DRAFT13_PROOF_LENGTH,
    PUBLIC_KEY_SIZE as DRAFT13_PUBLIC_KEY_LENGTH, SECRET_KEY_SIZE as DRAFT13_SECRET_KEY_LENGTH,
    SEED_SIZE as DRAFT13_SEED_LENGTH,
};
use core::convert::TryInto;
use rand_core::{OsRng, RngCore};
use zeroize::Zeroize;

const VRF_PUBLIC_KEY_LENGTH: usize = DRAFT13_PUBLIC_KEY_LENGTH;
const VRF_SECRET_KEY_LENGTH: usize = DRAFT13_SECRET_KEY_LENGTH;
const VRF_SEED_LENGTH: usize = DRAFT13_SEED_LENGTH;
const VRF_PROOF_LENGTH: usize = DRAFT13_PROOF_LENGTH; // Draft-13 batch-compatible uses 128-byte proofs
const VRF_OUTPUT_LENGTH: usize = DRAFT13_OUTPUT_LENGTH;

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

    let (sk_bytes, vk_bytes) = VrfDraft13::keypair_from_seed(&seed_array);

    pk.copy_from_slice(&vk_bytes);
    sk.copy_from_slice(&sk_bytes);

    seed_array.zeroize();

    0
}

pub fn sk_to_pk(pk: &mut [u8], sk: &[u8]) {
    if pk.len() == VRF_PUBLIC_KEY_LENGTH && sk.len() == VRF_SECRET_KEY_LENGTH {
        let mut sk_array = [0u8; VRF_SECRET_KEY_LENGTH];
        sk_array.copy_from_slice(sk);
        let derived_pk = secret_key_to_public(&sk_array);
        pk.copy_from_slice(&derived_pk);
        sk_array.zeroize();
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

    let mut sk_array = [0u8; VRF_SECRET_KEY_LENGTH];
    sk_array.copy_from_slice(sk);

    let result = VrfDraft13::prove(&sk_array, msg);
    sk_array.zeroize();

    match result {
        Ok(proof_bytes) => {
            proof.copy_from_slice(&proof_bytes);
            0
        }
        Err(_) => -1,
    }
}

pub fn verify(output: &mut [u8], pk: &[u8], proof: &[u8], msg: &[u8]) -> i32 {
    if output.len() != VRF_OUTPUT_LENGTH
        || pk.len() != VRF_PUBLIC_KEY_LENGTH
        || proof.len() != VRF_PROOF_LENGTH
    {
        return -1;
    }

    let pk_array: [u8; VRF_PUBLIC_KEY_LENGTH] = match pk.try_into() {
        Ok(arr) => arr,
        Err(_) => return -1,
    };

    let proof_array: [u8; VRF_PROOF_LENGTH] = match proof.try_into() {
        Ok(arr) => arr,
        Err(_) => return -1,
    };

    match VrfDraft13::verify(&pk_array, &proof_array, msg) {
        Ok(vrf_output) => {
            output.copy_from_slice(&vrf_output);
            0
        }
        Err(_) => -1,
    }
}

pub fn proof_to_hash(hash: &mut [u8], proof: &[u8]) -> i32 {
    if hash.len() != VRF_OUTPUT_LENGTH || proof.len() != VRF_PROOF_LENGTH {
        return -1;
    }

    let proof_array: [u8; VRF_PROOF_LENGTH] = match proof.try_into() {
        Ok(arr) => arr,
        Err(_) => return -1,
    };

    match VrfDraft13::proof_to_hash(&proof_array) {
        Ok(beta) => {
            hash.copy_from_slice(&beta);
            0
        }
        Err(_) => -1,
    }
}
