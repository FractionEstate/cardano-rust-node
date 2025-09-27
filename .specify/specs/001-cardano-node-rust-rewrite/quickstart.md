# Cardano Node Rust Quickstart Guide

## Prerequisites

### System Requirements
- **OS**: Linux (Ubuntu 20.04+, RHEL 8+), macOS 12+, Windows 10+ with WSL2
- **CPU**: 8+ cores recommended (minimum 4 cores)
- **Memory**: 32GB RAM recommended (minimum 16GB)
- **Storage**: 500GB+ SSD (NVMe recommended for mainnet)
- **Network**: Stable internet connection (100+ Mbps recommended)

### Development Tools
```bash
# Install Rust 1.75+
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
rustup toolchain install 1.75.0
rustup default 1.75.0

# Install additional tools
rustup component add clippy rustfmt
cargo install cargo-audit cargo-outdated

# Install system dependencies (Ubuntu/Debian)
sudo apt update
sudo apt install -y build-essential pkg-config libssl-dev
sudo apt install -y protobuf-compiler cmake

# Install system dependencies (macOS)
brew install protobuf cmake pkg-config openssl
```

## Quick Start - Testnet Node

### 1. Clone and Build
```bash
git clone https://github.com/cardano-rust/cardano-node-rust.git
cd cardano-node-rust

# Build optimized release version
cargo build --release --workspace

# Verify build
./target/release/cardano-node --version
```

### 2. Download Configuration Files
```bash
# Create configuration directory
mkdir -p ~/.cardano-node-rust/testnet

# Download testnet configuration (placeholder URLs)
curl -o ~/.cardano-node-rust/testnet/config.json \
  https://book.world.dev.cardano.org/environments/preprod/config.json

curl -o ~/.cardano-node-rust/testnet/topology.json \
  https://book.world.dev.cardano.org/environments/preprod/topology.json

curl -o ~/.cardano-node-rust/testnet/genesis-byron.json \
  https://book.world.dev.cardano.org/environments/preprod/byron-genesis.json

curl -o ~/.cardano-node-rust/testnet/genesis-shelley.json \
  https://book.world.dev.cardano.org/environments/preprod/shelley-genesis.json

curl -o ~/.cardano-node-rust/testnet/genesis-alonzo.json \
  https://book.world.dev.cardano.org/environments/preprod/alonzo-genesis.json

curl -o ~/.cardano-node-rust/testnet/genesis-conway.json \
  https://book.world.dev.cardano.org/environments/preprod/conway-genesis.json
```

### 3. Start Testnet Node
```bash
# Run node in foreground (for testing)
./target/release/cardano-node run \
  --config ~/.cardano-node-rust/testnet/config.json \
  --topology ~/.cardano-node-rust/testnet/topology.json \
  --database-path ~/.cardano-node-rust/testnet/db \
  --socket-path ~/.cardano-node-rust/testnet/node.socket \
  --port 3001

# Or run as systemd service (production)
sudo cp scripts/cardano-node-rust.service /etc/systemd/system/
sudo systemctl enable cardano-node-rust
sudo systemctl start cardano-node-rust
```

### 4. Verify Node Operation
```bash
# Check sync progress
curl http://localhost:3001/api/v1/chain/tip

# View logs
tail -f ~/.cardano-node-rust/testnet/logs/node.log

# Check service status (if using systemd)
systemctl status cardano-node-rust
```

## Development Workflow

### 1. Development Environment Setup
```bash
# Clone repository
git clone https://github.com/cardano-rust/cardano-node-rust.git
cd cardano-node-rust

# Install development dependencies
cargo install cargo-watch cargo-expand

# Setup pre-commit hooks
./scripts/setup-hooks.sh

# Run all tests to verify setup
cargo test --workspace
```

### 2. Running Tests

#### Unit Tests
```bash
# Run all unit tests
cargo test --lib

# Run specific crate tests
cargo test -p cardano-crypto
cargo test -p cardano-consensus

# Run with output
cargo test --lib -- --nocapture
```

#### Integration Tests
```bash
# Run integration tests
cargo test --test '*'

# Run specific integration test
cargo test --test crypto_compat

# Run with tracing enabled
RUST_LOG=debug cargo test --test consensus_compat
```

#### Property-Based Tests
```bash
# Run property tests (longer duration)
cargo test --release -- --ignored proptest

# Run with specific seed for reproducibility
PROPTEST_CASES=10000 cargo test test_ed25519_compatibility
```

### 3. Performance Benchmarking
```bash
# Run benchmarks
cargo bench

# Compare with baseline
cargo bench -- --save-baseline current
cargo bench -- --baseline current

# Profile memory usage
cargo install cargo-profdata
cargo profdata -- --bench consensus_bench
```

### 4. Code Quality Checks
```bash
# Format code
cargo fmt --all

# Check linting
cargo clippy --workspace -- -D warnings

# Security audit
cargo audit

# Check for outdated dependencies
cargo outdated
```

## Configuration Guide

### Node Configuration (config.json)
```json
{
  "Protocol": "Cardano",
  "ProtocolParameters": {
    "ProtocolVersion": {
      "Major": 8,
      "Minor": 0
    }
  },
  "GenesisFile": "genesis-shelley.json",
  "RequiresNetworkMagic": "RequiresNoMagic",
  "EnableLogMetrics": true,
  "EnableLogging": true,
  "minSeverity": "Info",
  "TraceBlockFetchClient": false,
  "TraceBlockFetchDecisions": false,
  "TraceBlockFetchProtocol": false,
  "TraceBlockFetchProtocolSerialised": false,
  "TraceBlockFetchServer": false,
  "TraceChainDb": true,
  "TraceChainSyncClient": false,
  "TraceChainSyncBlockServer": false,
  "TraceChainSyncHeaderServer": false,
  "TraceChainSyncProtocol": false
}
```

### Database Configuration
```json
{
  "storage": {
    "immutable_db": {
      "path": "./db/immutable",
      "backend": "lmdb",
      "max_size_gb": 100
    },
    "volatile_db": {
      "path": "./db/volatile",
      "backend": "rocksdb",
      "cache_size_mb": 1024
    },
    "ledger_db": {
      "path": "./db/ledger",
      "backend": "rocksdb",
      "cache_size_mb": 2048
    }
  }
}
```

### Network Configuration (topology.json)
```json
{
  "Producers": [
    {
      "addr": "preprod-node.world.dev.cardano.org",
      "port": 30000,
      "valency": 2
    },
    {
      "addr": "preprod.cardano-relay.stakenpool.com",
      "port": 30000,
      "valency": 1
    }
  ]
}
```

## Validation Checklist

### Node Health Verification
- [ ] Node starts without errors
- [ ] Sync progress increases over time
- [ ] Memory usage remains stable
- [ ] Network connections established
- [ ] API endpoints respond correctly
- [ ] Logs show no critical errors

### API Functionality Tests
```bash
# Test chain tip endpoint
curl -s http://localhost:3001/api/v1/chain/tip | jq .

# Test block retrieval (replace with actual hash)
BLOCK_HASH="abc123..."
curl -s "http://localhost:3001/api/v1/blocks/$BLOCK_HASH" | jq .

# Test UTXO query (replace with actual address)
ADDRESS="addr_test1..."
curl -s "http://localhost:3001/api/v1/addresses/$ADDRESS/utxos" | jq .

# Test transaction submission (with valid CBOR)
curl -X POST http://localhost:3001/api/v1/transactions \
  -H "Content-Type: application/cbor" \
  --data-binary @signed_tx.cbor
```

### Performance Validation
```bash
# Check sync speed (blocks/second)
./scripts/measure-sync-speed.sh

# Monitor resource usage
./scripts/monitor-resources.sh

# Compare with Haskell node
./scripts/compare-performance.sh
```

### Compatibility Testing
```bash
# Cross-validation with Haskell node
cargo test --test haskell_compat

# CBOR serialization compatibility
cargo test --test cbor_compat

# Crypto operation compatibility
cargo test --test crypto_compat
```

## Troubleshooting

### Common Issues

#### Node Won't Start
```bash
# Check configuration files
./target/release/cardano-node validate-config \
  --config ~/.cardano-node-rust/testnet/config.json

# Verify file permissions
ls -la ~/.cardano-node-rust/testnet/

# Check for port conflicts
netstat -tulpn | grep 3001
```

#### Sync Issues
```bash
# Clear corrupted database
rm -rf ~/.cardano-node-rust/testnet/db
mkdir -p ~/.cardano-node-rust/testnet/db

# Check network connectivity
./scripts/test-network-connectivity.sh

# Verify topology configuration
./scripts/validate-topology.sh
```

#### Memory Issues
```bash
# Check current memory usage
./scripts/memory-analysis.sh

# Adjust cache sizes in config
vim ~/.cardano-node-rust/testnet/config.json

# Monitor memory over time
./scripts/memory-profiler.sh
```

### Debug Mode
```bash
# Enable debug logging
RUST_LOG=debug ./target/release/cardano-node run \
  --config ~/.cardano-node-rust/testnet/config.json

# Enable specific module debug
RUST_LOG=cardano_consensus=debug,cardano_network=info \
  ./target/release/cardano-node run

# Save debug output
RUST_LOG=debug ./target/release/cardano-node run 2>&1 | \
  tee debug.log
```

### Getting Help
- **Documentation**: https://docs.cardano-node-rust.org
- **Issues**: https://github.com/cardano-rust/cardano-node-rust/issues
- **Discussions**: https://github.com/cardano-rust/cardano-node-rust/discussions
- **Discord**: https://discord.gg/cardano-rust
- **Forum**: https://forum.cardano.org/c/developers/rust-node

## Next Steps

1. **Mainnet Setup**: Follow mainnet configuration guide
2. **Stake Pool Operation**: Set up block production keys
3. **Monitoring**: Configure Prometheus/Grafana dashboards
4. **Automation**: Set up automated updates and backups
5. **Contributing**: Read CONTRIBUTING.md for development guidelines
