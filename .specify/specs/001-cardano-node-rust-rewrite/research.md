# Research: Cardano Node Rust Rewrite

## Technology Decisions

### Language and Runtime
**Decision**: Rust 1.75+ with Tokio async runtime
**Rationale**:
- Memory safety without garbage collection overhead
- Zero-cost abstractions for performance
- Strong type system prevents many consensus bugs
- Excellent async/await support for I/O-heavy operations
- Active ecosystem for cryptographic libraries
**Alternatives considered**:
- Go: Simpler but GC overhead, weaker type system
- C++: Performance but memory safety concerns
- Zig: Too immature for production blockchain

### Cryptographic Libraries
**Decision**: ed25519-dalek, blake2, sha2, vrf, blstrs crates
**Rationale**:
- ed25519-dalek: Battle-tested, constant-time operations, Haskell compatibility
- blake2/sha2: Official implementations, performance optimized
- vrf crate: Standards-compliant VRF implementation
- blstrs: BLS12-381 curve operations for Plutus compatibility
**Alternatives considered**:
- RustCrypto ecosystem: More modular but less integrated
- OpenSSL bindings: C dependencies, harder to audit
- Custom implementations: Too risky for consensus-critical code

### Serialization Format
**Decision**: minicbor for CBOR serialization
**Rationale**:
- Lightweight and fast CBOR implementation
- No_std support for embedded use cases
- Good error handling and debugging
- Maintains byte-for-byte compatibility with Haskell
**Alternatives considered**:
- ciborium: More features but heavier
- serde_cbor: Legacy, less maintained
- Custom CBOR: Too complex, compatibility risk

### Database Strategy
**Decision**: Hybrid approach - LMDB for immutable data, RocksDB for UTXO state
**Rationale**:
- LMDB: Excellent for write-once blockchain data, memory-mapped performance
- RocksDB: Optimized for frequent updates needed by UTXO set
- Proven in other blockchain implementations
- Allows for future UTXO-HD integration
**Alternatives considered**:
- Single database: Suboptimal for different access patterns
- PostgreSQL: SQL overhead for blockchain data
- Custom storage: Development risk and maintenance burden

### Networking Architecture
**Decision**: Custom P2P implementation using tokio
**Rationale**:
- Full control over protocol implementation
- Async I/O for handling many peers concurrently
- Direct compatibility with existing Cardano network protocols
- Performance optimization opportunities
**Alternatives considered**:
- libp2p-rs: Generic but adds complexity and overhead
- HTTP-based: Not suitable for P2P blockchain networking
- Existing network libraries: Don't match Cardano's specific needs

### Testing Strategy
**Decision**: Property-based testing with proptest + cross-validation
**Rationale**:
- Property-based tests catch edge cases better than unit tests
- Cross-validation with Haskell ensures behavioral compatibility
- Extensive test vectors from production Cardano network
- Fuzzing for protocol and serialization robustness
**Alternatives considered**:
- Unit testing only: Insufficient for consensus correctness
- Integration testing only: Slower feedback, harder debugging
- Formal verification: Too resource-intensive for full implementation

### Performance Monitoring
**Decision**: Custom metrics with Prometheus integration
**Rationale**:
- Real-time performance comparison with Haskell node
- Integration with existing Cardano monitoring infrastructure
- Custom metrics for blockchain-specific operations
- Historical performance tracking for regression detection
**Alternatives considered**:
- Generic APM tools: Don't understand blockchain specifics
- Built-in Rust profiling: Good for development but not production
- No monitoring: Unacceptable for production blockchain node

## Architecture Patterns

### Error Handling Strategy
**Decision**: Custom error types with thiserror + anyhow for context
**Rationale**:
- Type-safe error handling prevents consensus bugs
- Rich error context for debugging and monitoring
- Ergonomic error propagation with ? operator
- Structured errors for automated error handling
**Implementation**:
- Custom error enums per crate
- Error context preservation across async boundaries
- Structured logging for error analysis

### Async Programming Model
**Decision**: Async/await throughout with careful synchronization boundaries
**Rationale**:
- Non-blocking I/O essential for networking and storage
- Better resource utilization than thread-per-connection
- Rust's async model prevents common concurrency bugs
- Tokio ecosystem provides battle-tested primitives
**Implementation**:
- Async for I/O operations (network, disk)
- Sync for CPU-intensive operations (crypto, validation)
- Careful channel design for inter-task communication

### Configuration Management
**Decision**: Maintain Haskell configuration format compatibility
**Rationale**:
- Seamless migration for existing node operators
- Extensive existing configuration documentation
- Battle-tested configuration patterns
- Community familiarity reduces adoption friction
**Implementation**:
- JSON/YAML parsing with validation
- Environment variable overrides
- Runtime configuration reloading where safe

## Integration Patterns

### Haskell Compatibility Validation
**Decision**: Continuous cross-validation test suite
**Rationale**:
- Automated detection of behavioral divergence
- Regression prevention during development
- Confidence in production readiness
- Documentation of intended behavior differences
**Implementation**:
- Shared test vectors between implementations
- Automated test execution on every change
- Performance benchmark comparisons
- Serialization round-trip validation

### Development Workflow
**Decision**: Test-driven development with staging validation
**Rationale**:
- Tests define expected behavior before implementation
- Early detection of specification misunderstandings
- Regression prevention throughout development
- Documentation of system behavior
**Implementation**:
- Write failing tests first for each feature
- Implement minimal code to make tests pass
- Refactor with test safety net
- Validate on testnets before mainnet

### Deployment Strategy
**Decision**: Gradual rollout with extensive monitoring
**Rationale**:
- Risk mitigation for consensus-critical software
- Real-world validation before full deployment
- Community confidence building
- Rollback capability if issues discovered
**Implementation**:
- Testnet deployment and validation
- Limited mainnet deployment with monitoring
- Performance and correctness validation
- Full rollout after confidence established

## Risk Mitigation

### Consensus Divergence Prevention
**Approach**:
- Extensive property-based testing with Haskell test vectors
- Continuous integration testing against live networks
- Formal specification review and implementation validation
- Conservative implementation approach preferring correctness over optimization

### Performance Regression Prevention
**Approach**:
- Continuous benchmarking against Haskell implementation
- Performance regression detection in CI
- Memory usage monitoring and alerts
- Load testing with realistic blockchain data

### Security Vulnerability Prevention
**Approach**:
- Third-party security audit before production deployment
- Fuzzing for protocol and parsing robustness
- Careful review of cryptographic implementations
- Incident response plan for security issues

## Compatibility Research (2025-02-14)

### VRF reference mapping
- **Primary Haskell modules**:
	- `Cardano.Crypto.VRF.Class` (cardano-base/cardano-crypto-class) – defines the `VRFAlgorithm` type class, raw serialisation helpers, and the `CertifiedVRF` wrapper.
	- `Cardano.Crypto.VRF.Praos` (cardano-base/cardano-crypto-praos) – production ECVRF implementation via libsodium FFI, exposing the `PraosVRF` instance used throughout the node.
	- `Cardano.Crypto.VRF.PraosBatchCompat` – legacy batch-proof variant (128-byte proofs) retained for compatibility; mirrors Rust constant `VRF_BATCH_PROOF_LENGTH`.
	- `Test.Crypto.VRF` (cardano-base/cardano-crypto-tests) – QuickCheck properties plus golden vectors validating key generation, evaluation, verification, CBOR size expressions, and proof hashing.
- **Rust counterparts**:
	- `crates/cardano-crypto/src/vrf/mod.rs` – public API surface that enforces expected byte lengths (`VRF_*_LENGTH`) and exposes safe wrappers around proofs, outputs, and keys.
	- `crates/cardano-crypto/src/vrf/backend.rs` – pure-Rust ECVRF implementation on edwards25519 closely following libsodium draft-03 transcript construction.
- **Size and constant alignment**:
	- Haskell `verKeySizeVRF`, `signKeySizeVRF`, `certSizeVRF`, and `crypto_vrf_outputbytes` correspond exactly to Rust constants 32, 64, 80, and 64 respectively.
	- Seed length is 32 bytes in both implementations; batch proofs are 128 bytes (`PraosBatchCompat`) and tracked in Rust as `VRF_BATCH_PROOF_LENGTH` pending API exposure.
- **Operational semantics**:
	- Deterministic nonce derivation (seeded from secret scalar + hashed message) is implemented in both backends, so repeated proofs for identical inputs must match byte-for-byte.
	- `proof_to_hash` output mirrors Haskell’s `crypto_vrf_proof_to_hash`, producing a 64-byte digest used for Ouroboros randomness; Rust’s `VrfProof::to_hash` already enforces this length.
- **Test vectors**:
	- Golden files under `cardano-base/cardano-crypto-tests/test_vectors/vrf_ver03_*` cover zero/standard seeds from the libsodium ECVRF draft-03 reference. These should be mirrored in `tests/crypto/test_vrf_compat.rs` to assert compatibility.
	- Haskell QuickCheck suites assert raw serialisation round-trips, CBOR envelope sizes, and negative verification cases; Rust property tests (proptest) need to align with the same expectations.

### Immediate follow-ups
- Replace placeholder VRF vectors in Rust tests with the Praos golden files and validate outputs, proofs, and `proof_to_hash` values.
- Extend Rust property tests to cover error paths (altered proofs, mismatched inputs, malformed key lengths) consistent with Haskell test coverage.
- Decide whether to expose a batch-compatible VRF API akin to `PraosBatchCompatVRF` or to defer until we require federated leader election compatibility.

## Unknown Mitigation

All technical unknowns from the specification have been resolved through this research phase. No remaining NEEDS CLARIFICATION items.
