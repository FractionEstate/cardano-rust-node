# Block Production Runtime Wiring Guide (last reviewed: 2025-10-05)

## Current state summary

The consensus runner in `crates/cardano-node/src/run/mod.rs` still emits placeholder events and never instantiates `BlockProductionService`. At the same time, the consensus crate already exposes all building blocks required to forge blocks against real storage:

- `block_production_service.rs` drives slot events, leadership checks, mempool intake, and emits `BlockProductionEvent`s.
- `block_forging.rs` implements the Praos-based forging pipeline (KES rotation, VRF proofs, transaction selection).
- `block_production_integration.rs` can wire a `ChainDatabase` + `LedgerDatabase` pair into the service and keep caches fresh.
- `slot_notifier.rs` provides a slot clock compatible with the service.

This guide shows how to assemble these pieces today so that the node runtime can be replaced with a concrete implementation.

## Required building blocks

| Component | Location | Purpose |
| --- | --- | --- |
| `BlockProductionService`, `BlockProductionConfig` | `crates/cardano-consensus/src/block_production_service.rs` | Slot loop, event emission, and KES lifecycle management. |
| `BlockForger`, `ForgingConfig` | `crates/cardano-consensus/src/block_forging.rs` | Slot leadership checks, block body/header construction, KES signing. |
| `LeadershipCalculator`, `StakeDistribution` | `crates/cardano-consensus/src/leadership.rs` and `ouroboros.rs` | VRF threshold computation using current stake figures. |
| `BlockProductionIntegrator`, `AutoRefreshIntegrator` | `crates/cardano-consensus/src/block_production_integration.rs` | Provides real chain tip and ledger state via ChainDB/LedgerDB callbacks. |
| `ChainDatabaseImpl`, `LedgerDatabaseImpl` | `crates/cardano-storage/src/chaindb/mod.rs`, `crates/cardano-storage/src/ledgerdb/mod.rs` | Storage adapters that satisfy the integrator traits. |
| `SlotNotifier`, `SlotNotifierConfig` | `crates/cardano-consensus/src/slot_notifier.rs` | Tokio-based slot clock that broadcasts `SlotEvent`s. |

## Wiring example

The snippet below lives inside an async context (e.g. a Tokio task). It uses the in-memory storage backend for clarity; swap in the LMDB backend once the `legacy` feature is enabled.

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Result;
use cardano_consensus::{
    AutoRefreshIntegrator,
    BlockForger,
    BlockProductionConfig,
    BlockProductionOperationalCertificate,
    BlockProductionService,
    EpochNo,
    ForgingConfig,
    LeadershipCalculator,
    PoolId,
    ProtocolParameters,
    SlotNotifier,
    SlotNotifierConfig,
    StakeDistribution,
    VrfKey,
    KesKey,
};
use cardano_consensus::block_production::Transaction;
use cardano_crypto::{Blake2b256Hash, Ed25519KeyHash};
use cardano_storage::backends::{MemoryBackend, StorageBackend};
use cardano_storage::{ChainDatabaseImpl, LedgerDatabaseImpl};
use tokio::sync::{broadcast, mpsc};

async fn run_block_production_example() -> Result<()> {
    // 1. Storage backends --------------------------------------------------
    let backend = Arc::new(MemoryBackend::new());
    backend.init().await?;

    let chaindb = Arc::new(ChainDatabaseImpl::new(backend.clone()));
    let ledgerdb = Arc::new(LedgerDatabaseImpl::new(backend.clone()));

    // 2. Stake distribution + leadership calculator -----------------------
    let pool_id = PoolId(Blake2b256Hash::hash(b"example_pool"));
    let pool_stake = 1_000_000u64; // TODO: load from LedgerDB
    let total_stake = pool_stake;   // TODO: load from LedgerDB

    let mut pools = HashMap::new();
    pools.insert(pool_id.clone(), pool_stake);

    let stake_distribution = StakeDistribution {
        pools,
        total_stake,
    };

    let protocol_params = ProtocolParameters::testnet();
    let epoch_nonce = Blake2b256Hash::hash(b"epoch_nonce"); // TODO: derive from chain state
    let current_epoch = EpochNo(0); // TODO: read from chain metadata

    let leadership = LeadershipCalculator::new(
        stake_distribution.clone(),
        protocol_params.clone(),
        epoch_nonce,
        current_epoch,
    );

    // 3. Block forger ------------------------------------------------------
    let vrf_key = VrfKey::for_pool(&pool_id);            // TODO: load VRF key from disk
    let kes_key = KesKey::new(6);                        // TODO: load/rotate KES key
    let operational_cert = BlockProductionOperationalCertificate {
        hot_vkey: Ed25519KeyHash::from_test_data(b"hot_vkey"), // TODO: load real cert
        sequence_number: 0,
        kes_period: 0,
        sigma: Blake2b256Hash::hash(b"cold_signature"),
    };

    let forger = BlockForger::new(
        pool_id.clone(),
        vrf_key,
        kes_key,
        operational_cert,
        pool_stake,
        leadership,
    );

    // 4. Block production service -----------------------------------------
    let bp_config = BlockProductionConfig {
        pool_id: pool_id.clone(),
        pool_stake,
        total_stake,
        active_slot_coeff: protocol_params.active_slot_coefficient,
        epoch: current_epoch,
        epoch_nonce,
        forging_config: ForgingConfig::default(),
    };

    let mut service = BlockProductionService::new(bp_config, forger);

    // 5. Storage integrator ------------------------------------------------
    let refresh = AutoRefreshIntegrator::new(
        chaindb.clone(),
        ledgerdb.clone(),
        Duration::from_secs(20),
    );

    refresh
        .integrator()
        .wire_to_service(&mut service)
        .await?;

    let refresh_handle = refresh.start_auto_refresh();

    // 6. Slot clock + channels --------------------------------------------
    let slot_notifier = Arc::new(SlotNotifier::new(SlotNotifierConfig {
        genesis_time: SystemTime::now(), // TODO: load from configuration file
        slot_length_secs: protocol_params.slot_length,
        ..SlotNotifierConfig::default()
    }));

    let (mempool_tx, mempool_rx) = mpsc::channel::<Vec<Transaction>>(1_000);
    let (block_tx, mut block_rx) = mpsc::channel(64);
    let (_shutdown_tx, mut shutdown_rx) = broadcast::channel::<()>(1);

    let service = Arc::new(service);

    let notifier_handle = {
        let notifier = slot_notifier.clone();
        tokio::spawn(async move {
            if let Err(err) = notifier.run().await {
                tracing::error!(%err, "slot notifier stopped");
            }
        })
    };

    let service_handle = {
        let runnable = service.clone();
        let notifier = slot_notifier.clone();
        tokio::spawn(async move {
            if let Err(err) = runnable.run(notifier, mempool_rx, block_tx).await {
                tracing::error!(%err, "block production service stopped");
            }
        })
    };

    // Optional: listen for forged blocks ----------------------------------
    let block_listener = tokio::spawn(async move {
        while let Some(block) = block_rx.recv().await {
            tracing::info!(slot = block.header.slot.0, "forged block ready");
            // TODO: persist block, broadcast to peers, update ChainDB
        }
    });

    // Example: push an empty mempool snapshot to unblock the loop ---------
    mempool_tx.send(Vec::<Transaction>::new()).await?;

    // Wait for shutdown signal --------------------------------------------
    shutdown_rx.recv().await.ok();

    // Tear down ------------------------------------------------------------
    refresh_handle.abort();
    notifier_handle.abort();
    service_handle.abort();
    block_listener.abort();

    Ok(())
}
```

### Key points from the example

1. **Storage** – `MemoryBackend` keeps the example self-contained. For production, enable the `legacy` feature and instantiate `LmdbBackend`, or switch to the new CardanoDB backend once it exposes the same traits.
2. **Stake data** – all stake figures are placeholders; the epoch transition handler should populate snapshots inside `LedgerDatabase`, which you can query here instead of constants.
3. **Keys and certificates** – replace the `from_test_data` helpers with real pool credentials and hot/cold key material loaded from disk.
4. **Slot clock** – the notifier uses `SystemTime::now()` for genesis. Load `systemStart` and slot length from the configuration JSON so slots align with network time.
5. **Mempool** – the service consumes `Vec<Transaction>` batches. Wire this to the actual mempool subsystem once it is available; the example sends an empty batch to demonstrate the pathway.

## Production TODOs

- Implement persistence of forged blocks back into `ChainDatabase` and trigger network broadcast (see `crates/cardano-consensus/src/block_broadcaster.rs`).
- Replace the shutdown channel with the node runtime's broadcast sender used in `NodeRuntime::shutdown`.
- Feed new stake distribution snapshots from `EpochTransitionHandler` once it is integrated so the leadership calculator sees real data.
- Extend the storage integrator to populate `SimplifiedLedgerState.utxo_set`; this removes reliance on the placeholder values returned today.

## Observability checkpoints

- When the integrator is wired correctly, the `[WARN] No chain tip provider set` / `[WARN] No ledger state provider set` messages disappear from `BlockProductionService::build_forging_context`.
- `BlockProductionService::subscribe()` can be used to monitor `BlockProductionEvent::BlockForged` events for dashboard metrics.
- `AutoRefreshIntegrator` logs cache refreshes at `DEBUG` level; consider tying those into structured metrics once Prometheus exporters in `crates/cardano-consensus/src/metrics.rs` are enabled.
