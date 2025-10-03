# Cardano Rust Node Sync Roadmap

This document captures the steps required to evolve the current runtime into a fully syncing Cardano node. It focuses on the gaps discovered while attempting to validate live network behaviour.

## 1. Networking Diffusion

- **Mini-protocol handshake**: Wire `cardano_network::connection::HandshakeProtocol` into the runtime so peers from the topology file can be negotiated at startup.
- **Real TCP connections**: Replace the placeholder sleep loop in `run_network_subsystem` with a task that instantiates `ConnectionManager`, dials configured producers, and maintains keepalive traffic.
- **Protocol muxing**: Attach the ChainSync, BlockFetch, and TxSubmission clients to the multiplexer once the connection is authenticated.

## 2. Chain Synchronisation

- **ChainSync client**: Implement the client state machine to request headers and track chain tips. Reuse `cardano_consensus::ouroboros` primitives for slot/epoch conversions.
- **BlockFetch pipeline**: Integrate the block fetch mini-protocol to download block bodies referenced by headers, respecting pipelining and congestion control heuristics.
- **Ledger integration**: Feed fetched blocks into `cardano_ledger` for validation. Ensure ledger state updates are persisted before acknowledging completion to the network layer.

## 3. Storage Layer

- **Chain DB schema**: Extend `cardano_storage` with immutable/volatile stores mirroring the Haskell implementation (block store, header store, chunk files).
- **Rollback support**: Implement intersection lookups and rollback handling so ChainSync rollbacks are reflected in storage and ledger state.
- **State snapshots**: Add periodic snapshots or checkpointing to accelerate restarts.

## 4. Consensus and Leadership

- **KES/VRF integration**: Connect existing KES/VRF primitives to the runtime so the node can participate in block production once fully synced.
- **Leadership schedule**: Port the leadership schedule calculation to drive block forging in slots where the node is elected.
- **Mempool plumbing**: Integrate transaction selection from storage or external sources when forging blocks.

## 5. Observability and Control

- **Metrics**: Expose slot height, chain density, and peer counts via the tracing/metrics subsystem for monitoring sync progress.
- **CLI enhancements**: Provide commands or REST endpoints for querying current tip, peer list, and ledger status.
- **Error handling**: Ensure subsystem failures trigger restarts or controlled shutdowns with clear diagnostics.

## 6. Testing Strategy

- **Protocol harnesses**: Create integration tests that spin up mock peers implementing ChainSync/BlockFetch to validate the client pipelines.
- **Property tests**: Use proptest to ensure ledger/application invariants hold under random rollback and fork scenarios.
- **End-to-end testnet**: Stand up a small local cluster (node + two peers) to exercise full sync and block production.

## Suggested Milestones

1. **Networking MVP**: Runtime dials configured peers, negotiates version, and stays connected (no block sync yet).
2. **Header sync**: ChainSync client downloads and validates headers until caught up.
3. **Full block sync**: BlockFetch + ledger integration allows the node to reach current tip.
4. **Production readiness**: Add block production, mempool, metrics, and operational tooling.

This plan should be refined as the networking and ledger crates evolve, but it provides a clear sequence for delivering real synchronization capability.
