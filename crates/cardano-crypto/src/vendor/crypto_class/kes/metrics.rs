//! Feature-gated lightweight instrumentation for KES operations.
//!
//! Enabled via the `kes-metrics` crate feature. Provides approximate counters for
//! key generation, signing, signature bytes, and key evolution updates without
//! introducing any OS-specific probes or unsafe memory introspection. Intended
//! strictly for benchmarking / diagnostic builds.
//!
//! Counters are global and monotonic for the lifetime of the process. They are
//! deliberately relaxed-order to minimise overhead.

#[cfg(feature = "kes-metrics")]
use core::sync::atomic::{AtomicU64, Ordering};

/// Snapshot of KES metrics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KesMetrics {
    pub signing_keys: u64,
    pub signing_key_bytes: u64,
    pub signatures: u64,
    pub signature_bytes: u64,
    pub updates: u64,
}

#[cfg(feature = "kes-metrics")]
static SIGNING_KEYS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "kes-metrics")]
static SIGNING_KEY_BYTES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "kes-metrics")]
static SIGNATURES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "kes-metrics")]
static SIGNATURE_BYTES: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "kes-metrics")]
static UPDATES: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "kes-metrics")]
#[inline]
pub(crate) fn record_signing_key(size: usize) {
    SIGNING_KEYS.fetch_add(1, Ordering::Relaxed);
    SIGNING_KEY_BYTES.fetch_add(size as u64, Ordering::Relaxed);
}

#[cfg(feature = "kes-metrics")]
#[inline]
pub(crate) fn record_signature(size: usize) {
    SIGNATURES.fetch_add(1, Ordering::Relaxed);
    SIGNATURE_BYTES.fetch_add(size as u64, Ordering::Relaxed);
}

#[cfg(feature = "kes-metrics")]
#[inline]
pub(crate) fn record_update() {
    UPDATES.fetch_add(1, Ordering::Relaxed);
}

/// Obtain a metrics snapshot. With the feature disabled this returns zeros.
#[inline]
#[must_use]
pub fn snapshot() -> KesMetrics {
    #[cfg(feature = "kes-metrics")]
    {
        KesMetrics {
            signing_keys: SIGNING_KEYS.load(Ordering::Relaxed),
            signing_key_bytes: SIGNING_KEY_BYTES.load(Ordering::Relaxed),
            signatures: SIGNATURES.load(Ordering::Relaxed),
            signature_bytes: SIGNATURE_BYTES.load(Ordering::Relaxed),
            updates: UPDATES.load(Ordering::Relaxed),
        }
    }
    #[cfg(not(feature = "kes-metrics"))]
    {
        KesMetrics::default()
    }
}
