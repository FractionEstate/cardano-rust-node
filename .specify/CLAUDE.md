# Cardano Node Rust Development Guidelines

Auto-generated from all feature plans. Last updated: 2025-09-27

## Active Technologies
- **Language**: Rust 1.75+ (latest stable with async/await stabilization)
- **Async Runtime**: Tokio (multi-threaded scheduler for blockchain workloads)
- **Serialization**: minicbor (CBOR compatibility), serde (general serialization)
- **Cryptography**: ed25519-dalek, vrf-rs, blake2, blstrs (BLS12-381)
- **Storage**: LMDB (primary), RocksDB (alternative)
- **Testing**: cargo test, proptest (property-based testing)
- **Networking**: Custom Cardano protocol implementation over TCP

## Project Structure
```
cardano-node-rust/
├── Cargo.toml                      # Workspace configuration
├── crates/
│   ├── cardano-node/               # Main executable
│   ├── cardano-crypto/             # Cryptographic operations
│   ├── cardano-ledger/             # Ledger state and validation
│   ├── cardano-consensus/          # Ouroboros consensus protocol
│   ├── cardano-network/            # P2P networking and protocols
│   ├── cardano-storage/            # Database and persistence
│   ├── cardano-tracing/            # Logging and metrics
│   ├── cardano-api/                # External API interfaces
│   └── cardano-testnet/           # Testing utilities
├── tests/                         # Integration tests
├── benches/                       # Performance benchmarks
├── docs/                          # Documentation
└── configuration/                 # Network configurations
```

## Commands
```bash
# Build workspace
cargo build --workspace

# Run tests with property-based testing
cargo test --workspace

# Run crypto compatibility tests
cargo test --package cardano-crypto --test compatibility

# Build optimized release
cargo build --release --bin cardano-node

# Run benchmarks
cargo bench --bench block_validation

# Run node with config
./target/release/cardano-node run --config config.yaml --topology topology.json
```

## Code Style
- Follow Rust standard formatting with `rustfmt`
- Use `clippy` for linting with warnings as errors
- Property-based tests for all crypto and consensus operations
- TDD approach: tests before implementation
- CBOR serialization must be byte-identical with Haskell node
- All public APIs must be documented
- Use `#[derive(Debug, Clone, PartialEq, Eq)]` for data types where appropriate
- Error handling with `Result<T, Error>` types
- Async/await for all I/O operations

## Recent Changes
1. **001-cardano-node-rust-rewrite**: Complete Rust rewrite of Haskell Cardano Node
   - Added modular crate architecture mirroring Haskell packages
   - Established cryptographic foundation with compatibility requirements
   - Created comprehensive testing strategy for protocol compliance

<!-- MANUAL ADDITIONS START -->
<!-- MANUAL ADDITIONS END -->
