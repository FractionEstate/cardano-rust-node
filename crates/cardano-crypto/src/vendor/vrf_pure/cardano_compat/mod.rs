//! Cardano-specific VRF implementation module
//!
//! This module provides a pure Rust implementation that matches Cardano's libsodium
//! VRF implementation byte-for-byte. The implementation is organized into several
//! submodules for maintainability and testability.
//!
//! # Module Organization
//!
//! - `field` - Field element operations for Curve25519 (GF(2^255-19))
//! - `montgomery` - Montgomery curve operations including Elligator2
//! - `point` - Edwards point operations and coordinate conversions
//! - `prove` - VRF proof generation
//! - `verify` - VRF proof verification
//!
//! # Compatibility
//!
//! This implementation is designed to produce identical outputs to the C implementation
//! in cardano-crypto-praos/cbits/vrf03/ for all inputs. Every operation has been
//! carefully matched to the reference implementation.

mod debug;
pub mod field;
pub mod montgomery;
pub mod point;
pub mod prove;
pub mod verify;

// Re-export main API
pub use prove::cardano_vrf_prove;
pub use verify::cardano_vrf_verify;

#[cfg(test)]
mod tests;
