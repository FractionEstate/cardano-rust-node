# Cardano Node Rust Rewrite Constitution

## Core Principles

### I. Functional Parity & Correctness (CRITICAL)
Every component must maintain 100% functional compatibility with the original Haskell implementation. All cryptographic operations must produce identical results. Protocol compliance is non-negotiable - no deviations from Cardano consensus rules. Consensus safety is paramount over performance optimizations.

### II. Modular Architecture (MANDATORY)
Follow the original Cardano Node's modular design: separate consensus, networking, ledger, and node layers. Each module must be independently testable and replaceable. Clear interfaces between components with minimal coupling. Rust crates must mirror the Haskell package structure for maintainability.

### III. Memory Safety & Performance (CORE VALUE)
Leverage Rust's memory safety without garbage collection overhead. Zero-cost abstractions where possible. Performance must meet or exceed Haskell version. Memory usage must be predictable and bounded. Use async/await for I/O without blocking threads.

### IV. Cryptographic Security (NON-NEGOTIABLE)
All cryptographic implementations must use battle-tested libraries. Ed25519, VRF, and hash functions must produce identical outputs. Key derivation and signing must be constant-time. Extensive property-based testing for crypto operations. Security audits required for crypto modules.

### V. Protocol Compliance & Interoperability
Must fully implement Ouroboros consensus protocol. Compatible with existing Cardano network and other node implementations. All CBOR serialization must match byte-for-byte with Haskell version. Support all eras (Byron, Shelley, Allegra, Mary, Alonzo, Babbage, Conway).

## Security Requirements

Consensus safety: Never violate Cardano consensus rules under any circumstances. Chain validation must be identical to Haskell implementation. Fork choice rules must be precisely implemented. No tolerance for consensus divergence even under edge cases.

Network security: Maintain compatibility with existing P2P network protocol. Proper handling of malformed messages and adversarial peers. Rate limiting and DoS protection. Secure connection establishment and maintenance.

Operational security: Safe handling of signing keys and sensitive material. Proper file permissions and secure storage. Audit trails for all critical operations. Graceful degradation under resource constraints.

## Performance Standards

Throughput requirements: Block processing speed must match or exceed current Haskell node. Transaction validation throughput >= 1000 tx/sec. Mempool operations must be highly concurrent. Database operations must be optimized for UTXO model.

Resource constraints: Memory usage must be bounded and predictable. CPU usage optimized for multi-core systems. Network bandwidth usage must be efficient. Startup time should be minimized for better UX.

Scalability targets: Support for UTXO-HD (when available) integration. Efficient handling of large blocks and transactions. Graceful performance under high network load. Preparation for future protocol upgrades.

## Development Workflow

Testing strategy (NON-NEGOTIABLE): Property-based testing for all core logic using QuickCheck equivalents. Integration tests against real Cardano networks. Consensus compatibility tests with reference implementation. Performance benchmarking against Haskell version. Crypto correctness verification.

Code quality gates: All code must pass Rust compiler warnings as errors. Clippy lints must be addressed. Documentation required for all public APIs. Code coverage minimum 80% for core modules. Regular security and performance reviews.

Compatibility validation: Continuous integration testing against Cardano testnets. Regression testing against known blockchain states. Cross-validation of serialization with Haskell node. Protocol conformance testing with other implementations.

## Governance

Constitution supersedes all other practices. Any deviation from functional parity or protocol compliance must be documented, justified, and approved by the core team. Performance optimizations cannot compromise correctness or security. Use CLAUDE.md for runtime development guidance.

**Version**: 1.0.0 | **Ratified**: 2025-01-27 | **Last Amended**: 2025-01-27
