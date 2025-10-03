//! VRF backend implementation using cardano-vrf-pure
//!
//! This module provides C-style interface functions that wrap the pure Rust
//! VRF implementation from cardano-vrf-pure (VrfDraft03).

use cardano_vrf_pure::draft03::VrfDraft03;
use rand_core::{OsRng, RngCore};
use zeroize::Zeroize;

const VRF_PUBLIC_KEY_LENGTH: usize = 32;
const VRF_SECRET_KEY_LENGTH: usize = 64;
const VRF_SEED_LENGTH: usize = 32;
const VRF_PROOF_LENGTH: usize = 80;
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

    // Use VrfDraft03::keypair_from_seed
    let (secret_key, public_key) = VrfDraft03::keypair_from_seed(&seed_array);

    pk.copy_from_slice(&public_key);
    sk.copy_from_slice(&secret_key);

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

    let mut secret_key = [0u8; VRF_SECRET_KEY_LENGTH];
    secret_key.copy_from_slice(sk);

    // Use VrfDraft03::prove
    match VrfDraft03::prove(&secret_key, msg) {
        Ok(proof_bytes) => {
            proof.copy_from_slice(&proof_bytes);
            secret_key.zeroize();
            0
        }
        Err(_) => {
            secret_key.zeroize();
            -1
        }
    }
}

pub fn verify(output: &mut [u8], pk: &[u8], proof: &[u8], msg: &[u8]) -> i32 {
    if output.len() != VRF_OUTPUT_LENGTH
        || pk.len() != VRF_PUBLIC_KEY_LENGTH
        || proof.len() != VRF_PROOF_LENGTH
    {
        return -1;
    }

    let mut public_key = [0u8; VRF_PUBLIC_KEY_LENGTH];
    public_key.copy_from_slice(pk);

    let mut proof_array = [0u8; VRF_PROOF_LENGTH];
    proof_array.copy_from_slice(proof);

    // Use VrfDraft03::verify
    match VrfDraft03::verify(&public_key, &proof_array, msg) {
        Ok(output_bytes) => {
            output.copy_from_slice(&output_bytes);
            0
        }
        Err(_) => -1,
    }
}

pub fn proof_to_hash(hash: &mut [u8], proof: &[u8]) -> i32 {
    if hash.len() != VRF_OUTPUT_LENGTH || proof.len() != VRF_PROOF_LENGTH {
        return -1;
    }

    let mut proof_array = [0u8; VRF_PROOF_LENGTH];
    proof_array.copy_from_slice(proof);

    // Use VrfDraft03::proof_to_hash
    match VrfDraft03::proof_to_hash(&proof_array) {
        Ok(output_bytes) => {
            hash.copy_from_slice(&output_bytes);
            0
        }
        Err(_) => -1,
    }
}
