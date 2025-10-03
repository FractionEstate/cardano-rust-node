# Getting Started Guide

Quick start guide for the Cardano Node Rust implementation.

## Table of Contents

1. [Installation](#installation)
2. [First Run](#first-run)
3. [Using the CLI](#using-the-cli)
4. [Dashboard Guide](#dashboard-guide)
5. [Common Tasks](#common-tasks)
6. [Troubleshooting](#troubleshooting)

## Installation

### Prerequisites

Before you begin, ensure you have:

- **Rust 1.75+** - [Install Rust](https://rustup.rs/)
- **Git** - For cloning the repository
- **15GB+ free disk space** - For blockchain data

### Build from Source

```bash
# Clone the repository
git clone <repository-url>
cd cardano-node-rust

# Build the release binary
cargo build --release --bin cardano-node

# Verify installation
./target/release/cardano-node --version
```

The compiled binary will be at `target/release/cardano-node`.

### Optional: Install System-wide

```bash
# Copy to system path (Linux/macOS)
sudo cp target/release/cardano-node /usr/local/bin/

# Verify
cardano-node --version
```

## First Run

### Step 1: Download Network Configuration

Choose your network and download the configuration files:

#### Mainnet

```bash
# Create config directory
mkdir -p config/mainnet
cd config/mainnet

# Download mainnet configs
curl -o config.json https://book.world.dev.cardano.org/environments/mainnet/config.json
curl -o topology.json https://book.world.dev.cardano.org/environments/mainnet/topology.json
curl -o byron-genesis.json https://book.world.dev.cardano.org/environments/mainnet/byron-genesis.json
curl -o shelley-genesis.json https://book.world.dev.cardano.org/environments/mainnet/shelley-genesis.json
curl -o alonzo-genesis.json https://book.world.dev.cardano.org/environments/mainnet/alonzo-genesis.json
curl -o conway-genesis.json https://book.world.dev.cardano.org/environments/mainnet/conway-genesis.json
```

#### Pre-production Testnet

```bash
mkdir -p config/preprod
cd config/preprod

curl -o config.json https://book.world.dev.cardano.org/environments/preprod/config.json
curl -o topology.json https://book.world.dev.cardano.org/environments/preprod/topology.json
curl -o byron-genesis.json https://book.world.dev.cardano.org/environments/preprod/byron-genesis.json
curl -o shelley-genesis.json https://book.world.dev.cardano.org/environments/preprod/shelley-genesis.json
curl -o alonzo-genesis.json https://book.world.dev.cardano.org/environments/preprod/alonzo-genesis.json
curl -o conway-genesis.json https://book.world.dev.cardano.org/environments/preprod/conway-genesis.json
```

#### Preview Testnet

```bash
mkdir -p config/preview
cd config/preview

curl -o config.json https://book.world.dev.cardano.org/environments/preview/config.json
curl -o topology.json https://book.world.dev.cardano.org/environments/preview/topology.json
curl -o byron-genesis.json https://book.world.dev.cardano.org/environments/preview/byron-genesis.json
curl -o shelley-genesis.json https://book.world.dev.cardano.org/environments/preview/shelley-genesis.json
curl -o alonzo-genesis.json https://book.world.dev.cardano.org/environments/preview/alonzo-genesis.json
curl -o conway-genesis.json https://book.world.dev.cardano.org/environments/preview/conway-genesis.json
```

### Step 2: Validate Configuration

```bash
cardano-node validate \
  --config config/mainnet/config.json \
  --topology config/mainnet/topology.json
```

Expected output:
```
✅ Configuration validation successful
✅ Topology validation successful
✅ Genesis hash verification passed
```

### Step 3: Start the Node

```bash
cardano-node run \
  --config config/mainnet/config.json \
  --topology config/mainnet/topology.json \
  --database-path ./db \
  --socket-path ./node.socket
```

The node will:
1. ✅ Load configuration
2. ✅ Validate genesis files
3. ✅ Connect to P2P network
4. ✅ Begin syncing blockchain
5. ✅ Start metrics servers

## Using the CLI

### Basic Commands

#### Get Node Information

```bash
cardano-node info --socket-path ./node.socket
```

Output:
```
Node Version: 8.7.3
Network: mainnet
Protocol Version: 8.0
Sync Progress: 95.2%
Chain Tip: Block 12345678, Slot 98765432
```

#### Query Chain Tip

```bash
cardano-node query chain-tip --socket-path ./node.socket
```

Output:
```
Chain tip:
  Block: 12345678
  Slot: 98765432
  Hash: abc123def456...
  Epoch: 450
```

#### Get Protocol Parameters

```bash
cardano-node query protocol-parameters --socket-path ./node.socket
```

Output: JSON with all protocol parameters

### Address Operations

#### Generate Payment Keys

```bash
# Generate verification key and signing key
cardano-node address key-gen \
  --verification-key-file payment.vkey \
  --signing-key-file payment.skey
```

#### Build Address

```bash
cardano-node address build \
  --payment-verification-key-file payment.vkey \
  --out-file payment.addr \
  --mainnet
```

#### Query Address UTxOs

```bash
cardano-node query utxo \
  --address $(cat payment.addr) \
  --socket-path ./node.socket
```

### Transaction Operations

#### Build Transaction

```bash
cardano-node transaction build \
  --tx-in "abc123...#0" \
  --tx-out "addr1q...+1000000" \
  --change-address $(cat payment.addr) \
  --out-file tx.raw \
  --socket-path ./node.socket
```

#### Sign Transaction

```bash
cardano-node transaction sign \
  --tx-file tx.raw \
  --signing-key-file payment.skey \
  --out-file tx.signed
```

#### Submit Transaction

```bash
cardano-node transaction submit \
  --tx-file tx.signed \
  --socket-path ./node.socket
```

#### Calculate Transaction ID

```bash
cardano-node transaction txid \
  --tx-file tx.signed
```

### Stake Pool Operations

#### Register Stake Pool

```bash
cardano-node stake-pool registration \
  --pool-pledge 500000000000 \
  --pool-cost 340000000 \
  --pool-margin 0.02 \
  --pool-owner stake.vkey \
  --cold-verification-key-file cold.vkey \
  --vrf-verification-key-file vrf.vkey \
  --out-file pool-registration.cert
```

#### Calculate Pool ID

```bash
cardano-node stake-pool id \
  --cold-verification-key-file cold.vkey
```

### Governance Operations (Conway Era)

#### Create Governance Proposal

```bash
cardano-node governance create-proposal \
  --governance-action-deposit 1000000000 \
  --deposit-return-stake-verification-key-file stake.vkey \
  --anchor-url "https://example.com/proposal.json" \
  --anchor-data-hash "abc123..." \
  --out-file proposal.cert
```

#### Vote on Proposal

```bash
cardano-node governance vote \
  --governance-action-tx-id "abc123...#0" \
  --governance-action-index 0 \
  --vote-choice yes \
  --signing-key-file stake.skey \
  --out-file vote.cert
```

## Dashboard Guide

### Launch Dashboard

```bash
cardano-node dashboard --socket-path ./node.socket
```

Or with custom settings:

```bash
cardano-node dashboard \
  --socket-path ./node.socket \
  --refresh-interval 1
```

### Dashboard Layout

```
┌─────────────────────────────────────────────────────────────┐
│ [1] Overview  [2] Blockchain  [3] Network  [4] Logs         │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  Overview Tab:                                                │
│  ┌─────────────────────────────────────────────────────┐    │
│  │ Node: v8.7.3 | Network: mainnet                     │    │
│  │ Sync: [████████████████░░] 95%                      │    │
│  │ Chain Tip: Block 12345678 | Slot 98765432           │    │
│  │                                                       │    │
│  │ Resources:                                            │    │
│  │   CPU: 25%  [██████░░░░░░░░]                        │    │
│  │   Memory: 2.0 GB                                      │    │
│  │   Disk: 50 GB                                         │    │
│  │                                                       │    │
│  │ Network I/O:                                          │    │
│  │   ↓ 1.2 MB/s | ↑ 450 KB/s                           │    │
│  └─────────────────────────────────────────────────────┘    │
│                                                               │
└─────────────────────────────────────────────────────────────┘
  Press 1-4 to switch tabs | q to quit | ←→ to navigate
```

### Keyboard Controls

| Key | Action |
|-----|--------|
| `1` | Switch to Overview tab |
| `2` | Switch to Blockchain tab |
| `3` | Switch to Network tab |
| `4` | Switch to Logs tab |
| `←` | Navigate left |
| `→` | Navigate right |
| `q` | Quit dashboard |

### Tab Details

#### 1. Overview Tab
- **Node Information**: Version, network, protocol
- **Sync Progress**: Visual progress bar with percentage
- **Chain Status**: Current block, slot, epoch
- **Resources**: CPU, memory, disk usage
- **Network I/O**: Upload/download speeds

#### 2. Blockchain Tab
- **Recent Blocks**: Last 10 blocks
- **Block Details**: Number, hash, tx count, size
- **Transaction Activity**: Transactions per block
- **Slot Information**: Slot numbers and times

#### 3. Network Tab
- **Peer List**: Connected peers with addresses
- **Connection Status**: Active/inactive peers
- **Latency**: Round-trip time for each peer
- **Total Peers**: Connected vs. available

#### 4. Logs Tab
- **Real-time Logs**: Streaming node events
- **Timestamps**: Precise event timing
- **Log Levels**: Info, Warning, Error
- **Auto-scroll**: Automatic scroll to latest

## Common Tasks

### Checking Sync Status

```bash
# Quick status check
cardano-node info --socket-path ./node.socket

# Detailed chain info
cardano-node query chain-tip --socket-path ./node.socket

# Real-time monitoring
cardano-node dashboard --socket-path ./node.socket
```

### Sending ADA

```bash
# 1. Query UTxOs
cardano-node query utxo \
  --address $(cat payment.addr) \
  --socket-path ./node.socket > utxos.json

# 2. Build transaction (2 ADA = 2000000 lovelace)
cardano-node transaction build \
  --tx-in "$(jq -r 'keys[0]' utxos.json)" \
  --tx-out "addr1_recipient+2000000" \
  --change-address $(cat payment.addr) \
  --out-file tx.raw \
  --socket-path ./node.socket

# 3. Sign transaction
cardano-node transaction sign \
  --tx-file tx.raw \
  --signing-key-file payment.skey \
  --out-file tx.signed

# 4. Submit transaction
cardano-node transaction submit \
  --tx-file tx.signed \
  --socket-path ./node.socket

# 5. Get transaction ID
cardano-node transaction txid --tx-file tx.signed
```

### Setting Up Stake Pool

```bash
# 1. Generate cold keys
cardano-node address key-gen \
  --verification-key-file cold.vkey \
  --signing-key-file cold.skey

# 2. Generate VRF keys
cardano-node address key-gen \
  --verification-key-file vrf.vkey \
  --signing-key-file vrf.skey

# 3. Generate KES keys
cardano-node address key-gen \
  --verification-key-file kes.vkey \
  --signing-key-file kes.skey

# 4. Get pool ID
cardano-node stake-pool id \
  --cold-verification-key-file cold.vkey > pool.id

# 5. Create registration certificate
cardano-node stake-pool registration \
  --pool-pledge 500000000000 \
  --pool-cost 340000000 \
  --pool-margin 0.02 \
  --pool-owner stake.vkey \
  --cold-verification-key-file cold.vkey \
  --vrf-verification-key-file vrf.vkey \
  --out-file pool-registration.cert
```

### Delegating Stake

```bash
# 1. Create delegation certificate
cardano-node stake-address delegation \
  --stake-verification-key-file stake.vkey \
  --pool-id $(cat pool.id) \
  --out-file delegation.cert

# 2. Build transaction with certificate
cardano-node transaction build \
  --tx-in "utxo_hash#index" \
  --certificate-file delegation.cert \
  --change-address $(cat payment.addr) \
  --out-file tx.raw \
  --socket-path ./node.socket

# 3. Sign with both payment and stake keys
cardano-node transaction sign \
  --tx-file tx.raw \
  --signing-key-file payment.skey \
  --signing-key-file stake.skey \
  --out-file tx.signed

# 4. Submit
cardano-node transaction submit \
  --tx-file tx.signed \
  --socket-path ./node.socket
```

## Troubleshooting

### Node Won't Start

**Issue**: Node fails to start

```bash
# Check configuration
cardano-node validate \
  --config config.json \
  --topology topology.json

# Check socket path permissions
ls -la node.socket

# Check if port is already in use
lsof -i :3001  # Default P2P port
```

### Slow Sync

**Issue**: Blockchain sync is very slow

```bash
# Check peer connections
cardano-node query chain-tip --socket-path ./node.socket

# Monitor in dashboard
cardano-node dashboard --socket-path ./node.socket

# Check disk I/O
iostat -x 1

# Check available peers in topology
cat topology.json | jq '.bootstrapPeers'
```

### Socket Connection Error

**Issue**: Cannot connect to node socket

```bash
# Verify socket exists
ls -la node.socket

# Check socket path environment
export CARDANO_NODE_SOCKET_PATH=./node.socket

# Verify node is running
ps aux | grep cardano-node

# Check socket permissions
chmod 660 node.socket
```

### Out of Memory

**Issue**: Node consumes too much memory

```bash
# Check current memory usage
cardano-node info --socket-path ./node.socket

# Adjust config.json LedgerDB settings
{
  "LedgerDB": {
    "Backend": "OnDisk",  // Use disk instead of memory
    "SnapshotInterval": 4320
  }
}
```

### Genesis Hash Mismatch

**Issue**: Genesis file hash verification fails

```bash
# Re-download genesis files
curl -o byron-genesis.json https://book.world.dev.cardano.org/environments/mainnet/byron-genesis.json

# Verify hash manually
blake2b byron-genesis.json

# Compare with config.json
cat config.json | jq '.ByronGenesisHash'
```

### Transaction Fails

**Issue**: Transaction submission rejected

```bash
# Check transaction
cardano-node transaction view --tx-file tx.signed

# Verify UTxOs still exist
cardano-node query utxo \
  --address $(cat payment.addr) \
  --socket-path ./node.socket

# Check protocol parameters (fees, etc.)
cardano-node query protocol-parameters \
  --socket-path ./node.socket
```

## Next Steps

Now that you have the node running:

1. **Explore the CLI** - See [CLI_REFERENCE.md](CLI_REFERENCE.md) for all commands
2. **Use the API** - Check [API_REFERENCE.md](API_REFERENCE.md) for REST/WebSocket APIs
3. **Monitor Performance** - Use the dashboard for real-time insights
4. **Join the Network** - Connect to mainnet and contribute
5. **Run a Stake Pool** - Set up your own pool (coming soon)

## Resources

- **Documentation**: [docs/](.)
- **Main README**: [../README.md](../README.md)
- **Network Configs**: https://book.world.dev.cardano.org/environments.html
- **Cardano Docs**: https://docs.cardano.org/

## Getting Help

- Check the [Troubleshooting](#troubleshooting) section
- Review [FEATURES.md](FEATURES.md) for feature status
- See [API_REFERENCE.md](API_REFERENCE.md) for API details
- Read [CLI_REFERENCE.md](CLI_REFERENCE.md) for command help
