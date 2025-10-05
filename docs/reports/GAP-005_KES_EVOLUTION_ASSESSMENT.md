# GAP-005: KES Key Evolution - Current Implementation Status

**Assessment Date**: October 2025
**Status**: ⚠️ **PARTIALLY IMPLEMENTED** (~60% Complete)
**Priority**: P1 (High) - Block Production Critical
**Estimated Completion Effort**: 1-2 weeks

---

## Executive Summary

GAP-005 was initially characterized as "Not Automatic" with manual KES evolution only. However, comprehensive codebase analysis reveals **significant existing implementation** of KES functionality:

- ✅ **Core crypto**: Full KES evolution logic (evolve, sign, verify)
- ✅ **Block forging**: Automatic in-memory evolution during block production
- ✅ **Configuration**: KesAutoRotation settings fully defined
- ✅ **Helper methods**: Period tracking, expiration checks, rotation logic
- ⚠️ **Missing**: Background monitoring service for proactive rotation
- ⚠️ **Missing**: File-based key persistence and operational cert generation
- ⚠️ **Missing**: Comprehensive alerting system

**Key Finding**: The core KES evolution functionality exists and works during block forging. What's missing is the "automation layer" - a background service that proactively monitors and rotates keys even when the node is not actively forging blocks.

---

## Current Implementation Analysis

### ✅ 1. Core KES Cryptography - COMPLETE

**Location**: `crates/cardano-crypto/src/kes/mod.rs` (713 lines)

**Implemented Features**:

```rust
pub struct KesSecretKey {
    root_public_key: Vec<u8>,
    current_key: Ed25519SigningKey,
    current_period: u64,
    max_period: u64,
    depth: u32,
}

impl KesSecretKey {
    /// Generate a new KES key at period 0
    pub fn generate(depth: u32) -> Self { /* ... */ }

    /// Evolve the key to the next period
    pub fn evolve(&self) -> Result<Self> { /* ... */ }

    /// Evolve to a specific period
    pub fn evolve_to(mut self, target_period: u64) -> Result<Self> { /* ... */ }

    /// Sign block header with KES key
    pub fn sign(&self, period: u64, message: &[u8]) -> Result<KesSignature> { /* ... */ }

    /// Get current period
    pub fn current_period(&self) -> u64 { /* ... */ }

    /// Get maximum period
    pub fn max_period(&self) -> u64 { /* ... */ }

    /// Check if key has expired
    pub fn is_expired(&self) -> bool { /* ... */ }
}
```

**Status**: ✅ **Fully functional** - Forward-secure evolution with proper period tracking

**Tests**: Unit tests cover evolution, expiration, and signature verification

---

### ✅ 2. Block Production Integration - COMPLETE

**Location**: `crates/cardano-consensus/src/block_production.rs` (lines 35-110)

**Implemented Features**:

```rust
pub struct KesKey {
    pub secret_key: KesSecretKey,
    pub max_period: u64,
}

impl KesKey {
    /// Create a new KES key with specified depth
    pub fn new(depth: u32) -> Self { /* ... */ }

    /// Check if KES key needs evolution
    pub fn needs_evolution(&self, current_period: u64) -> bool {
        current_period > self.secret_key.current_period()
    }

    /// Check if expired
    pub fn is_expired(&self) -> bool { /* ... */ }

    /// Get number of periods remaining until expiration
    pub fn periods_remaining(&self) -> u64 {
        self.max_period.saturating_sub(self.secret_key.current_period())
    }

    /// Check if key is approaching expiration (within threshold periods)
    pub fn is_approaching_expiration(&self, threshold_periods: u64) -> bool {
        self.periods_remaining() <= threshold_periods
    }

    /// Evolve KES key to new period
    pub fn evolve(&mut self, target_period: u64) -> Result<()> { /* ... */ }

    /// Sign block header with KES key
    pub fn sign_block(&self, header_bytes: &[u8]) -> Result<KesSignature> { /* ... */ }
}
```

**Status**: ✅ **Fully functional** - Complete wrapper with helper methods

---

### ✅ 3. Auto-Evolution During Block Forging - IMPLEMENTED

**Location**: `crates/cardano-consensus/src/block_forging.rs` (lines 120-260)

**Implemented Features**:

```rust
impl BlockForger {
    pub async fn forge_block(&mut self, context: ForgingContext) -> Result<ForgedBlock> {
        // Calculate KES period and evolve key if needed
        let kes_period = context.current_slot.0 / self.config.kes_period_length;

        if let Some((from, to)) = self.evolve_kes_if_needed(kes_period)? {
            tracing::info!("✓ KES key evolved from period {} to {}", from, to);

            // Check if approaching expiration and warn
            let remaining = self.kes_key.periods_remaining();
            if remaining <= 10 {
                tracing::warn!(
                    "⚠️  KES key approaching expiration! Only {} periods remaining. \
                     Generate new keys soon.",
                    remaining
                );
            }
        }

        // ... forge block with evolved key
    }

    fn evolve_kes_if_needed(&mut self, current_kes_period: u64)
        -> Result<Option<(u64, u64)>>
    {
        if self.kes_key.needs_evolution(current_kes_period) {
            let from_period = self.kes_key.current_period();

            // Check if key is expired before attempting evolution
            if self.kes_key.is_expired() {
                return Err(ConsensusError::KesKeyExpired(
                    format!("KES key expired at period {}", from_period)
                ));
            }

            // Evolve the KES key
            self.kes_key.evolve(current_kes_period)?;

            // Update operational certificate
            self.operational_cert.kes_period = current_kes_period;
            self.operational_cert.sequence_number += 1;

            return Ok(Some((from_period, current_kes_period)));
        }

        Ok(None)
    }
}
```

**Status**: ✅ **Working** - Automatic evolution during block forging with warnings

**Limitation**: Only evolves when forging blocks, not proactively in background

---

### ✅ 4. Configuration Support - IMPLEMENTED

**Location**: `crates/cardano-node/src/config/block_producer.rs` (lines 54-128)

**Implemented Structures**:

```rust
/// KES key configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KesKeyConfig {
    pub signing_key_file: PathBuf,
    pub verification_key_file: Option<PathBuf>,
    pub kes_period: u64,
    pub max_kes_evolutions: u64,
    pub start_kes_period: u64,
    pub format: KeyFormat,
    pub auto_rotation: Option<KesAutoRotation>,
}

/// KES key auto-rotation settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KesAutoRotation {
    /// Enable automatic KES key rotation
    pub enabled: bool,

    /// Rotate this many periods before expiration (safety margin)
    pub rotation_margin_periods: u64, // Default: 5

    /// Directory for storing rotated keys
    pub rotation_dir: PathBuf,

    /// Alert when this many periods remain before expiration
    pub alert_margin_periods: u64, // Default: 10
}

impl BlockProducerConfig {
    /// Get the current KES evolution number
    pub fn current_kes_evolution(&self) -> u64 {
        self.kes_key.kes_period
            .saturating_sub(self.kes_key.start_kes_period)
    }

    /// Check if KES key needs rotation
    pub fn needs_kes_rotation(&self) -> bool {
        if let Some(auto_rotation) = &self.kes_key.auto_rotation {
            if !auto_rotation.enabled {
                return false;
            }
            let remaining = self.kes_key.max_kes_evolutions
                .saturating_sub(self.current_kes_evolution());
            remaining <= auto_rotation.rotation_margin_periods
        } else {
            false
        }
    }

    /// Check if should alert about KES expiration
    pub fn should_alert_kes_expiration(&self) -> bool {
        if let Some(auto_rotation) = &self.kes_key.auto_rotation {
            let remaining = self.kes_key.max_kes_evolutions
                .saturating_sub(self.current_kes_evolution());
            remaining <= auto_rotation.alert_margin_periods
        } else {
            false
        }
    }
}
```

**Status**: ✅ **Complete** - Full configuration support with helper methods

**Tests**: Unit tests verify rotation check logic

---

### ✅ 5. KES Period Management - IMPLEMENTED

**Location**: `crates/cardano-consensus/src/ouroboros.rs` (lines 227-265)

**Implemented Features**:

```rust
pub struct KesManager {
    pub kes_period_length: u64, // Slots per KES period (default: 129600)
    pub max_kes_evolutions: u64,
}

impl KesManager {
    pub fn new(kes_period_length: u64, max_kes_evolutions: u64) -> Self {
        Self {
            kes_period_length,
            max_kes_evolutions,
        }
    }

    /// Calculate current KES period for slot
    pub fn kes_period_for_slot(&self, slot: SlotNo) -> u64 {
        slot.0 / self.kes_period_length
    }

    /// Validate KES signature period
    pub fn validate_kes_signature(
        &self,
        signature_kes_period: u64,
        current_slot: SlotNo,
        cert_kes_period: u64,
    ) -> Result<()> {
        let current_period = self.kes_period_for_slot(current_slot);

        // Check signature period matches cert period
        if signature_kes_period != cert_kes_period {
            return Err(ConsensusError::InvalidKesSignature(
                format!("KES period mismatch")
            ));
        }

        // Check period is not in the future
        if signature_kes_period > current_period {
            return Err(ConsensusError::InvalidKesSignature(
                format!("KES period from future")
            ));
        }

        Ok(())
    }
}
```

**Status**: ✅ **Complete** - Period calculation and validation

---

## Missing Implementation (GAP-005 Requirements)

### ❌ 1. Background KES Monitoring Service - NOT IMPLEMENTED

**What's Missing**:

A background service that:
- Runs independently of block forging
- Periodically checks current KES period vs key period
- Triggers alerts at `alert_margin_periods` threshold
- Triggers rotation at `rotation_margin_periods` threshold
- Handles graceful shutdown and restarts

**Required Implementation**:

```rust
// Location: crates/cardano-consensus/src/kes_evolution_service.rs (NEW FILE)

pub struct KesEvolutionService {
    config: KesAutoRotation,
    kes_key: Arc<RwLock<KesKey>>,
    current_slot: Arc<RwLock<SlotNo>>,
    kes_manager: KesManager,
    shutdown: CancellationToken,
}

impl KesEvolutionService {
    pub fn new(
        config: KesAutoRotation,
        kes_key: Arc<RwLock<KesKey>>,
        current_slot: Arc<RwLock<SlotNo>>,
        kes_manager: KesManager,
    ) -> Self { /* ... */ }

    /// Start monitoring service
    pub async fn start(&self) {
        let mut check_interval = tokio::time::interval(Duration::from_secs(3600)); // 1 hour

        loop {
            tokio::select! {
                _ = check_interval.tick() => {
                    if let Err(e) = self.check_and_evolve().await {
                        tracing::error!("KES evolution check failed: {}", e);
                    }
                }
                _ = self.shutdown.cancelled() => {
                    tracing::info!("KES evolution service shutting down");
                    break;
                }
            }
        }
    }

    async fn check_and_evolve(&self) -> Result<()> {
        let slot = *self.current_slot.read().await;
        let current_kes_period = self.kes_manager.kes_period_for_slot(slot);

        let mut kes_key = self.kes_key.write().await;
        let periods_remaining = kes_key.periods_remaining();

        // Alert if approaching expiration
        if periods_remaining <= self.config.alert_margin_periods {
            tracing::warn!(
                "⚠️  KES key approaching expiration! {} periods remaining",
                periods_remaining
            );
            self.emit_alert(periods_remaining).await?;
        }

        // Rotate if at rotation threshold
        if self.config.enabled && periods_remaining <= self.config.rotation_margin_periods {
            tracing::warn!(
                "🔄 KES key rotation triggered ({} periods remaining)",
                periods_remaining
            );
            self.rotate_kes_key(&mut kes_key, current_kes_period).await?;
        }

        Ok(())
    }

    async fn rotate_kes_key(
        &self,
        kes_key: &mut KesKey,
        current_kes_period: u64,
    ) -> Result<()> {
        // 1. Generate new KES key
        let new_key = KesSecretKey::generate(6); // depth=6 for mainnet

        // 2. Save old key to rotation_dir
        self.archive_old_key(kes_key).await?;

        // 3. Save new key to file
        self.save_new_key(&new_key, current_kes_period).await?;

        // 4. Generate new operational certificate (if cold key available)
        // Note: This requires cold key access or manual SPO intervention
        self.request_new_operational_cert(current_kes_period).await?;

        // 5. Update in-memory key
        *kes_key = KesKey {
            secret_key: new_key,
            max_period: 62, // 2^6 - 2
        };

        tracing::info!("✅ KES key rotation completed successfully");

        Ok(())
    }

    async fn emit_alert(&self, periods_remaining: u64) -> Result<()> {
        // Log to console
        tracing::warn!(
            "⚠️  KES KEY EXPIRATION ALERT: {} periods remaining",
            periods_remaining
        );

        // TODO: Optional external notifications
        // - Write to metrics/prometheus
        // - Send webhook notification
        // - Send email alert
        // - Update health status file

        Ok(())
    }
}
```

**Effort**: 3-4 days

---

### ❌ 2. File-Based Key Persistence - NOT IMPLEMENTED

**What's Missing**:

Functions to:
- Save rotated keys to `rotation_dir` with timestamps
- Load KES keys from Cardano CLI JSON format
- Generate new operational certificates
- Coordinate with cold key (if available)

**Required Implementation**:

```rust
impl KesEvolutionService {
    async fn archive_old_key(&self, kes_key: &KesKey) -> Result<()> {
        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("kes_old_{}.skey", timestamp);
        let path = self.config.rotation_dir.join(filename);

        // Create rotation directory if not exists
        tokio::fs::create_dir_all(&self.config.rotation_dir).await?;

        // Save key in Cardano CLI JSON format
        let key_json = serde_json::json!({
            "type": "KesSigningKey_ed25519_kes_2^6",
            "description": "KES Signing Key (archived)",
            "cborHex": hex::encode(kes_key.secret_key.to_bytes())
        });

        tokio::fs::write(path, serde_json::to_string_pretty(&key_json)?).await?;

        tracing::info!("Archived old KES key to rotation directory");
        Ok(())
    }

    async fn save_new_key(
        &self,
        new_key: &KesSecretKey,
        kes_period: u64,
    ) -> Result<()> {
        let key_json = serde_json::json!({
            "type": "KesSigningKey_ed25519_kes_2^6",
            "description": format!("KES Signing Key (period {})", kes_period),
            "cborHex": hex::encode(new_key.to_bytes())
        });

        // Write to temporary file first, then atomic rename
        let temp_path = self.config.signing_key_file.with_extension("tmp");
        tokio::fs::write(&temp_path, serde_json::to_string_pretty(&key_json)?).await?;
        tokio::fs::rename(temp_path, &self.config.signing_key_file).await?;

        tracing::info!("Saved new KES key to {}", self.config.signing_key_file.display());
        Ok(())
    }

    async fn request_new_operational_cert(&self, kes_period: u64) -> Result<()> {
        // Check if cold key is available for automatic cert generation
        if let Some(cold_key_path) = &self.cold_key_config {
            // Generate new operational certificate automatically
            self.generate_operational_cert(cold_key_path, kes_period).await?;
        } else {
            // Cold key not available - alert SPO to manually generate cert
            tracing::warn!(
                "⚠️  NEW OPERATIONAL CERTIFICATE REQUIRED!\n\
                 \n\
                 Run the following commands:\n\
                 1. cardano-cli node key-gen-KES --verification-key-file kes-new.vkey \\\n\
                    --signing-key-file kes-new.skey\n\
                 2. cardano-cli node issue-op-cert --kes-verification-key-file kes-new.vkey \\\n\
                    --cold-signing-key-file cold.skey \\\n\
                    --operational-certificate-issue-counter cold.counter \\\n\
                    --kes-period {} --out-file node-new.cert\n\
                 3. Update configuration and restart node",
                kes_period
            );

            // Write instructions to file for SPO
            let instructions_path = self.config.rotation_dir.join("ROTATION_REQUIRED.txt");
            let instructions = format!(
                "KES Key Rotation Required\n\
                 ========================\n\
                 Date: {}\n\
                 KES Period: {}\n\
                 \n\
                 Steps:\n\
                 1. Generate new operational certificate using the new KES key\n\
                 2. Update node configuration with new cert\n\
                 3. Restart node\n",
                chrono::Utc::now(),
                kes_period
            );
            tokio::fs::write(instructions_path, instructions).await?;
        }

        Ok(())
    }
}
```

**Effort**: 2-3 days

---

### ❌ 3. Comprehensive Alerting System - PARTIAL

**What Exists**:
- Basic warning logs when <10 periods remain (during block forging only)

**What's Missing**:
- Proactive alerts at configurable thresholds
- Multiple alert levels (info, warning, critical)
- External notification support (metrics, webhooks, email)
- Health status file for monitoring

**Required Implementation**:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AlertLevel {
    Info,    // >20 periods
    Warning, // 10-20 periods
    Critical, // <10 periods
}

impl KesEvolutionService {
    fn determine_alert_level(&self, periods_remaining: u64) -> AlertLevel {
        if periods_remaining <= 10 {
            AlertLevel::Critical
        } else if periods_remaining <= 20 {
            AlertLevel::Warning
        } else {
            AlertLevel::Info
        }
    }

    async fn emit_comprehensive_alert(
        &self,
        periods_remaining: u64,
        level: AlertLevel,
    ) -> Result<()> {
        match level {
            AlertLevel::Critical => {
                tracing::error!(
                    "🚨 CRITICAL: KES key expiring soon! {} periods remaining",
                    periods_remaining
                );
            }
            AlertLevel::Warning => {
                tracing::warn!(
                    "⚠️  WARNING: KES key approaching expiration. {} periods remaining",
                    periods_remaining
                );
            }
            AlertLevel::Info => {
                tracing::info!(
                    "ℹ️  INFO: KES key status check. {} periods remaining",
                    periods_remaining
                );
            }
        }

        // Export metrics (if enabled)
        if let Some(metrics) = &self.metrics {
            metrics.set_kes_periods_remaining(periods_remaining);
            metrics.set_kes_alert_level(level as i64);
        }

        // Write health status file
        self.write_health_status(periods_remaining, level).await?;

        // Optional: External notifications
        if level == AlertLevel::Critical {
            self.send_external_notification(periods_remaining).await?;
        }

        Ok(())
    }

    async fn write_health_status(
        &self,
        periods_remaining: u64,
        level: AlertLevel,
    ) -> Result<()> {
        let status = serde_json::json!({
            "component": "kes_key",
            "status": match level {
                AlertLevel::Info => "healthy",
                AlertLevel::Warning => "warning",
                AlertLevel::Critical => "critical",
            },
            "periods_remaining": periods_remaining,
            "last_checked": chrono::Utc::now().to_rfc3339(),
        });

        let health_file = self.config.rotation_dir.join("kes_health.json");
        tokio::fs::write(health_file, serde_json::to_string_pretty(&status)?).await?;

        Ok(())
    }
}
```

**Effort**: 1-2 days

---

### ❌ 4. Node Lifecycle Integration - NOT IMPLEMENTED

**What's Missing**:
- Start `KesEvolutionService` on node startup
- Pass shared state between service and block forger
- Graceful shutdown handling
- Configuration validation

**Required Implementation**:

```rust
// Location: crates/cardano-node/src/main.rs (modifications)

async fn start_node(config: NodeConfig) -> Result<()> {
    // ... existing node initialization ...

    // Initialize KES evolution service (if block producer enabled)
    let kes_service = if config.block_producer.enabled {
        if let Some(auto_rotation) = &config.block_producer.kes_key.auto_rotation {
            if auto_rotation.enabled {
                let service = KesEvolutionService::new(
                    auto_rotation.clone(),
                    kes_key_shared.clone(),
                    current_slot_shared.clone(),
                    kes_manager.clone(),
                );

                // Spawn service in background
                tokio::spawn(async move {
                    service.start().await;
                });

                tracing::info!("✓ KES evolution service started");
                Some(service)
            } else {
                tracing::info!("KES auto-rotation disabled");
                None
            }
        } else {
            None
        }
    } else {
        None
    };

    // ... rest of node initialization ...

    // Wait for shutdown signal
    shutdown_signal.await;

    // Graceful shutdown
    if let Some(service) = kes_service {
        service.shutdown().await;
    }

    Ok(())
}
```

**Effort**: 1 day

---

## Testing Requirements

### Unit Tests Needed:

1. **KesEvolutionService Tests**:
   - Period monitoring logic
   - Alert trigger thresholds
   - Rotation trigger conditions
   - File I/O operations
   - Error handling

2. **Configuration Tests**:
   - Parsing KesAutoRotation from JSON
   - Default value validation
   - needs_kes_rotation() edge cases
   - should_alert_kes_expiration() thresholds

### Integration Tests Needed:

1. **Full Rotation Cycle Test**:
   - Start with key at period 0
   - Fast-forward to rotation_margin_periods threshold
   - Verify rotation triggers
   - Verify new key saved
   - Verify old key archived
   - Verify operational cert request

2. **Alert System Test**:
   - Trigger alerts at various thresholds
   - Verify log messages
   - Verify health status file updates
   - Verify metrics (if enabled)

3. **Block Forging with Service Test**:
   - Run service and block forger concurrently
   - Verify both can evolve key safely
   - Verify synchronization via Arc<RwLock>

**Effort**: 2-3 days

---

## Implementation Plan

### Phase 1: Core Service (Week 1, Days 1-3)

**Tasks**:
1. Create `kes_evolution_service.rs` module
2. Implement `KesEvolutionService` struct with monitoring loop
3. Implement alert logic at configurable thresholds
4. Add basic logging and health status file

**Deliverables**:
- Working background monitoring service
- Alert triggers at alert_margin_periods
- Health status JSON file

**Tests**:
- Unit tests for alert logic
- Mock time advancement tests

### Phase 2: Key Rotation (Week 1, Days 4-5)

**Tasks**:
1. Implement file-based key archival
2. Implement new key generation and saving
3. Add operational cert generation (if cold key available)
4. Add SPO instructions file for manual cert generation

**Deliverables**:
- Complete rotation workflow
- Keys saved in Cardano CLI JSON format
- Clear SPO instructions for manual steps

**Tests**:
- Integration test for full rotation cycle
- File I/O tests

### Phase 3: Integration & Alerting (Week 2, Days 1-2)

**Tasks**:
1. Wire service into node startup (main.rs)
2. Add graceful shutdown handling
3. Implement comprehensive alerting (logs, metrics, external)
4. Add configuration validation

**Deliverables**:
- Service runs with node lifecycle
- Multiple alert levels (info, warning, critical)
- Optional external notifications

**Tests**:
- Integration tests with full node
- Alert system tests

### Phase 4: Testing & Documentation (Week 2, Days 3-5)

**Tasks**:
1. Create comprehensive test suite
2. Test with real KES period transitions
3. Document configuration options
4. Update HASKELL_COMPATIBILITY_GAPS.md
5. Create operator guide for KES rotation

**Deliverables**:
- 90%+ test coverage
- Operator documentation
- GAP-005 closure report

**Tests**:
- All unit and integration tests passing
- Manual testing with real keys

---

## Success Criteria

### Must Have (Required for GAP-005 Closure):

- ✅ Background service monitors KES periods independently
- ✅ Alerts trigger at configurable thresholds
- ✅ Automatic key rotation at rotation_margin_periods
- ✅ Old keys archived to rotation_dir
- ✅ New keys saved in Cardano CLI format
- ✅ Service integrated with node lifecycle
- ✅ Graceful shutdown handling
- ✅ Comprehensive test coverage (>85%)
- ✅ Operator documentation

### Nice to Have (Post-GAP-005):

- Metrics export (Prometheus)
- External notifications (webhooks, email)
- Automatic operational cert generation (requires cold key integration)
- Dashboard integration
- Multi-key rotation scheduling

---

## Risk Assessment

### Low Risk ✅

- **Core crypto**: Already implemented and tested
- **Block forging**: Evolution works during forging
- **Configuration**: Structures defined and tested

### Medium Risk 🟡

- **File I/O**: Key archival and saving
  - Mitigation: Atomic writes, comprehensive error handling

- **Concurrency**: Service and block forger share KES key
  - Mitigation: Use Arc<RwLock<KesKey>> for safe sharing

- **Operational cert**: May require manual SPO intervention
  - Mitigation: Clear instructions, optional automation

### Managed Risk 🟢

- **Testing**: Need real KES period transitions
  - Mitigation: Mock time for fast testing, manual verification

---

## Recommendation

**Status**: GAP-005 is **60% complete** - significantly more advanced than gap document suggests.

**Priority**: P1 (High) - Correct severity. Missing rotation can cause block production failures for SPOs.

**Effort**: 1-2 weeks (not 2-3 days as initially estimated)
- Week 1: Core service + rotation workflow
- Week 2: Integration + testing + documentation

**Next Steps**:
1. Create `kes_evolution_service.rs` module
2. Implement background monitoring service
3. Add file-based rotation workflow
4. Wire into node lifecycle
5. Comprehensive testing

**Dependencies**:
- None (all required components exist)

**Impact**:
- +2% compatibility when complete (95% → 97%)
- Production readiness for SPOs
- Reduced operational burden (no manual rotation)

---

## References

### Existing Implementation

- **Core Crypto**: `crates/cardano-crypto/src/kes/mod.rs` (713 lines)
- **Block Production**: `crates/cardano-consensus/src/block_production.rs` (lines 35-110)
- **Block Forging**: `crates/cardano-consensus/src/block_forging.rs` (lines 120-260)
- **Configuration**: `crates/cardano-node/src/config/block_producer.rs` (lines 54-128)
- **KES Manager**: `crates/cardano-consensus/src/ouroboros.rs` (lines 227-265)

### Related Documentation

- **Block Producer Config**: `crates/cardano-node/config/block-producer/README.md`
- **KES Overview**: `crates/cardano-crypto/src/kes/mod.rs` (documentation comments)

---

**Assessment Prepared By**: Development Team
**Date**: October 2025
**Status**: ⚠️ **PARTIALLY IMPLEMENTED** - Clear path to completion
