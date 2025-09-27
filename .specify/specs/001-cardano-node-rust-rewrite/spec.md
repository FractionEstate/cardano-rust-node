# Cardano Node Rust Rewrite Specification

## Overview

This specification defines the complete rewrite of the Cardano Node from Haskell to Rust, maintaining 100% functional compatibility while leveraging Rust's performance, memory safety, and modern tooling capabilities.

## Background

The Cardano Node is the core component for participating in the Cardano decentralized blockchain network. The original implementation is written in Haskell and integrates several key layers:

- **Ledger Layer**: Transaction processing and UTXO management
- **Consensus Layer**: Ouroboros consensus protocol implementation
- **Networking Layer**: P2P communication and protocol handling
- **Storage Layer**: Block and state persistence
- **Cryptographic Layer**: Ed25519, VRF, and hash operations
- **Node Layer**: Main executable and API interfaces

The Rust rewrite must maintain byte-for-byte compatibility with the Haskell implementation while providing improved performance, reduced memory usage, and better operational characteristics.

## Requirements

### Functional Requirements

#### FR1: Complete Protocol Compatibility
- **FR1.1**: Support all Cardano eras (Byron, Shelley, Allegra, Mary, Alonzo, Babbage, Conway)
- **FR1.2**: Implement Ouroboros consensus protocol with identical behavior
- **FR1.3**: Maintain CBOR serialization compatibility (byte-for-byte matching)
- **FR1.4**: Support all transaction types and validation rules

#### FR2: Cryptographic Operations
- **FR2.1**: Ed25519 digital signatures with Haskell compatibility
- **FR2.2**: VRF (Verifiable Random Functions) for leader election
- **FR2.3**: BLAKE2b and SHA256 hash functions
- **FR2.4**: BLS12-381 cryptographic operations

#### FR3: Network Protocol Compliance
- **FR3.1**: P2P networking compatible with existing Cardano network
- **FR3.2**: Support for all network protocols (handshake, ChainSync, BlockFetch, etc.)
- **FR3.3**: Proper handling of network topology and peer discovery

#### FR4: Storage and Persistence
- **FR4.1**: Efficient UTXO storage and retrieval
- **FR4.2**: Block storage with proper indexing
- **FR4.3**: Immutable database operations
- **FR4.4**: Support for database migrations and upgrades

#### FR5: API Compatibility
- **FR5.1**: REST API compatibility with existing cardano-node
- **FR5.2**: CLI interface matching existing functionality
- **FR5.3**: Configuration file format compatibility

### Non-Functional Requirements

#### NFR1: Performance Standards
- **NFR1.1**: Block processing speed >= current Haskell implementation
- **NFR1.2**: Transaction validation throughput >= 1000 tx/sec
- **NFR1.3**: Memory usage <= 80% of Haskell version under normal load
- **NFR1.4**: Startup time <= 50% of Haskell version

#### NFR2: Security Requirements
- **NFR2.1**: Zero consensus divergence tolerance
- **NFR2.2**: Memory safety guaranteed by Rust type system
- **NFR2.3**: Secure key handling and cryptographic operations
- **NFR2.4**: Protection against DoS and malformed input attacks

#### NFR3: Reliability and Availability
- **NFR3.1**: 99.9% uptime under normal network conditions
- **NFR3.2**: Graceful degradation under resource constraints
- **NFR3.3**: Automatic recovery from transient failures
- **NFR3.4**: Comprehensive error handling and logging

#### NFR4: Maintainability
- **NFR4.1**: Modular architecture with clear separation of concerns
- **NFR4.2**: Comprehensive test coverage (>90% for critical components)
- **NFR4.3**: Documentation for all public APIs and interfaces
- **NFR4.4**: Code style consistency with automated formatting

## Architecture

### Crate Structure

The implementation follows a modular crate structure mirroring the Haskell packages:

```
cardano-node-rust/
├── crates/
│   ├── cardano-crypto/        # Cryptographic operations
│   ├── cardano-consensus/     # Ouroboros consensus protocol
│   ├── cardano-ledger/        # Ledger state and transactions
│   ├── cardano-network/       # P2P networking layer
│   ├── cardano-storage/       # Database and persistence
│   ├── cardano-tracing/       # Logging and observability
│   ├── cardano-api/           # Public API interfaces
│   ├── cardano-node/          # Main node executable
│   └── cardano-testnet/       # Testing utilities
└── tests/                     # Integration tests
```

### Key Design Principles

1. **Modular Design**: Each crate has a single responsibility with minimal coupling
2. **Test-Driven Development**: Tests written before implementation to ensure correctness
3. **Performance by Design**: Async/await for I/O, zero-copy where possible
4. **Memory Safety**: Leverage Rust's ownership system for safe concurrent access
5. **Protocol Compliance**: Extensive validation against Haskell reference implementation

## Success Criteria

### Acceptance Criteria

#### AC1: Functional Compatibility
- [ ] All cryptographic operations produce identical outputs to Haskell version
- [ ] Successfully syncs and validates mainnet blockchain from genesis
- [ ] Passes all consensus compatibility tests
- [ ] CBOR serialization matches byte-for-byte with Haskell implementation

#### AC2: Performance Benchmarks
- [ ] Block validation time <= 90% of Haskell version
- [ ] Memory usage <= 80% of Haskell version
- [ ] Network throughput >= 100% of Haskell version
- [ ] Startup time <= 50% of Haskell version

#### AC3: Network Integration
- [ ] Successfully connects to and participates in Cardano mainnet
- [ ] Compatible with existing SPO infrastructure
- [ ] Proper handling of network protocol versions
- [ ] Successful interaction with other node implementations

#### AC4: Operational Readiness
- [ ] Production-ready logging and monitoring
- [ ] Comprehensive error handling and recovery
- [ ] Security audit completion with no critical findings
- [ ] Documentation complete for operators and developers

## Technical Constraints

### Technology Stack
- **Language**: Rust 1.75+ with 2021 edition
- **Async Runtime**: Tokio for async/await operations
- **Serialization**: CBOR via minicbor crate
- **Cryptography**: ed25519-dalek, blake2, sha2, vrf, blstrs crates
- **Database**: LMDB and RocksDB for different storage needs
- **Networking**: Custom P2P protocol implementation

### Dependencies on External Systems
- **Cardano Network**: Must be compatible with existing network protocols
- **Haskell Reference**: All behavior must match the Haskell implementation
- **Configuration**: Must use existing Cardano configuration file formats

## Dependencies

### Internal Dependencies
- Each crate builds upon lower-level crates in dependency order
- Crypto operations are foundational to all other components
- Network layer depends on consensus for protocol validation
- Storage layer provides persistence for ledger and consensus

### External Dependencies
- Cardano ledger specifications for transaction validation rules
- Ouroboros consensus specifications for protocol compliance
- Cardano network specifications for P2P communication
- CBOR specifications for serialization compatibility

## Risks and Mitigations

### High-Risk Areas
1. **Consensus Divergence**: Risk of implementing protocol incorrectly
   - Mitigation: Extensive cross-validation with Haskell implementation
   - Mitigation: Property-based testing with identical test vectors

2. **Cryptographic Incompatibility**: Risk of different crypto outputs
   - Mitigation: Use same underlying crypto libraries where possible
   - Mitigation: Comprehensive test vectors from Haskell implementation

3. **Performance Regression**: Risk of slower performance than Haskell
   - Mitigation: Continuous benchmarking against Haskell version
   - Mitigation: Performance-focused design from day one

4. **Network Protocol Bugs**: Risk of network incompatibility
   - Mitigation: Extensive integration testing with existing nodes
   - Mitigation: Gradual rollout on testnets before mainnet

## Implementation Phases

### Phase 1: Foundation (Crypto + Core)
- Cryptographic operations with Haskell compatibility
- Basic CBOR serialization framework
- Core data structures and error handling

### Phase 2: Consensus Layer
- Ouroboros protocol implementation
- Block validation and chain selection
- Epoch boundary processing

### Phase 3: Ledger Implementation
- UTXO management and transaction processing
- Multi-era support (Byron through Conway)
- Smart contract execution (Plutus integration)

### Phase 4: Networking Layer
- P2P protocol implementation
- Peer discovery and connection management
- Protocol version negotiation

### Phase 5: Storage and Persistence
- Database schema and operations
- Efficient UTXO-HD integration
- Backup and recovery mechanisms

### Phase 6: Integration and Testing
- End-to-end integration testing
- Performance optimization
- Security audit and hardening

### Phase 7: Production Deployment
- Documentation completion
- Operator migration guides
- Mainnet deployment strategy

## Clarifications

### Session 1: Architecture and Compatibility (2025-01-27)
**Question**: How strict is the compatibility requirement with the Haskell implementation?
**Answer**: Compatibility must be 100% for consensus-critical operations. CBOR serialization must be byte-for-byte identical. Performance optimizations are allowed as long as they don't change observable behavior.

**Question**: What is the migration strategy for existing node operators?
**Answer**: Rust implementation should be a drop-in replacement. Same configuration files, same CLI interface, same API endpoints. Operators should be able to switch with minimal changes.

**Question**: How should we handle Haskell-specific implementation details?
**Answer**: Where Haskell implementation has quirks or undefined behavior, we should match the actual behavior, not the specification. Real-world compatibility takes precedence.

### Session 2: Performance and Testing (2025-01-27)
**Question**: What are the specific performance targets?
**Answer**: Memory usage should be significantly lower (target 50-80% of Haskell). CPU performance should match or exceed. Network latency should be comparable. Startup time improvement is highly desired.

**Question**: What testing strategy should we use?
**Answer**: Test-driven development with property-based testing. Cross-validation with Haskell for all outputs. Integration testing on testnets. Performance benchmarking on every change.

**Question**: How do we ensure cryptographic correctness?
**Answer**: Use identical test vectors from Haskell implementation. Property-based testing for crypto operations. Security audit by qualified third parties before production use.

### Session 3: Implementation Strategy (2025-01-27)
**Question**: Should we implement all eras simultaneously or incrementally?
**Answer**: Implement incrementally, starting with the most recent era (Conway) and working backwards. This allows faster time-to-value while maintaining full compatibility.

**Question**: How do we handle the async/sync boundary with existing code?
**Answer**: Design async from the ground up. Don't try to retrofit sync code. Use tokio consistently throughout for better performance and maintainability.

**Question**: What's the strategy for handling Plutus smart contracts?
**Answer**: Initially use FFI to existing Haskell Plutus evaluator. Later phases can implement native Rust Plutus interpreter for better integration and performance.
