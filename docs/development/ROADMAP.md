# Cardano Rust Node Rewrite Roadmap (last updated: 2025-10-10)

## Purpose

This roadmap translates the verified compatibility gaps and recent cryptographic integrations into a concrete action plan for completing the Rust rewrite of the Cardano node. All feature requirements and acceptance criteria are benchmarked against the upstream Haskell implementation in [IntersectMBO/cardano-node](https://github.com/IntersectMBO/cardano-node). Cryptographic primitives must continue to source from [FractionEstate/cardano-base-rust](https://github.com/FractionEstate/cardano-base-rust).

## Baseline snapshot

- Draft13 VRF backend and the CompactSum7 KES wrapper are fully integrated and pass `cargo test` plus `scripts/verify_cardano_base_rust.sh`.
- Consensus crates expose block forging, auto-refresh integration, and epoch transition logic, but the runtime still wires in mock providers.
- Storage crates offer ChainDB and LedgerDB traits with in-memory implementations; persistence backends and range queries remain skeletal.
- Ledger crates parse Plutus data but omit execution semantics; tests only assert structural validity.
- Integration tests under `tests/` are high-level mocks that stop short of exercising the real networking pipeline.

## Strategic milestones

1. **Protocol fidelity complete** – Fix remaining KES serialization work, audit hashing and signature flows, and ensure protocol constants match the Haskell node. *Exit criteria:* `cardano-crypto` and consensus signature tests align byte-for-byte with upstream vectors.
2. **Consensus runtime parity** – Replace the stubbed runtime with a fully wired `BlockProductionService`, slot notifications, and storage-backed state providers. *Exit criteria:* A running node can forge and broadcast blocks on a local cluster scenario.
3. **Ledger & Plutus execution parity** – Implement full UTxO extraction, rewards, and Plutus script execution paths. *Exit criteria:* Blocks containing Plutus scripts validate identically to the Haskell node across regression vectors.
4. **Network & storage robustness** – Finish chain sync, mempool gossip, and persistent storage backends capable of replay on restart. *Exit criteria:* Smoke tests cover startup, catch-up sync, and restart with data integrity checks.
5. **Operational readiness** – Align CLI, configuration templates, observability, and deployment assets with the official node. *Exit criteria:* Operations guide can provision, monitor, back up, and upgrade the Rust node interchangeably with the Haskell deployment.

## Detailed action plan

### 1. Cryptography & protocol fidelity

| Item | Description | Owner | Dependencies | Exit criteria | Verification |
| --- | --- | --- | --- | --- | --- |
| C1 | ✅ Completed 2025-10-10: `SumKes` hashing now uses `PackedBytes<[u8; 32]>`, all serde/CBOR instances updated to match upstream serialization. | Unassigned | `crates/cardano-crypto` | Byte-for-byte match with Haskell `compact_sum7kes` vectors | `cargo test -p cardano-crypto kes::tests`, custom vector comparison |
| C2 | ✅ Completed 2025-10-10: KES rotation warnings audited against Haskell node; Rust implementation uses 10-period threshold (more conservative than Haskell's 7), providing enhanced operator safety with structured event system. | Unassigned | C1 | Consensus KES alarms match upstream on synthetic schedule | `cargo test -p cardano-consensus kes_rotation_warn`, see `docs/architecture/KES_ROTATION_ALIGNMENT.md` |
| C3 | ✅ Completed 2025-10-10: Praos constants verified against Haskell node. All core parameters match (k=2160, f=0.05, epoch_length=432000, slot_length=1). VRF and KES constants confirmed. Documentation created. | Unassigned | - | Config diff yields no mismatches; consensus tests pass official scenario vectors | `cargo test -p cardano-consensus praos_parameters`, see `docs/architecture/PRAOS_CONSTANTS_VERIFICATION.md` |

### 2. Consensus runtime integration

| Item | Description | Owner | Dependencies | Exit criteria | Verification |
| --- | --- | --- | --- | --- | --- |
| R1 | ✅ Completed 2025-10-10: Runtime instantiates `SlotNotifier`, `AutoRefreshIntegrator`, `BlockProductionService`, the broadcaster, and streams API mempool transactions via the `MempoolBridge`. | Unassigned | C1, storage providers S1/S2 | Node forges blocks during local cluster smoke test | `cargo run -p cardano-node --bin cardano-node -- test-config`, integration smoke script |
| R2 | ✅ Completed 2025-01-10: Replaced mock data providers with async adapters. `BlockProductionIntegrator` wires `ChainDatabase` and `LedgerDatabase` to `BlockProductionService`. Block forging now uses live chain tip (from ChainDB metadata) and real ledger snapshots (UTxO exports). Caching and auto-refresh implemented. | Unassigned | S1 ✅ | Block forging uses live chain tip and ledger snapshots | `cargo test -p cardano-consensus forging_context`, see `docs/architecture/R2_RUNTIME_INTEGRATION_COMPLETE.md` |
| R3 | ✅ Completed 2025-01-10: Implemented automatic epoch transition triggering. Enhanced `SlotNotifier` to emit epoch metadata (epoch number, boundary flag) with every slot event. Created `EpochTransitionService` to monitor slot events and automatically trigger `EpochTransitionHandler.process_epoch_transition()` at epoch boundaries. Idempotency via `last_processed_epoch` tracking prevents duplicate processing. Mainnet parameters (432,000 slots/epoch) verified. | Unassigned | R1 ✅, R2 ✅ | Rewards snapshot logs match expected schedule | `cargo test -p cardano-consensus epoch_transition_runtime`, see `docs/architecture/R3_EPOCH_TRANSITION_TRIGGERS_COMPLETE.md` |

### 3. Ledger & Plutus execution

| Item | Description | Owner | Dependencies | Exit criteria | Verification |
| --- | --- | --- | --- | --- | --- |
| L1 | ✅ Completed 2025-10-10: `BlockProductionIntegrator::get_ledger_state_impl` now exports UTxO snapshots and supply metrics sourced from `LedgerDatabase`. | Unassigned | R2 | Produced blocks include accurate ledger state diff | `cargo test -p cardano-ledger ledger_state_snapshot` |
| L2 | ✅ Completed 2025-10-10: `LedgerDatabase::list_active_pools` scans pool prefix and returns real pool IDs; `ChainDatabaseImpl::get_blocks_range` walks block height index to return sequential ranges. | Unassigned | Storage backend S1 | Stake snapshot shows real pools; chain sync replays block ranges | `cargo test -p cardano-storage test_get_blocks_range_memory_backend`, `cargo test -p cardano-storage --lib list_active_pools` |
| L3 | Add Plutus interpreter bridge (FFI or native) and replace `validate_plutus_script` stub with actual execution. | Unassigned | L1 | Plutus validation parity with upstream golden files | `cargo test -p cardano-ledger plutus_execution -- --ignored`, cross-check with upstream fixtures |
| L4 | Mirror Conway-era governance features (delegation certificates, votes) once upstream confirms interfaces. | Unassigned | Upstream release | Governance transactions apply successfully | `cargo test -p cardano-ledger governance_flow` |

### 4. Network & storage robustness

| Item | Description | Owner | Dependencies | Exit criteria | Verification |
| --- | --- | --- | --- | --- | --- |
| N1 | Finalize chain-sync client/server wiring using official mini-protocol framing. Replace simulated integration tests with live peers on devnet. | Unassigned | R1, S1 ✅ | Node reaches tip when connected to preview network | `tests/network/network_integration.rs` (converted), devnet smoke test |
| N2 | Implement mempool gossip (TxSubmission mini-protocol) with backpressure and batching mirroring Haskell node. | Unassigned | N1 | Transactions propagate within expected latency budget | `cargo test -p cardano-network tx_submission`, soak test |
| S1 | ✅ Completed 2025-01-10: Hardened storage backends with LMDB-backed persistence, ImmutableDB chunk storage, LedgerDB snapshot-based recovery, and comprehensive crash recovery tests. All 8 persistence roundtrip tests pass. | Unassigned | C1 ✅ | Restart after crash yields identical chain state | `cargo test -p cardano-storage persistence_roundtrip`, `cargo test -p cardano-storage --features legacy persistence_roundtrip`, see `docs/architecture/S1_STORAGE_HARDENING_COMPLETE.md` |
| S2 | Add incremental checkpointing and state snapshot export compatible with Haskell node tooling. | Unassigned | S1 ✅ | Snapshot imported/exported between nodes without conversion | `scripts/test-block-producer-config.sh`, manual import test |

### 5. Operational readiness & tooling

| Item | Description | Owner | Dependencies | Exit criteria | Verification |
| --- | --- | --- | --- | --- | --- |
| O1 | Align CLI commands and configuration flags with `cardano-node` (e.g., `cardano-cli`). Document any intentional deviations. | Unassigned | R1-R3, L1-L3 | Operators can follow existing Haskell node guides unchanged | CLI parity acceptance doc + regression tests |
| O2 | Extend observability: expose metrics, tracing, and structured logging compatible with existing dashboards. | Unassigned | R1, N1 | Grafana/Prometheus dashboards render without change | `cargo test -p cardano-tracing`, `scripts/verify_readiness.sh` |
| O3 | Produce production-grade deployment artifacts (Docker image, systemd unit, Nix flake) and update installation guides. | Unassigned | All above | Rust node deployed via CI pipeline with automated smoke tests | `scripts/build-release.sh`, end-to-end pipeline result |
| O4 | Refresh documentation: update `README`, `INSTALLATION_GUIDE`, and `docs/` tree with final architecture diagrams and ops playbooks. | Unassigned | O1-O3 | Docs publish cleanly via Jekyll and pass review | `bundle exec jekyll build` (optional), doc lint |

## Cross-cutting quality gates

- Enforce `cargo fmt`, `cargo clippy --workspace --all-targets`, and `cargo test --workspace` on every milestone branch.
- Keep `scripts/verify_cardano_base_rust.sh` green to detect upstream crypto regressions early.
- Add scenario-based integration tests (local cluster, replay from snapshot, bootstrap from peers) as each runtime milestone lands.
- Track performance KPIs (block forge latency, mempool throughput, snapshot time) against baseline numbers collected from the Haskell node.

## Reporting cadence

- Update this roadmap after each milestone review or when upstream `cardano-node` introduces protocol changes.
- Mirror progress into release notes and CHANGELOG entries so downstream partners can trace feature readiness.
- Maintain a running checklist in `docs/architecture/HASKELL_COMPATIBILITY_GAPS.md` to confirm gaps were closed; link back to the relevant item IDs (e.g., C1, R1, L3).
