# Cardano Rust Node - Quick Start Guide

> **Get your Cardano Rust node running in under 10 minutes**

---

## 🚀 Five-Minute Start

### Step 1: Install (2 minutes)

**Linux/macOS:**
```bash
# Download and install
curl -sSL https://get.cardano-rust-node.io | sh

# Verify
cardano-node --version
```

**Windows (WSL2):**
```bash
# Inside WSL2 Ubuntu
curl -sSL https://get.cardano-rust-node.io | sh
cardano-node --version
```

### Step 2: Initialize (1 minute)

```bash
# Create directory
mkdir ~/cardano-node && cd ~/cardano-node

# Initialize for preview testnet
cardano-node init --network preview
```

This downloads all required configuration files automatically.

### Step 3: Start Node (1 minute)

```bash
# Start the node
cardano-node run \
  --config config.json \
  --topology topology.json \
  --database-path db/ \
  --socket-path node.socket
```

### Step 4: Verify (1 minute)

Open a **new terminal** and run:

```bash
# Set socket path
export CARDANO_NODE_SOCKET_PATH=~/cardano-node/node.socket

# Check sync progress
cardano-node query tip
```

**Expected output:**
```json
{
  "block": 123456,
  "epoch": 450,
  "era": "Conway",
  "hash": "abc123...",
  "slot": 123456789,
  "syncProgress": "5.23%"
}
```

### Step 5: Wait for Sync (2-24 hours)

Your node is now syncing! Initial sync time depends on your hardware:
- **Preview testnet**: 30 minutes - 2 hours
- **Mainnet with SSD**: 16-24 hours
- **Mainnet with HDD**: 48-72 hours

---

## 📚 Common Tasks

### Check Sync Status

```bash
cardano-node query tip
```

### Query Protocol Parameters

```bash
cardano-node query protocol-parameters
```

### Create a Wallet

```bash
# Generate payment keys
cardano-node address key-gen \
  --verification-key-file payment.vkey \
  --signing-key-file payment.skey

# Generate stake keys
cardano-node stake-address key-gen \
  --verification-key-file stake.vkey \
  --signing-key-file stake.skey

# Build payment address
cardano-node address build \
  --payment-verification-key-file payment.vkey \
  --stake-verification-key-file stake.vkey \
  --out-file payment.addr \
  --testnet-magic 2

# Display your address
cat payment.addr
```

### Query Your Balance

```bash
# Get your address
ADDRESS=$(cat payment.addr)

# Query UTxOs
cardano-node query utxo --address $ADDRESS
```

### Send a Transaction

```bash
# Set variables
RECIPIENT="addr_test1..."
AMOUNT=1000000  # 1 ADA in lovelace

# Build transaction
cardano-node transaction build \
  --tx-in "YOUR_UTXO_HASH#INDEX" \
  --tx-out "$RECIPIENT+$AMOUNT" \
  --change-address $(cat payment.addr) \
  --testnet-magic 2 \
  --out-file tx.raw

# Sign transaction
cardano-node transaction sign \
  --tx-file tx.raw \
  --signing-key-file payment.skey \
  --out-file tx.signed

# Submit transaction
cardano-node transaction submit --tx-file tx.signed

# Check transaction status
cardano-node transaction txid --tx-file tx.signed
```

---

## 🔧 Configuration

### Run as Background Service

**Linux (systemd):**

```bash
# Create service file
sudo tee /etc/systemd/system/cardano-node.service > /dev/null <<EOF
[Unit]
Description=Cardano Node (Rust)
After=network.target

[Service]
Type=simple
User=$USER
WorkingDirectory=$HOME/cardano-node
ExecStart=/usr/local/bin/cardano-node run \\
  --config $HOME/cardano-node/config.json \\
  --topology $HOME/cardano-node/topology.json \\
  --database-path $HOME/cardano-node/db \\
  --socket-path $HOME/cardano-node/node.socket
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
EOF

# Enable and start
sudo systemctl daemon-reload
sudo systemctl enable cardano-node
sudo systemctl start cardano-node

# Check status
sudo systemctl status cardano-node

# View logs
sudo journalctl -u cardano-node -f
```

**macOS (launchd):**

```bash
# Create plist file
tee ~/Library/LaunchAgents/io.cardano.node.plist > /dev/null <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>io.cardano.node</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/cardano-node</string>
        <string>run</string>
        <string>--config</string>
        <string>$HOME/cardano-node/config.json</string>
        <string>--topology</string>
        <string>$HOME/cardano-node/topology.json</string>
        <string>--database-path</string>
        <string>$HOME/cardano-node/db</string>
        <string>--socket-path</string>
        <string>$HOME/cardano-node/node.socket</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardErrorPath</key>
    <string>$HOME/cardano-node/error.log</string>
    <key>StandardOutPath</key>
    <string>$HOME/cardano-node/output.log</string>
</dict>
</plist>
EOF

# Load service
launchctl load ~/Library/LaunchAgents/io.cardano.node.plist

# Check status
launchctl list | grep cardano
```

### Docker Quick Start

```bash
# Pull image
docker pull fractionestate/cardano-node-rust:latest

# Run node
docker run -d \
  --name cardano-node \
  --restart unless-stopped \
  -v ~/cardano-data:/data \
  -p 3001:3001 \
  fractionestate/cardano-node-rust:latest \
  init-and-run --network preview

# Check logs
docker logs -f cardano-node

# Query tip
docker exec cardano-node cardano-node query tip --socket-path /data/node.socket
```

---

## 🌐 Network Selection

### Preview Testnet (Recommended for Testing)
```bash
cardano-node init --network preview
# Magic: 2
# Fast sync, frequent updates, free test ADA
```

### Preprod Testnet (Pre-Production)
```bash
cardano-node init --network preprod
# Magic: 1
# More stable than preview, production-like
```

### Mainnet (Production)
```bash
cardano-node init --network mainnet
# Magic: 764824073
# Real ADA, full security, slower sync
```

### Custom Network
```bash
# Download custom configs
wget <custom-config-url> -O config.json
wget <custom-topology-url> -O topology.json
wget <custom-genesis-url> -O genesis.json

# Start node
cardano-node run --config config.json ...
```

---

## 🔍 Monitoring

### Check Node Health

```bash
# Quick status
cardano-node query tip

# Detailed node info
cardano-node node info

# Protocol parameters
cardano-node query protocol-parameters

# Stake distribution
cardano-node query stake-distribution
```

### Monitor Sync Progress

```bash
# Simple progress check
watch -n 10 'cardano-node query tip | jq .syncProgress'

# Detailed monitoring script
cat > check-sync.sh <<'EOF'
#!/bin/bash
while true; do
  TIP=$(cardano-node query tip 2>/dev/null)
  if [ $? -eq 0 ]; then
    SYNC=$(echo $TIP | jq -r .syncProgress)
    BLOCK=$(echo $TIP | jq -r .block)
    EPOCH=$(echo $TIP | jq -r .epoch)
    echo "$(date): Sync: $SYNC | Block: $BLOCK | Epoch: $EPOCH"
  else
    echo "$(date): Node not responding..."
  fi
  sleep 30
done
EOF
chmod +x check-sync.sh
./check-sync.sh
```

---

## 💡 Pro Tips

### Speed Up Initial Sync

1. **Use SSD storage**
   ```bash
   # Check if using SSD
   lsblk -d -o name,rota
   # rota=0 means SSD, rota=1 means HDD
   ```

2. **Increase bulk sync connections**
   ```json
   // In config.json
   "MaxConcurrencyBulkSync": 4  // Default is 2
   ```

3. **Use snapshot (advanced)**
   ```bash
   # Download ledger snapshot (if available)
   wget <snapshot-url> -O db-snapshot.tar.gz
   tar -xzf db-snapshot.tar.gz -C db/
   ```

### Save Disk Space

```bash
# Enable database compression (in config)
"DatabaseCompression": true

# Periodic cleanup
cardano-node admin compact-db --database-path db/
```

### Reduce Memory Usage

```json
// In config.json
"MaxBlockFetch": 32,  // Reduce from default 64
"CacheSize": 4096     // Reduce from default 8192
```

---

## ⚠️ Troubleshooting

### Node Won't Start

**Check configuration:**
```bash
cardano-node validate --config config.json
```

**Check ports:**
```bash
# Check if port 3001 is available
sudo netstat -tulpn | grep 3001

# Use different port if needed
# Edit config.json: "Port": 3002
```

**Check permissions:**
```bash
# Ensure you can write to database directory
ls -la db/
chmod -R u+w db/
```

### Sync Stuck

**Restart node:**
```bash
pkill cardano-node
cardano-node run ...
```

**Check network:**
```bash
# Test connectivity to relays
ping relays-new.cardano-mainnet.iohk.io

# Check DNS
nslookup relays-new.cardano-mainnet.iohk.io
```

**Update topology:**
```bash
# Download latest topology
cardano-node init --network preview --force-update
```

### Out of Disk Space

**Check usage:**
```bash
df -h
du -sh ~/cardano-node/db
```

**Clean up:**
```bash
# Remove old logs
rm -f ~/cardano-node/*.log

# Compact database
cardano-node admin compact-db --database-path ~/cardano-node/db
```

### Socket Connection Failed

**Check socket exists:**
```bash
ls -la node.socket
```

**Use absolute path:**
```bash
export CARDANO_NODE_SOCKET_PATH=$(pwd)/node.socket
```

**Check node is running:**
```bash
ps aux | grep cardano-node
```

---

## 📖 Next Steps

### For Developers
- **[CLI Reference](docs/api/CLI_REFERENCE.md)** - All commands and options
- **[API Documentation](docs/api/API_REFERENCE.md)** - Rust API reference
- **[Transaction Guide](docs/guides/transactions.md)** - Build complex transactions
- **[Smart Contracts](docs/guides/plutus.md)** - Deploy and interact with Plutus scripts

### For Stake Pool Operators
- **[Block Producer Setup](docs/guides/block-producer.md)** - Run a stake pool
- **[Key Management](docs/guides/key-management.md)** - Secure key practices
- **[Monitoring Setup](docs/guides/monitoring.md)** - Production monitoring
- **[High Availability](docs/guides/ha-setup.md)** - HA configurations

### For Node Operators
- **[Performance Tuning](docs/guides/performance.md)** - Optimize node performance
- **[Security Hardening](docs/guides/security.md)** - Secure your node
- **[Backup & Recovery](docs/guides/backup.md)** - Data protection
- **[Upgrading Guide](docs/guides/upgrading.md)** - Safe upgrade procedures

### Migration from Haskell Node
- **[Migration Guide](MIGRATION_GUIDE.md)** - Step-by-step migration
- **[Compatibility Matrix](CARDANO_API_CLI_ALIGNMENT.md)** - API/CLI compatibility
- **[FAQ](docs/FAQ.md)** - Common questions

---

## 🆘 Need Help?

### Documentation
- **Website**: https://cardano-rust-node.io
- **Docs**: https://docs.cardano-rust-node.io
- **API Docs**: https://docs.rs/cardano-node-rust

### Community
- **Discord**: https://discord.gg/cardano-rust-node
- **Forum**: https://forum.cardano.org/c/developers/rust-node
- **Stack Exchange**: https://cardano.stackexchange.com (tag: `rust-node`)

### Support
- **GitHub Issues**: https://github.com/FractionEstate/cardano-rust-node/issues
- **Discussions**: https://github.com/FractionEstate/cardano-rust-node/discussions
- **Email**: support@cardano-rust-node.io

---

## ✅ Quick Reference

### Essential Commands

```bash
# Node operations
cardano-node run                    # Start node
cardano-node query tip              # Check sync status
cardano-node query protocol-parameters  # Get protocol params

# Wallet operations
cardano-node address key-gen        # Generate payment keys
cardano-node stake-address key-gen  # Generate stake keys
cardano-node address build          # Build address
cardano-node query utxo             # Check balance

# Transaction operations
cardano-node transaction build      # Build transaction
cardano-node transaction sign       # Sign transaction
cardano-node transaction submit     # Submit transaction
cardano-node transaction view       # View transaction

# Stake pool operations
cardano-node stake-pool registration-certificate   # Register pool
cardano-node query stake-pools      # List all pools
cardano-node query stake-distribution  # Pool stake distribution
```

### Environment Variables

```bash
# Socket path (so you don't need --socket-path every time)
export CARDANO_NODE_SOCKET_PATH=~/cardano-node/node.socket

# Network magic (for testnet commands)
export CARDANO_TESTNET_MAGIC=2  # Preview
# Or: export CARDANO_TESTNET_MAGIC=1  # Preprod

# Log level
export RUST_LOG=info  # or debug, warn, error
```

---

**Your Cardano Rust node is ready!** 🎉

Start building on Cardano with a fast, efficient, and compatible Rust implementation.
