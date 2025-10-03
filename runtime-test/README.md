# Runtime Smoke Test

This directory contains a minimal configuration for spinning up the Rust Cardano node locally.

## Prerequisites

- Rust toolchain matching the workspace (see `rust-toolchain.toml` if present).
- `cargo` available on the PATH.

## Quick Start

Run the node with the sample configuration:

```bash
cargo run --bin cardano-node -- run \
  --config runtime-test/config.json \
  --topology runtime-test/topology.json \
  --database-path runtime-test/db \
  --socket-path runtime-test/node.socket \
  --port 3001
```

To keep the smoke test short-lived while still validating subsystem startup, wrap the command in `timeout` and enable info-level logging:

```bash
timeout 10s env RUST_LOG=info cargo run --bin cardano-node -- run \
  --config runtime-test/config.json \
  --topology runtime-test/topology.json \
  --database-path runtime-test/db \
  --socket-path runtime-test/node.socket \
  --port 3001
```

The node will boot, start all subsystems, and then shut down when the timeout elapses. Logs confirm configuration loading, subsystem startup, and graceful shutdown.

## Cleaning Up

After the run you can remove transient artifacts:

```bash
rm -f runtime-test/node.socket
```

The `runtime-test/db` directory is currently empty and serves as the placeholder for ledger state during smoke tests.
