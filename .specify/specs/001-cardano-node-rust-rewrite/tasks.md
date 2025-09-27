# Tasks: Cardano Node Rust Rewrite

**Input**: Design documents from `/specs/001-cardano-node-rust-rewrite/`

**Prerequisites**: plan.md, research.md, data-model.md, contracts/

## Execution Flow (main)
```
1. Load plan.md from feature directory
   → Implementation plan loaded with Rust ecosystem and modular architecture
   → Extract: Rust 1.75+, tokio, modular crate structure, blockchain protocols

2. Load design documents:
   → data-model.md: Blockchain entities → model creation tasks
   → contracts/: API interfaces → contract test and implementation tasks
   → research.md: Technology decisions → setup and integration tasks

3. Generate tasks by category:
   → Foundation: project setup, crypto, serialization
   → Core Protocol: consensus, ledger, networking
   → Integration: storage, APIs, configuration
   → Validation: testing, benchmarking, compatibility

4. Apply task rules:
   → Independent crates = mark [P] for parallel development
   → Same crate = sequential development within crate
   → Tests before implementation (TDD approach)

5. Number tasks sequentially (T001, T002...)
6. Generate dependency graph reflecting crate dependencies
```

## Format: `[ID] [P?] Description`
- **[P]**: Can run in parallel (different crates, no dependencies)
- Include exact file paths in descriptions

## ✅ Phase 3.1: Foundation Setup (COMPLETE)
- [x] T001 Create Cargo workspace and crate structure per plan.md
- [x] T002 Initialize cardano-crypto crate in crates/cardano-crypto/Cargo.toml
- [x] T003 [P] Initialize cardano-consensus crate in crates/cardano-consensus/Cargo.toml
- [x] T004 [P] Initialize cardano-ledger crate in crates/cardano-ledger/Cargo.toml
- [x] T005 [P] Initialize cardano-network crate in crates/cardano-network/Cargo.toml
- [x] T006 [P] Initialize cardano-storage crate in crates/cardano-storage/Cargo.toml
- [x] T007 [P] Initialize cardano-api crate in crates/cardano-api/Cargo.toml
- [x] T008 [P] Initialize cardano-node crate in crates/cardano-node/Cargo.toml
- [x] T009 [P] Initialize cardano-tracing crate in crates/cardano-tracing/Cargo.toml
- [x] T010 Configure workspace dependencies and optimization settings

## ✅ Phase 3.2: Cryptographic Foundation TDD (COMPLETE)
- [x] T011 [P] Ed25519 signature tests in crates/cardano-crypto/tests/ed25519_basic.rs
- [x] T012 [P] VRF proof tests in tests/crypto/test_vrf_compat.rs
- [x] T013 [P] Hash function tests in tests/crypto/test_hash_compat.rs
- [x] T014 [P] BLS signature tests in tests/crypto/test_bls_compat.rs
- [x] T015 [P] Cross-validation tests in tests/crypto/test_vectors.rs

## ✅ Phase 3.3: Cryptographic Implementation (COMPLETE)
- [x] T016 Ed25519 signature operations in crates/cardano-crypto/src/ed25519/mod.rs
- [x] T017 VRF proof generation in crates/cardano-crypto/src/vrf/mod.rs
- [x] T018 Hash function implementations in crates/cardano-crypto/src/hash/mod.rs
- [x] T019 BLS signature operations in crates/cardano-crypto/src/bls/mod.rs
- [x] T020 Integration tests for all crypto modules in crates/cardano-crypto/tests/integration.rs

## ✅ Phase 3.4: Serialization Foundation TDD (COMPLETE)
- [x] T021 [P] CBOR encoding tests in tests/serialization/test_cbor_encoding.rs
- [x] T022 [P] CBOR decoding tests in tests/serialization/test_cbor_decoding.rs
- [x] T023 [P] Cross-compatibility tests in tests/serialization/test_haskell_compat.rs

## ✅ Phase 3.5: Serialization Implementation (COMPLETE)
- [x] T024 CBOR serialization traits in crates/cardano-ledger/src/cbor/mod.rs
- [x] T025 Blockchain type serialization in crates/cardano-ledger/src/serialization/mod.rs

## ✅ Phase 3.6: Core Data Types TDD (COMPLETE)
- [x] T026 [P] Block type tests in tests/ledger/test_block_types.rs
- [x] T027 [P] Transaction type tests in tests/ledger/test_transaction_types.rs
- [x] T028 [P] Address type tests in tests/ledger/test_address_types.rs
- [x] T029 [P] Certificate type tests in tests/ledger/test_certificate_types.rs

## ✅ Phase 3.7: Core Data Type Implementation (COMPLETE)
- [x] T030 Block types in crates/cardano-ledger/src/block/mod.rs
- [x] T031 Transaction types in crates/cardano-ledger/src/transaction/mod.rs
- [x] T032 Address types in crates/cardano-ledger/src/address/mod.rs
- [x] T033 Certificate types in crates/cardano-ledger/src/certificate/mod.rs
- [x] T034 Common types and utilities in crates/cardano-ledger/src/types/mod.rs

## ✅ Phase 3.8: Era Validation Tests (COMPLETE)
- [x] T035 [P] Byron era validation tests in tests/ledger/test_byron_validation.rs
- [x] T036 [P] Shelley era validation tests in tests/ledger/test_shelley_validation.rs
- [x] T037 [P] Allegra era validation tests in tests/ledger/test_allegra_validation.rs
- [x] T038 [P] Mary era validation tests in tests/ledger/test_mary_validation.rs
- [x] T039 [P] Alonzo era validation tests in tests/ledger/test_alonzo_validation.rs
- [x] T040 [P] Babbage era validation tests in tests/ledger/test_babbage_validation.rs
- [x] T041 [P] Conway era validation tests in tests/ledger/test_conway_validation.rs

## ✅ Phase 3.9: Era Implementation Prerequisites (COMPLETE)
- [x] T042 Byron era implementation in crates/cardano-ledger/src/byron/mod.rs ✅ COMPLETE
- [x] T043 Shelley era implementation in crates/cardano-ledger/src/shelley/mod.rs ✅ COMPLETE
- [x] T044 Allegra era implementation in crates/cardano-ledger/src/allegra/mod.rs ✅ COMPLETE

## ✅ Phase 3.10: Multi-Era Ledger Rules (COMPLETE)
- [x] T045 Mary era implementation in crates/cardano-ledger/src/mary/mod.rs
- [x] T046 Alonzo era implementation with Plutus in crates/cardano-ledger/src/alonzo/mod.rs
- [x] T047 Babbage era implementation in crates/cardano-ledger/src/babbage/mod.rs
- [x] T048 Conway era implementation with governance in crates/cardano-ledger/src/conway/mod.rs

## ✅ Phase 3.11: Consensus Protocol TDD (COMPLETE)
- [x] T049 [P] Ouroboros consensus protocol tests in tests/consensus/test_ouroboros_protocol.rs
- [x] T050 [P] Chain selection rule tests in tests/consensus/test_chain_selection.rs
- [x] T051 [P] Block production tests in tests/consensus/test_block_production.rs
- [x] T052 [P] Block validation pipeline tests in tests/consensus/test_block_validation.rs

## ✅ Phase 3.12: Consensus Implementation (COMPLETE)
- [x] T053 Ouroboros consensus core in crates/cardano-consensus/src/ouroboros.rs ✅ COMPLETE
- [x] T054 Chain Selection in crates/cardano-consensus/src/chain_selection.rs ✅ COMPLETE
  - ✅ Selection Parameters and configuration
  - ✅ Chain comparison algorithms with VRF tiebreakers (Unrestricted/Restricted)
  - ✅ Fork point detection
  - ✅ Chain quality metrics integration
  - Dependencies: T053 ✅
  - Validation: ✅ 8 unit tests for selection rules and tiebreaker scenarios
- [x] T055 Block production logic in crates/cardano-consensus/src/block_production.rs ✅ COMPLETE
  - ✅ VRF slot leadership evaluation
  - ✅ KES key evolution and signing
  - ✅ Transaction selection with fee prioritization
  - ✅ Block forging pipeline
  - ✅ Production scheduling for epochs
  - Dependencies: T053, T054 ✅
  - Validation: ✅ 16 unit tests for block production pipeline
- [x] T056 Block validation pipeline in crates/cardano-consensus/src/validation.rs ✅ COMPLETE
  - ✅ Comprehensive ValidationPipeline with 5-phase validation (header, body, transactions, cryptographic, ledger state)
  - ✅ Header validation (protocol magic, block size, operational certificates, VRF proof validation)
  - ✅ Transaction validation (inputs, outputs, fees, UTxO balance checking)
  - ✅ Native script and Plutus script validation systems
  - ✅ Validation configuration with toggleable features
  - ✅ Protocol parameters for mainnet configuration
  - ✅ Comprehensive error handling with ConsensusError system
  - Dependencies: T053, T054, T055 ✅
  - Validation: ✅ Compiles successfully with comprehensive implementation (640+ lines)

## ✅ Phase 3.13: Network Protocol Stack TDD (COMPLETE)
- [x] T057 [P] ChainSync protocol tests in tests/network/test_chainsync.rs
- [x] T058 [P] BlockFetch protocol tests in tests/network/test_blockfetch.rs
- [x] T059 [P] TxSubmission protocol tests in tests/network/test_txsubmission.rs
- [x] T060 [P] P2P peer selection tests in tests/network/test_peer_selection.rs

## Phase 3.14: Network Implementation
- [x] T061 ChainSync protocol implementation in crates/cardano-network/src/protocols/chainsync.rs ✅ COMPLETE
- [x] T062 BlockFetch protocol implementation in crates/cardano-network/src/protocols/blockfetch.rs ✅ COMPLETE
- [x] T063 TxSubmission protocol implementation in crates/cardano-network/src/protocols/txsubmission.rs ✅ COMPLETE
- [x] T064 P2P diffusion and peer selection in crates/cardano-network/src/diffusion/mod.rs ✅ COMPLETE
- [x] T065 Network connection management in crates/cardano-network/src/connection/mod.rs

## ✅ Phase 3.15: Storage Backend TDD (COMPLETE)
- [x] T066 [P] LMDB backend tests in tests/storage/test_lmdb_backend.rs
- [x] T067 [P] Storage interface compatibility tests in tests/storage/test_storage_interface.rs
- [x] T068 [P] Chain database operation tests in tests/storage/test_chaindb.rs
- [x] T069 [P] Ledger database operation tests in tests/storage/test_ledgerdb.rs

## Phase 3.16: Storage Implementation ✅ COMPLETE
- [x] T070 LMDB storage backend in crates/cardano-storage/src/backends/lmdb.rs ✅ COMPLETE
- [x] T071 RocksDB storage backend in crates/cardano-storage/src/backends/rocksdb.rs ✅ COMPLETE
- [x] T072 Chain database abstraction in crates/cardano-storage/src/chaindb/mod.rs ✅ COMPLETE
- [x] T073 Ledger database abstraction in crates/cardano-storage/src/ledgerdb/mod.rs ✅ COMPLETE

## Phase 3.17: API Interfaces TDD ✅ COMPLETE
- [x] T074 [P] Local socket API tests in tests/api/test_local_socket.rs ✅ COMPLETE
- [x] T075 [P] CLI interface tests in tests/api/test_cli_interface.rs ✅ COMPLETE
- [x] T076 [P] Submit API tests in tests/api/test_submit_api.rs ✅ COMPLETE

## ✅ Phase 3.18: API Implementation (COMPLETE)
- [x] T077 Local socket protocol implementation in crates/cardano-api/src/local_socket/mod.rs
- [x] T078 REST API endpoints in crates/cardano-api/src/rest_api/mod.rs
- [x] T079 Submit API implementation in crates/cardano-api/src/submit_api/mod.rs

## Phase 3.19: Main Node Integration
- [ ] T080 CLI argument parsing tests in tests/node/test_cli_parsing.rs
- [ ] T081 [P] Configuration loading tests in tests/node/test_config_loading.rs
- [ ] T082 [P] Node startup and shutdown tests in tests/node/test_node_lifecycle.rs
- [ ] T083 CLI interface implementation in crates/cardano-node/src/cli/mod.rs
- [ ] T084 Configuration management in crates/cardano-node/src/config/mod.rs
- [ ] T085 Node runtime and main loop in crates/cardano-node/src/run/mod.rs
- [ ] T086 Main executable entry point in crates/cardano-node/src/main.rs

## Phase 3.20: Tracing and Monitoring
- [ ] T087 [P] Metrics collection tests in tests/tracing/test_metrics.rs
- [ ] T088 [P] Tracing compatibility tests in tests/tracing/test_trace_compat.rs
- [ ] T089 Metrics and monitoring in crates/cardano-tracing/src/metrics/mod.rs
- [ ] T090 Structured tracing implementation in crates/cardano-tracing/src/tracers/mod.rs

## Phase 3.21: Integration Testing
- [ ] T091 [P] End-to-end node operation tests in tests/integration/test_e2e_operation.rs
- [ ] T092 [P] Haskell node compatibility tests in tests/integration/test_haskell_compat.rs
- [ ] T093 [P] Network interoperability tests in tests/integration/test_network_interop.rs
- [ ] T094 [P] Performance benchmark tests in tests/integration/test_performance.rs
- [ ] T095 Multi-node testnet setup in crates/cardano-testnet/src/cluster/mod.rs
- [ ] T096 Test scenario framework in crates/cardano-testnet/src/scenarios/mod.rs

## Phase 3.22: Performance Optimization
- [ ] T097 [P] Block validation benchmarks in benches/block_validation.rs
- [ ] T098 [P] Transaction processing benchmarks in benches/transaction_processing.rs
- [ ] T099 [P] Network protocol benchmarks in benches/network_protocols.rs
- [ ] T100 Memory usage optimization and profiling
- [ ] T101 Database query optimization

## Phase 3.23: Documentation and Deployment
- [ ] T102 [P] API documentation generation in docs/api/
- [ ] T103 [P] Architecture documentation in docs/architecture.md
- [ ] T104 [P] Deployment guides in docs/deployment/
- [ ] T105 [P] Configuration reference in docs/configuration.md
- [ ] T106 Docker containerization setup
- [ ] T107 CI/CD pipeline configuration
- [ ] T108 Release packaging and distribution

## Dependencies

**Foundation Dependencies**:
- T001 (workspace) blocks all other tasks
- T002-T010 (crate initialization) must complete before implementation tasks

**Cryptographic Dependencies**:
- T011-T015 (crypto tests) before T016-T020 (crypto implementation) ✅ COMPLETE
- T016-T020 block all other implementation (crypto is fundamental) ✅ COMPLETE

**Serialization Dependencies**:
- T021-T023 (CBOR tests) before T024-T025 (CBOR implementation) ✅ COMPLETE
- T024-T025 block T030-T034 (data types need serialization) ✅ COMPLETE

**Core Type Dependencies**:
- T026-T029 (type tests) before T030-T034 (type implementation) ✅ COMPLETE
- T030-T034 block T042-T048 (era implementations need core types) ✅ COMPLETE

**Era Implementation Dependencies**:
- T035-T041 (era tests) should precede era implementations
- T042-T044 (missing Byron/Shelley/Allegra) before full consensus
- T045-T048 (Mary/Alonzo/Babbage/Conway) ✅ COMPLETE

**Major Component Dependencies**:
- Ledger (T030-T048) blocks Consensus (T053-T056) ✅ LEDGER COMPLETE
- Consensus (T053-T056) blocks Network (T061-T065)
- Network (T061-T065) blocks Storage (T070-T073)
- Storage (T070-T073) blocks API (T077-T079)
- API (T077-T079) blocks Node integration (T083-T086)

## Current Status and Next Actions

### ✅ COMPLETED (Major Achievement)
**Phase 3.8: Era Validation Tests** - T035-T041 complete (5,600+ lines)
- All Cardano eras from Byron through Conway with comprehensive validation
- Test-driven development approach with proper failing tests
- Advanced features: VRF proofs, smart contracts, governance mechanisms

**Phase 3.10: Multi-Era Ledger Rules** - T045-T048 complete
- Mary era with multi-asset support
- Alonzo era with Plutus V1 smart contracts
- Babbage era with Plutus V2 and reference inputs
- Conway era with on-chain governance

**Phase 3.11: Consensus Protocol TDD** - T049-T050 complete
- Ouroboros consensus protocol tests with slot leadership and VRF validation
- Chain selection rule tests with fork resolution and quality metrics

### 🚨 IMMEDIATE PRIORITIES
1. **T042-T044**: Complete Byron/Shelley/Allegra era implementations (missing dependencies)
2. **T051-T052**: Complete remaining consensus protocol tests (block production, validation pipeline)
3. **T053-T056**: Begin consensus implementation (tests ready, dependencies met)

### ⚡ READY TO START (Dependencies Met)
- **Consensus Protocol TDD** (T049-T052): Ledger foundation complete
- **Era Tests** (T035-T041): Can run in parallel [P]

## Parallel Execution Examples

**Ready Now - Era Validation Tests (T035-T041)**:
```bash
# All can run in parallel - different test files:
Task: "Byron era validation tests in tests/ledger/test_byron_validation.rs"
Task: "Shelley era validation tests in tests/ledger/test_shelley_validation.rs"
Task: "Allegra era validation tests in tests/ledger/test_allegra_validation.rs"
Task: "Mary era validation tests in tests/ledger/test_mary_validation.rs"
Task: "Alonzo era validation tests in tests/ledger/test_alonzo_validation.rs"
Task: "Babbage era validation tests in tests/ledger/test_babbage_validation.rs"
Task: "Conway era validation tests in tests/ledger/test_conway_validation.rs"
```

**Ready Now - Consensus Tests (T049-T052)**:
```bash
# All can run in parallel - different test files:
Task: "Ouroboros consensus protocol tests in tests/consensus/test_ouroboros.rs"
Task: "Chain selection rule tests in tests/consensus/test_chain_selection.rs"
Task: "Block production tests in tests/consensus/test_block_production.rs"
Task: "Block validation pipeline tests in tests/consensus/test_block_validation.rs"
```

## Notes
- ✅ Cryptographic foundation complete and tested
- ✅ Multi-era ledger rules complete (Mary/Alonzo/Babbage/Conway)
- 🚨 Missing Byron/Shelley/Allegra eras need implementation
- 🚨 Era validation tests needed before consensus implementation
- ⚡ Consensus layer ready to start (dependencies met)
- Follow TDD: Tests before implementation
- [P] tasks run in parallel (different files)
- Sequential tasks share same files
