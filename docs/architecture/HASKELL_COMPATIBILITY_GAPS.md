# Haskell Compatibility Gaps (last reviewed: 2025-10-05)

## Purpose

This document catalogues the verified differences between this Rust node and the upstream Haskell implementation. Every statement below was checked against the current sources on 2025-10-05 so downstream work can rely on it without guesswork.


## Confirmed implementation snapshot

- **Consensus building blocks**
  - `crates/cardano-consensus/src/block_forging.rs` implements the forging pipeline (leadership checks, KES rotation, and block validation before broadcast).
  - `crates/cardano-consensus/src/block_production_service.rs` manages slot notifications, mempool intake, KES expiry warnings, and event broadcasting.
  - `crates/cardano-consensus/src/leadership.rs` provides the Praos VRF threshold calculations consumed by the forger and service.
  - `crates/cardano-consensus/src/block_production_integration.rs` ships an integration layer ready to hook `ChainDatabase` and `LedgerDatabase` into the forging service.
- **Storage layer foundations**
  - `crates/cardano-storage/src/chaindb/mod.rs` persists blocks, transactions, and chain metadata via the `ChainDatabase` trait and the `ChainDatabaseImpl` adapter.
  - `crates/cardano-storage/src/ledgerdb/mod.rs` exposes APIs for UTxOs, stake pools, delegations, rewards, snapshots, and protocol parameters with an in-memory `MemoryBackend` and optional LMDB/RocksDB backends (behind the `legacy` feature).
- **Epoch transitions**
  - `crates/cardano-consensus/src/epoch_transition.rs` implements nonce evolution, stake snapshotting, reward calculation, and snapshot persistence hooks; it is wired against `LedgerDatabase` but not yet invoked by the runtime.


## Confirmed gaps

| Area | Current behaviour | Evidence |
| --- | --- | --- |
| Consensus runtime integration | The node runtime still runs a stub; `run_consensus_subsystem` in `crates/cardano-node/src/run/mod.rs` simply sleeps and emits `NodeEvent::BlockReceived` with the comment `// Simulate periodic consensus activity`. No code path there instantiates `BlockProductionService`, `SlotNotifier`, or the storage integrator. | `crates/cardano-node/src/run/mod.rs` |
| Block production data sources | Without wiring, `BlockProductionService::build_forging_context` falls back to mock data, printing `[WARN] ... using mock data` and hard-coding `Blake2b256Hash::hash(b"prev_block")`. Outside of the integrator module nothing calls `set_chain_tip_provider` or `set_ledger_state_provider`, so the mock path is always taken today. | `crates/cardano-consensus/src/block_production_service.rs` |
| Ledger data supplied to block production | `BlockProductionIntegrator::get_ledger_state_impl` currently returns an empty `utxo_set` plus constant supply/treasury/reserve values because full UTxO extraction is marked TODO. The auto-refresh wrapper therefore keeps refreshing placeholder data. | `crates/cardano-consensus/src/block_production_integration.rs` |
| Stake snapshot inputs | The epoch transition logic requests active pools via `LedgerDatabase::list_active_pools()`, but the default implementation returns `Ok(Vec::new())`, and `ChainDatabaseImpl::get_blocks_range` is likewise a stub returning `Vec::new()`. This prevents real stake and chain data from flowing into snapshots. | `crates/cardano-storage/src/ledgerdb/mod.rs`, `crates/cardano-storage/src/chaindb/mod.rs` |
| Integration test realism | The long-running integration tests under `tests/integration/` simulate network and consensus behaviour instead of talking to the real node. Example: `tests/integration/mainnet_sync_test.rs` states `// Mock implementation - in production this would connect to real mainnet nodes` and just increments counters in a loop. | `tests/integration/mainnet_sync_test.rs` |
| Plutus execution | Plutus assets are parsed, but execution is stubbed: `validate_plutus_script` in `tests/consensus/test_block_validation.rs` only checks script size/version and never interprets bytecode. There is no Plutus interpreter in `crates/cardano-ledger`. | `tests/consensus/test_block_validation.rs` |

## Suggested next steps

- Replace the consensus runtime stub with a real pipeline that creates `SlotNotifier`, `BlockProductionService`, and `AutoRefreshIntegrator`, then feeds forged blocks into the network broadcaster.
- Extend `BlockProductionIntegrator` to populate `SimplifiedLedgerState` with real UTxO snapshots (consider a paginated fetch from `LedgerDatabase`) and refresh on new blocks instead of fixed intervals.
- Implement pool iteration in `LedgerDatabase::list_active_pools` and block range queries in `ChainDatabaseImpl::get_blocks_range` so that epoch transitions and chain sync operate on real data.
- Gradually convert the mocked integration tests into end-to-end tests that exercise the actual networking and storage stacks once the runtime pipeline exists.
- Introduce a Plutus execution bridge (via bindings or a Rust interpreter) and move the current structural checks into preflight validation.


## Verification checklist

- Run the targeted consensus unit tests while iterating: `cargo test --package cardano-consensus block_production_service` exercises the forging service and its KES guards.
- For storage-side work, `cargo test --package cardano-storage` ensures the in-memory backend, ChainDB, and LedgerDB contracts remain intact.
- When touching integration tests, use `cargo test --package tests --test integration -- --ignored` to verify the mocked flows until real end-to-end tests replace them.
