//! Vendored cardano-crypto-class modules
//!
//! This module contains select components from the cardano-base-rust project,
//! vendored directly into this crate for simplified dependency management.
//!
//! # Original Source
//! - Repository: <https://github.com/FractionEstate/cardano-base-rust>
//! - Package: cardano-crypto-class v0.1.0
//! - License: Apache-2.0 OR MIT
//! - Authors: FractionEstate
//!
//! # Modifications
//! - Added `DirectSerialise`/`DirectDeserialise` implementations for `Vec<u8>`
//!   to support CompactSumKES and SumKES hash-based verification keys
//!
//! # Copyright Notice
//! Copyright (c) 2024 FractionEstate
//! Licensed under the Apache License, Version 2.0 or the MIT license.

pub mod direct_serialise;
pub mod dsign;
pub mod ffi;
pub mod hash;
pub mod kes;
pub mod mlocked_bytes;
pub mod mlocked_seed;
pub mod packed_bytes;
pub mod pinned_sized_bytes;
pub mod seed;
pub mod util;
