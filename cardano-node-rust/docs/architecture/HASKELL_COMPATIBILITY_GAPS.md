# Haskell Cardano-Node Compatibility Analysis

## ⚠️ CRITICAL FINDINGS: PRODUCTION-BLOCKING INCOMPATIBILITIES DISCOVERED

**Status**: The Rust implementation is **NOT compatible** with the official Haskell cardano-node (v10.5.1)
**Impact**: Cannot connect to mainnet/testnet networks without significant changes
**Reference**: [IntersectMBO/cardano-node](https://github.com/IntersectMBO/cardano-node)

---

## 1. Configuration File Incompatibilities

### 1.1 config.json Structure

**Official Haskell Schema** (60+ fields):
```json
{
  "AlonzoGenesisFile": "alonzo-genesis.json",
  "AlonzoGenesisHash": "7e94a15f55d1e82d10f09203fa1d40f8eede58fd8066542cf6566008068ed874",
  "ByronGenesisFile": "byron-genesis.json",
  "ByronGenesisHash": "5f20df933584822601f9e3f8c024eb5eb252fe8cefb24d1317dc3d432e940ebb",
  "CheckpointsFile": "checkpoints.json",
  "CheckpointsFileHash": "3e6dee5bae7acc6d870187e72674b37c929be8c66e62a552cf6a876b1af31ade",
  "ConsensusMode": "PraosMode",
  "ConwayGenesisFile": "conway-genesis.json",
  "ConwayGenesisHash": "15a199f895e461ec0ffc6dd4e4028af28a492ab4e806d39cb674c88f7643ef62",
  "EnableP2P": true,
  "LastKnownBlockVersion-Alt": 0,
  "LastKnownBlockVersion-Major": 3,
  "LastKnownBlockVersion-Minor": 0,
  "LedgerDB": {
    "Backend": "V2InMemory",
    "NumOfDiskSnapshots": 2,
    "QueryBatchSize": 100000,
    "SnapshotInterval": 4320
  },
  "MaxKnownMajorProtocolVersion": 2,
  "MinNodeVersion": "10.4.0",
  "PeerSharing": true,
  "Protocol": "Cardano",
  "RequiresNetworkMagic": "RequiresNoMagic",
  "ShelleyGenesisFile": "shelley-genesis.json",
  "ShelleyGenesisHash": "1a3be38bcbb7911969283716ad7aa550250226b76a61fc51cc9a9a35d9276d81",
  "TraceBlockFetchClient": false,
  "TraceChainDb": true,
  "TraceChainSyncClient": false,
  "TraceConnectionManager": true,
  // ... 40+ additional trace flags
  "TracingVerbosity": "NormalVerbosity",
  "TurnOnLogMetrics": true,
  "TurnOnLogging": true,
  "hasEKG": 12788,
  "hasPrometheus": ["127.0.0.1", 12798],
  "minSeverity": "Info",
  "options": { /* mapBackends, mapSubtrace */ },
  "rotation": { /* log rotation settings */ }
}
```

**Current Rust Schema** (10 fields):
```rust
pub struct NodeConfiguration {
    pub network_magic: u32,
    pub listening_port: u16,
    pub database_path: PathBuf,
    pub socket_path: PathBuf,
    pub enable_logging: bool,
    pub max_connections: u32,
    pub enable_metrics: bool,
    pub metrics_port: u16,
    pub topology_file: PathBuf,
}
```

**Missing Critical Fields**:
- ❌ Genesis file paths and hashes (Byron, Shelley, Alonzo, Conway)
- ❌ `CheckpointsFile` and hash validation
- ❌ `ConsensusMode` (PraosMode/GenesisMode)
- ❌ `EnableP2P` flag
- ❌ `PeerSharing` settings
- ❌ `LastKnownBlockVersion` (Major/Minor/Alt)
- ❌ `LedgerDB` configuration (Backend, snapshots, etc.)
- ❌ `Protocol` field ("Cardano")
- ❌ `RequiresNetworkMagic` handling
- ❌ 40+ `Trace*` flags for debugging/monitoring
- ❌ `TracingVerbosity` setting
- ❌ EKG metrics port (`hasEKG`)
- ❌ Prometheus configuration (`hasPrometheus`)
- ❌ Logging backends (`defaultBackends`, `defaultScribes`)
- ❌ Advanced options (`mapBackends`, `mapSubtrace`)
- ❌ Log rotation settings

---

## 2. Topology File Incompatibilities

### 2.1 topology.json P2P Structure

**Official Haskell P2P Schema**:
```json
{
  "bootstrapPeers": [
    {
      "address": "backbone.cardano.iog.io",
      "port": 3001
    }
  ],
  "localRoots": [
    {
      "accessPoints": [],
      "advertise": false,
      "trustable": false,
      "valency": 1
    }
  ],
  "publicRoots": [
    {
      "accessPoints": [],
      "advertise": false
    }
  ],
  "useLedgerAfterSlot": 157852837
}
```

**Current Rust Schema**:
```rust
pub struct NetworkTopology {
    pub producers: Vec<TopologyProducer>,  // Simple list only
}

pub struct TopologyProducer {
    pub addr: String,
    pub port: u16,
    pub valency: u32,
}
```

**Missing P2P Features**:
- ❌ `bootstrapPeers` array (required for initial network discovery)
- ❌ `localRoots` configuration (local peer management)
- ❌ `publicRoots` configuration (public peer discovery)
- ❌ `useLedgerAfterSlot` (transition from bootstrap to ledger peers)
- ❌ `advertise` flag (peer advertising capability)
- ❌ `trustable` flag (trusted peer designation)
- ❌ `accessPoints` lists (dynamic peer management)
- ❌ P2P mode support entirely missing

**Impact**: Cannot participate in P2P network; incompatible with modern Cardano topology

---

## 3. Genesis File Support

### 3.1 Required Genesis Files

**Haskell Requirements**:
- ✅ `byron-genesis.json` (with hash validation)
- ✅ `shelley-genesis.json` (with hash validation)
- ✅ `alonzo-genesis.json` (with hash validation)
- ✅ `conway-genesis.json` (with hash validation - LATEST ERA)

**Rust Implementation**:
```rust
pub struct AdvancedNodeConfiguration {
    pub byron_genesis_file: Option<PathBuf>,     // Path only, no hash
    pub shelley_genesis_file: Option<PathBuf>,   // Path only, no hash
    pub alonzo_genesis_file: Option<PathBuf>,    // Path only, no hash
    pub conway_genesis_file: Option<PathBuf>,    // Path only, no hash
}
```

**Missing Features**:
- ❌ Genesis file hash validation (security-critical)
- ❌ Genesis file parsing and validation logic
- ❌ Genesis parameter extraction (protocol parameters, initial UTxO, etc.)
- ❌ Era transition handling based on genesis configs

---

## 4. Consensus Mode Support

### 4.1 Consensus Configuration

**Haskell Modes**:
```json
{
  "ConsensusMode": "PraosMode"  // or "GenesisMode"
}
```

For GenesisMode:
```json
{
  "ConsensusMode": "GenesisMode",
  "peerSnapshotFile": "peer-snapshot.json"  // in topology.json
}
```

**Rust Implementation**:
- ❌ No `ConsensusMode` field
- ❌ No Genesis mode support
- ❌ No peer snapshot handling
- ❌ Hardcoded assumptions about consensus

---

## 5. Protocol Versioning

### 5.1 Block Version Tracking

**Haskell**:
```json
{
  "LastKnownBlockVersion-Major": 3,
  "LastKnownBlockVersion-Minor": 0,
  "LastKnownBlockVersion-Alt": 0,
  "MaxKnownMajorProtocolVersion": 2,
  "MinNodeVersion": "10.4.0"
}
```

**Rust**:
```rust
pub struct ProtocolVersion {
    pub major: u32,
    pub minor: u32,
    // Missing: Alt, MaxKnown, MinNodeVersion
}
```

**Impact**: Cannot negotiate correct protocol versions with peers

---

## 6. Tracing & Monitoring Incompatibilities

### 6.1 Trace Flags

**Haskell** (40+ configurable trace flags):
- `TraceBlockFetchClient`, `TraceBlockFetchProtocol`, `TraceBlockFetchServer`
- `TraceChainDb`, `TraceChainSyncClient`, `TraceChainSyncProtocol`
- `TraceConnectionManager`, `TraceDNSResolver`, `TraceDNSSubscription`
- `TraceErrorPolicy`, `TraceForge`, `TraceHandshake`
- `TraceInboundGovernor`, `TraceIpSubscription`, `TraceLedgerPeers`
- `TraceMempool`, `TraceMux`, `TracePeerSelection`
- And 25+ more...

**Rust**:
```rust
pub struct TracingConfiguration {
    pub trace_chain_sync_client: bool,      // Only 4 flags
    pub trace_block_fetch_protocol: bool,
    pub trace_chain_db: bool,
    pub trace_tx_submission: bool,
}
```

**Missing**: 36+ trace flags required for production debugging

### 6.2 Metrics Configuration

**Haskell**:
```json
{
  "hasEKG": 12788,                    // EKG metrics port
  "hasPrometheus": ["127.0.0.1", 12798],
  "TurnOnLogMetrics": true
}
```

**Rust**:
```rust
pub enable_metrics: bool,   // Simple flag only
pub metrics_port: u16,      // Single port
```

**Missing**:
- ❌ EKG metrics system
- ❌ Prometheus host/port configuration
- ❌ Log metrics integration

---

## 7. Ledger Database Configuration

### 7.1 LedgerDB Settings

**Haskell**:
```json
{
  "LedgerDB": {
    "Backend": "V2InMemory",      // or "OnDisk"
    "NumOfDiskSnapshots": 2,
    "QueryBatchSize": 100000,
    "SnapshotInterval": 4320
  }
}
```

**Rust**:
- ❌ No LedgerDB configuration
- ❌ No backend selection (in-memory vs on-disk)
- ❌ No snapshot management
- ❌ No query batching configuration

---

## 8. Additional Missing Features

### 8.1 UTXO-HD Support
**Haskell**: Supports UTXO-HD mode (since v10.4.1)
**Rust**: ❌ Not implemented

### 8.2 Checkpoints
**Haskell**: Uses `checkpoints.json` with hash validation
**Rust**: ❌ No checkpoint support

### 8.3 Peer Sharing
**Haskell**: `"PeerSharing": true` (different for block producers)
**Rust**: ❌ Not implemented

### 8.4 Network Magic Handling
**Haskell**: `"RequiresNetworkMagic": "RequiresNoMagic"` (mainnet) or `"RequiresMagic"` (testnets)
**Rust**: Only has `network_magic: u32` field

---

## 9. Compatibility Test Results

### Test: Can Rust node connect to mainnet?
**Result**: ❌ **NO** - Configuration schema incompatible

### Test: Can Rust node parse official config files?
**Result**: ❌ **NO** - Missing 50+ required fields

### Test: Can Rust node participate in P2P network?
**Result**: ❌ **NO** - P2P topology not supported

### Test: Can Rust node validate genesis files?
**Result**: ❌ **NO** - Hash validation missing

### Test: CBOR serialization compatibility?
**Status**: ⚠️ **UNTESTED** - No test vectors verified against Haskell

---

## 10. Required Actions for Compatibility

### Priority 1: Critical (Network Connection)
1. ✅ **Implement full config.json schema** with all 60+ fields
2. ✅ **Implement P2P topology.json schema** (bootstrapPeers, localRoots, publicRoots)
3. ✅ **Add genesis file hash validation**
4. ✅ **Implement ConsensusMode support** (PraosMode/GenesisMode)
5. ✅ **Add Protocol and NetworkMagic handling**

### Priority 2: High (Monitoring & Debugging)
6. ✅ **Implement all 40+ trace flags**
7. ✅ **Add EKG metrics support**
8. ✅ **Add Prometheus configuration**
9. ✅ **Implement log rotation and advanced logging options**

### Priority 3: Medium (Performance & Features)
10. ✅ **Implement LedgerDB configuration**
11. ✅ **Add checkpoint support**
12. ✅ **Implement PeerSharing protocol**
13. ✅ **Add UTXO-HD support**

### Priority 4: Verification
14. ✅ **Create CBOR compatibility test suite** against Haskell test vectors
15. ✅ **Verify protocol message formats** (ChainSync, BlockFetch, TxSubmission)
16. ✅ **Test against official mainnet/testnet networks**
17. ✅ **Validate genesis file parsing** for all eras

---

## 11. Estimated Effort

**Configuration Compatibility**: 2-3 weeks
- Implement full config.json schema
- Implement P2P topology schema
- Add genesis validation
- Add all trace flags

**Protocol Verification**: 2-4 weeks
- CBOR serialization tests
- Wire protocol verification
- Network integration testing

**Feature Parity**: 4-6 weeks
- LedgerDB implementation
- UTXO-HD support
- Checkpoint handling
- Full metrics integration

**Total Estimate**: 8-13 weeks for full Haskell compatibility

---

## 12. Recommendations

### Immediate Actions:
1. **DO NOT deploy to mainnet/testnet** - incompatible with network
2. **Create configuration migration tool** to convert Haskell configs
3. **Implement configuration schema validation** against official schemas
4. **Add integration tests** with official Cardano networks

### Architecture Changes:
1. **Refactor config module** to match Haskell structure exactly
2. **Implement P2P networking stack** compatible with official topology
3. **Add comprehensive tracing system** with all required flags
4. **Implement genesis validation** with hash checking

### Testing Strategy:
1. **Download official test vectors** from Haskell implementation
2. **Create CBOR compatibility suite** for all data structures
3. **Test against Preview testnet** before mainnet attempts
4. **Validate wire protocol** byte-by-byte against Haskell node

---

## 13. Conclusion

**Current Status**: The Rust implementation is a **prototype** that demonstrates core concepts but is **NOT production-ready** or **network-compatible** with the official Cardano network.

**Key Issues**:
- Configuration schema is ~15% complete (10/60+ fields)
- P2P networking is not supported
- Genesis validation is missing
- Tracing/monitoring is minimal (~10% of required flags)
- CBOR compatibility is unverified
- Cannot connect to any official Cardano network

**Path Forward**: Requires significant additional development (8-13 weeks estimated) to achieve compatibility with Haskell cardano-node v10.5.1.

**Recommendation**: Use this implementation for **research and learning purposes only**. For production use, rely on the official Haskell cardano-node until compatibility is verified.

---

**Generated**: 2025-01-XX
**Reference Implementation**: [cardano-node v10.5.1](https://github.com/IntersectMBO/cardano-node/releases/tag/10.5.1)
**Configuration Docs**: https://book.world.dev.cardano.org/environments.html
