//! Internal debugging utilities for Cardano compatibility layer.
//!
//! Logging is disabled by default to keep the cryptographic core silent.
//! Enable the `vrf-debug` feature (and optionally set the `CARDANO_VRF_DEBUG`
//! environment variable) to surface diagnostic output during development or
//! advanced troubleshooting.

#[cfg(feature = "vrf-debug")]
use std::sync::OnceLock;

#[cfg(feature = "vrf-debug")]
fn is_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("CARDANO_VRF_DEBUG").is_ok())
}

#[cfg(not(feature = "vrf-debug"))]
#[inline(always)]
fn is_enabled() -> bool {
    false
}

/// Emit a lazily constructed debug message when VRF debugging is enabled.
#[inline(always)]
pub fn log<F>(message: F)
where
    F: FnOnce() -> String,
{
    if is_enabled() {
        eprintln!("{}", message());
    }
}
