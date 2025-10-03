# Cardano Node CLI Reference

Complete command-line interface reference for the Cardano Node Rust implementation.

## Table of Contents

- [Quick Start](#quick-start)
- [Global Options](#global-options)
- [Commands](#commands)
  - [run](#run---run-the-node)
  - [query](#query---query-blockchain-state)
  - [transaction](#transaction---transaction-operations)
  - [stake-pool](#stake-pool---stake-pool-operations)
  - [stake-address](#stake-address---stake-address-operations)
  - [address](#address---address-operations)
  - [governance](#governance---governance-operations)
  - [dashboard](#dashboard---interactive-terminal-dashboard)
  - [admin](#admin---node-administration)
- [Examples](#examples)

## Quick Start

```bash
# Run a node on mainnet
cardano-node run \
  --config config.json \
  --topology topology.json \
  --database-path db/ \
  --socket-path node.socket

# Start the interactive dashboard
cardano-node dashboard --socket-path node.socket

# Query chain tip
cardano-node query chain-tip --socket-path node.socket

# Query UTxOs for an address
cardano-node query utxo addr_test1... --socket-path node.socket
```

## Global Options

Available for all commands:

| Option | Description | Default |
|--------|-------------|---------|
| `-v, --verbose` | Enable verbose logging | false |
| `--log-level <LEVEL>` | Set log level (error, warn, info, debug, trace) | info |
| `--log-format <FORMAT>` | Set log format (json, plain) | plain |

## Commands

### `run` - Run the Node

Start the Cardano node with specified configuration.

```bash
cardano-node run [OPTIONS]
```

#### Options

| Option | Description |
|--------|-------------|
| `--config <PATH>` | Path to node configuration file |
| `--topology <PATH>` | Path to network topology file |
| `--database-path <PATH>` | Path to blockchain database directory |
| `--socket-path <PATH>` | Path to node socket for local connections |
| `--host-addr <ADDR>` | Host address to bind the node to |
| `--port <PORT>` | Port number for peer connections |
| `--protocol-magic <NUMBER>` | Protocol magic number for the network |
| `--validate-db` | Perform database validation during startup |
| `--shutdown-ipc <PATH>` | Path to shutdown signal file |
| `--metrics` | Enable Prometheus metrics collection |
| `--metrics-host <HOST>` | Host address for metrics endpoint |
| `--metrics-port <PORT>` | Port for Prometheus metrics endpoint |
| `--byron-genesis <PATH>` | Path to Byron era genesis configuration |
| `--shelley-genesis <PATH>` | Path to Shelley era genesis configuration |
| `--alonzo-genesis <PATH>` | Path to Alonzo era genesis configuration |
| `--conway-genesis <PATH>` | Path to Conway era genesis configuration |
| `--dev-mode` | Enable development mode (warning: less secure) |

#### Example

```bash
cardano-node run \
  --config mainnet-config.json \
  --topology mainnet-topology.json \
  --database-path mainnet-db/ \
  --socket-path mainnet.socket \
  --port 3001 \
  --metrics \
  --metrics-port 12798
```

### `query` - Query Blockchain State

Query various blockchain and node state information.

```bash
cardano-node query <SUBCOMMAND> [OPTIONS]
```

#### Subcommands

##### `chain-tip`

Query the current chain tip.

```bash
cardano-node query chain-tip [--socket-path <PATH>]
```

##### `protocol-parameters`

Query current protocol parameters.

```bash
cardano-node query protocol-parameters [--out-file <PATH>] [--socket-path <PATH>]
```

##### `utxo`

Query UTxOs for an address.

```bash
cardano-node query utxo <ADDRESS> [--out-file <PATH>] [--socket-path <PATH>]
```

##### `stake-pool`

Query stake pool information.

```bash
cardano-node query stake-pool <POOL_ID> [--socket-path <PATH>]
```

##### `ledger-state`

Query ledger state.

```bash
cardano-node query ledger-state [--out-file <PATH>] [--socket-path <PATH>]
```

##### `stake-distribution`

Query stake distribution.

```bash
cardano-node query stake-distribution [--socket-path <PATH>]
```

##### `leadership-schedule`

Query leadership schedule for a stake pool.

```bash
cardano-node query leadership-schedule <POOL_ID> \
  --vrf-signing-key-file <PATH> \
  [--socket-path <PATH>]
```

### `transaction` - Transaction Operations

Build, sign, and submit transactions.

```bash
cardano-node transaction <SUBCOMMAND> [OPTIONS]
```

#### Subcommands

##### `build`

Build a transaction.

```bash
cardano-node transaction build \
  --tx-in <TXHASH#TXIX> \
  --tx-out <ADDRESS+LOVELACE> \
  [--change-address <ADDRESS>] \
  --out-file <PATH> \
  [--protocol-params-file <PATH>]
```

##### `sign`

Sign a transaction.

```bash
cardano-node transaction sign \
  --tx-body-file <PATH> \
  --signing-key-file <PATH> \
  --out-file <PATH>
```

##### `submit`

Submit a signed transaction.

```bash
cardano-node transaction submit \
  --tx-file <PATH> \
  [--socket-path <PATH>]
```

##### `txid`

Calculate transaction ID.

```bash
cardano-node transaction txid --tx-file <PATH>
```

##### `view`

View transaction details.

```bash
cardano-node transaction view <TX_FILE>
```

### `stake-pool` - Stake Pool Operations

Manage stake pool registration and operations.

```bash
cardano-node stake-pool <SUBCOMMAND> [OPTIONS]
```

#### Subcommands

##### `register`

Register a stake pool.

```bash
cardano-node stake-pool register \
  --pool-registration-cert <PATH> \
  --signing-key-file <PATH> \
  --out-file <PATH>
```

##### `deregister`

Deregister a stake pool.

```bash
cardano-node stake-pool deregister <POOL_ID> \
  --epoch <NUMBER> \
  --out-file <PATH>
```

##### `metadata-hash`

Calculate pool metadata hash.

```bash
cardano-node stake-pool metadata-hash <METADATA_FILE>
```

##### `id`

Generate pool ID from verification key.

```bash
cardano-node stake-pool id --cold-verification-key-file <PATH>
```

### `stake-address` - Stake Address Operations

Manage stake address registration and delegation.

```bash
cardano-node stake-address <SUBCOMMAND> [OPTIONS]
```

#### Subcommands

##### `build`

Build a stake address.

```bash
cardano-node stake-address build \
  --stake-verification-key-file <PATH> \
  --network-id <NETWORK> \
  [--out-file <PATH>]
```

##### `register`

Register a stake address.

```bash
cardano-node stake-address register <STAKE_ADDRESS> \
  --key-deposit <LOVELACE> \
  --out-file <PATH>
```

##### `deregister`

Deregister a stake address.

```bash
cardano-node stake-address deregister <STAKE_ADDRESS> \
  --out-file <PATH>
```

##### `delegate`

Delegate stake to a pool.

```bash
cardano-node stake-address delegate <STAKE_ADDRESS> \
  --stake-pool-id <POOL_ID> \
  --out-file <PATH>
```

### `address` - Address Operations

Build and analyze Cardano addresses.

```bash
cardano-node address <SUBCOMMAND> [OPTIONS]
```

#### Subcommands

##### `build`

Build a payment address.

```bash
cardano-node address build \
  [--payment-verification-key-file <PATH>] \
  [--stake-verification-key-file <PATH>] \
  --network-id <NETWORK> \
  [--out-file <PATH>]
```

##### `info`

Get address information.

```bash
cardano-node address info <ADDRESS>
```

### `governance` - Governance Operations

Conway era governance operations.

```bash
cardano-node governance <SUBCOMMAND> [OPTIONS]
```

#### Subcommands

##### `create-action`

Create a governance action.

```bash
cardano-node governance create-action \
  --action-type <TYPE> \
  [--anchor-url <URL>] \
  [--anchor-hash <HASH>] \
  --out-file <PATH>
```

##### `vote`

Vote on a governance action.

```bash
cardano-node governance vote <ACTION_ID> \
  --vote <yes|no|abstain> \
  --signing-key-file <PATH> \
  --out-file <PATH>
```

##### `query`

Query governance state.

```bash
cardano-node governance query [--socket-path <PATH>]
```

### `dashboard` - Interactive Terminal Dashboard

Launch the interactive TUI dashboard for real-time node monitoring.

```bash
cardano-node dashboard [OPTIONS]
```

#### Options

| Option | Description | Default |
|--------|-------------|---------|
| `--socket-path <PATH>` | Path to node socket | none |
| `--refresh-interval <SECONDS>` | Dashboard refresh interval | 2 |

#### Features

- **Real-time Metrics**: CPU, memory, disk usage, network I/O
- **Blockchain Status**: Sync progress, chain tip, block validation
- **Network Info**: Connected peers, bandwidth statistics
- **Live Logs**: Recent node activity and events
- **Interactive Navigation**: Tab switching with 1-4 keys or arrow keys

#### Keyboard Controls

| Key | Action |
|-----|--------|
| `q` | Quit dashboard |
| `1-4` | Switch to tab (Overview, Blockchain, Network, Logs) |
| `←→` | Navigate tabs |

#### Example

```bash
# Start dashboard with 5-second refresh
cardano-node dashboard \
  --socket-path node.socket \
  --refresh-interval 5
```

### `admin` - Node Administration

Administrative commands for node management.

```bash
cardano-node admin <SUBCOMMAND> [OPTIONS]
```

#### Subcommands

##### `shutdown`

Shutdown the node gracefully.

```bash
cardano-node admin shutdown [--socket-path <PATH>]
```

##### `restart`

Restart the node.

```bash
cardano-node admin restart [--socket-path <PATH>]
```

##### `metrics`

Export node metrics.

```bash
cardano-node admin metrics \
  [--format <json|prometheus>] \
  [--out-file <PATH>]
```

##### `db validate`

Validate database integrity.

```bash
cardano-node admin db validate <DB_PATH>
```

##### `db compact`

Compact database.

```bash
cardano-node admin db compact <DB_PATH>
```

##### `db export`

Export database snapshot.

```bash
cardano-node admin db export <DB_PATH> --out-file <PATH>
```

##### `db import`

Import database snapshot.

```bash
cardano-node admin db import <SNAPSHOT_FILE> --db-path <PATH>
```

## Examples

### Running a Mainnet Node

```bash
# Download configs
curl -o config.json https://book.world.dev.cardano.org/environments/mainnet/config.json
curl -o topology.json https://book.world.dev.cardano.org/environments/mainnet/topology.json

# Run node
cardano-node run \
  --config config.json \
  --topology topology.json \
  --database-path mainnet-db \
  --socket-path mainnet.socket \
  --port 3001
```

### Querying Blockchain Data

```bash
# Get chain tip
cardano-node query chain-tip --socket-path mainnet.socket

# Get protocol parameters
cardano-node query protocol-parameters \
  --out-file protocol.json \
  --socket-path mainnet.socket

# Query UTxOs
cardano-node query utxo \
  addr1q... \
  --out-file utxos.json \
  --socket-path mainnet.socket
```

### Building and Submitting a Transaction

```bash
# Build transaction
cardano-node transaction build \
  --tx-in "abc123...#0" \
  --tx-out "addr1q...+2000000" \
  --change-address "addr1q..." \
  --out-file tx.raw \
  --protocol-params-file protocol.json

# Sign transaction
cardano-node transaction sign \
  --tx-body-file tx.raw \
  --signing-key-file payment.skey \
  --out-file tx.signed

# Submit transaction
cardano-node transaction submit \
  --tx-file tx.signed \
  --socket-path mainnet.socket
```

### Stake Pool Operations

```bash
# Create pool metadata hash
cardano-node stake-pool metadata-hash pool-metadata.json

# Register pool
cardano-node stake-pool register \
  --pool-registration-cert pool.cert \
  --signing-key-file cold.skey \
  --signing-key-file owner.skey \
  --out-file pool-reg.tx

# Query leadership schedule
cardano-node query leadership-schedule pool1... \
  --vrf-signing-key-file vrf.skey \
  --socket-path mainnet.socket
```

### Monitoring with Dashboard

```bash
# Launch interactive dashboard
cardano-node dashboard --socket-path mainnet.socket

# With custom refresh interval
cardano-node dashboard \
  --socket-path mainnet.socket \
  --refresh-interval 1
```

### Database Administration

```bash
# Validate database
cardano-node admin db validate mainnet-db/

# Compact database
cardano-node admin db compact mainnet-db/

# Export snapshot
cardano-node admin db export mainnet-db/ \
  --out-file snapshot-2025-10-03.db

# Import snapshot
cardano-node admin db import snapshot-2025-10-03.db \
  --db-path mainnet-db/
```

## Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `CARDANO_NODE_SOCKET_PATH` | Default socket path | none |
| `CARDANO_NODE_NETWORK_ID` | Default network ID | none |
| `RUST_LOG` | Rust log level override | info |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Configuration error |
| 3 | Network error |
| 4 | Database error |
| 5 | Validation error |

## See Also

- [README.md](../README.md) - Project overview
- [CONTRIBUTING.md](../CONTRIBUTING.md) - Contribution guidelines
- [PRODUCTION_READY.md](../PRODUCTION_READY.md) - Deployment guide
