//! Vendored cardano-vrf-pure modules
//!
//! This module contains select components from the cardano-base-rust project,
//! vendored directly into this crate for simplified dependency management.
//!
//! # Original Source
//! - Repository: <https://github.com/FractionEstate/cardano-base-rust>
//! - Package: cardano-vrf-pure v0.1.0
//! - License: Apache-2.0 OR MIT
//! - Authors: FractionEstate
//!
//! # Copyright Notice
//! Copyright (c) 2024 FractionEstate
//! Licensed under the Apache License, Version 2.0 or the MIT license.

use thiserror::Error;

/// VRF operation result type
pub type VrfResult<T> = Result<T, VrfError>;

/// VRF-specific errors
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum VrfError {
    /// Invalid proof provided
    #[error("Invalid VRF proof")]
    InvalidProof,

    /// Invalid public key
    #[error("Invalid public key")]
    InvalidPublicKey,

    /// Invalid secret key
    #[error("Invalid secret key")]
    InvalidSecretKey,

    /// Invalid point encoding
    #[error("Invalid point encoding")]
    InvalidPoint,

    /// Invalid scalar encoding
    #[error("Invalid scalar encoding")]
    InvalidScalar,

    /// Verification failed
    #[error("VRF verification failed")]
    VerificationFailed,
}

pub mod cardano_compat;
pub mod common;
pub mod draft13;
