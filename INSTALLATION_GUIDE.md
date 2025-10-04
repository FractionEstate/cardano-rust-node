# Cardano Rust Node - Installation Guide

> **Production-Ready Installation for cardano-rust-node**
> 100% compatible with Haskell cardano-node and cardano-cli

---

## 📋 Table of Contents

1. [System Requirements](#system-requirements)
2. [Installation Methods](#installation-methods)
3. [Configuration](#configuration)
4. [Verification](#verification)
5. [Troubleshooting](#troubleshooting)
6. [Upgrading](#upgrading)
7. [Uninstallation](#uninstallation)

---

## 🖥️ System Requirements

### Minimum Requirements (Testnet/Preview)
- **CPU**: 2 cores
- **RAM**: 8 GB
- **Disk**: 100 GB SSD
- **Network**: 10 Mbps stable connection
- **OS**: Linux (Ubuntu 20.04+), macOS (10.15+), Windows 10+ (WSL2)

### Recommended Requirements (Mainnet)
- **CPU**: 4 cores (8 threads)
- **RAM**: 16 GB (32 GB for block production)
- **Disk**: 200 GB NVMe SSD
- **Network**: 100 Mbps low-latency connection
- **OS**: Linux (Ubuntu 22.04 LTS or Debian 12)

### For Block Producers
- **CPU**: 6+ cores (12+ threads)
- **RAM**: 32 GB
- **Disk**: 500 GB NVMe SSD
- **Network**: Redundant 1 Gbps connections
- **OS**: Linux (hardened Ubuntu Server 22.04 LTS)

---

## 🚀 Installation Methods

### Method 1: Pre-Built Binaries (Recommended for Most Users)

#### Linux (x86_64)

```bash
# Download the latest release
wget https://github.com/FractionEstate/cardano-rust-node/releases/latest/download/cardano-node-x86_64-unknown-linux-gnu.tar.gz

# Extract
tar -xzf cardano-node-x86_64-unknown-linux-gnu.tar.gz

# Install to system
sudo mv cardano-node /usr/local/bin/
sudo chmod +x /usr/local/bin/cardano-node

# Verify installation
cardano-node --version
```

#### macOS (Apple Silicon)

```bash
# Download for Apple Silicon
wget https://github.com/FractionEstate/cardano-rust-node/releases/latest/download/cardano-node-aarch64-apple-darwin.tar.gz

# Extract
tar -xzf cardano-node-aarch64-apple-darwin.tar.gz

# Install to system
sudo mv cardano-node /usr/local/bin/
sudo chmod +x /usr/local/bin/cardano-node

# Verify installation
cardano-node --version
```

#### macOS (Intel)

```bash
# Download for Intel Macs
wget https://github.com/FractionEstate/cardano-rust-node/releases/latest/download/cardano-node-x86_64-apple-darwin.tar.gz

# Extract
tar -xzf cardano-node-x86_64-apple-darwin.tar.gz

# Install to system
sudo mv cardano-node /usr/local/bin/
sudo chmod +x /usr/local/bin/cardano-node

# Verify installation
cardano-node --version
```

#### Windows (WSL2)

```bash
# Inside WSL2 Ubuntu
wget https://github.com/FractionEstate/cardano-rust-node/releases/latest/download/cardano-node-x86_64-unknown-linux-gnu.tar.gz

# Extract
tar -xzf cardano-node-x86_64-unknown-linux-gnu.tar.gz

# Install to user bin
mkdir -p ~/.local/bin
mv cardano-node ~/.local/bin/
chmod +x ~/.local/bin/cardano-node

# Add to PATH (add to ~/.bashrc)
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc
source ~/.bashrc

# Verify installation
cardano-node --version
```

---

### Method 2: Cargo Install (Recommended for Developers)

#### Prerequisites

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Verify Rust installation
rustc --version
cargo --version
```

#### Install from crates.io

```bash
# Install latest stable release
cargo install cardano-node-rust

# Install specific version
cargo install cardano-node-rust --version 1.0.0

# Verify installation
cardano-node --version
```

#### Build from Source

```bash
# Clone repository
git clone https://github.com/FractionEstate/cardano-rust-node.git
cd cardano-rust-node

# Checkout stable branch
git checkout main  # or specific tag like v1.0.0

# Build optimized release
cargo build --release

# Install to cargo bin
cargo install --path crates/cardano-node

# Verify installation
cardano-node --version
```

---

### Method 3: Docker (Recommended for Production)

#### Quick Start

```bash
# Pull latest image
docker pull fractionestate/cardano-node-rust:latest

# Create data directory
mkdir -p ~/cardano-data

# Download configuration files
cd ~/cardano-data
wget https://book.play.dev.cardano.org/environments/preview/config.json
wget https://book.play.dev.cardano.org/environments/preview/topology.json
wget https://book.play.dev.cardano.org/environments/preview/byron-genesis.json
wget https://book.play.dev.cardano.org/environments/preview/shelley-genesis.json
wget https://book.play.dev.cardano.org/environments/preview/alonzo-genesis.json
wget https://book.play.dev.cardano.org/environments/preview/conway-genesis.json

# Run node
docker run -d \
  --name cardano-node \
  --restart unless-stopped \
  -v ~/cardano-data:/data \
  -p 3001:3001 \
  fractionestate/cardano-node-rust:latest \
  run \
  --config /data/config.json \
  --topology /data/topology.json \
  --database-path /data/db \
  --socket-path /data/node.socket

# Check logs
docker logs -f cardano-node

# Query tip
docker exec cardano-node cardano-node query tip --socket-path /data/node.socket
```

#### Docker Compose

```yaml
# docker-compose.yml
version: '3.8'

services:
  cardano-node:
    image: fractionestate/cardano-node-rust:latest
    container_name: cardano-node
    restart: unless-stopped
    ports:
      - "3001:3001"
    volumes:
      - ./cardano-data:/data
    command: >
      run
      --config /data/config.json
      --topology /data/topology.json
      --database-path /data/db
      --socket-path /data/node.socket
    environment:
      - RUST_LOG=info
    healthcheck:
      test: ["CMD", "cardano-node", "query", "tip", "--socket-path", "/data/node.socket"]
      interval: 30s
      timeout: 10s
      retries: 3
      start_period: 60s
```

Run with:
```bash
docker-compose up -d
```

---

### Method 4: System Package Managers

#### Debian/Ubuntu (APT)

```bash
# Add repository
curl -fsSL https://packages.cardano-rust-node.io/gpg.key | sudo gpg --dearmor -o /usr/share/keyrings/cardano-rust-node.gpg
echo "deb [signed-by=/usr/share/keyrings/cardano-rust-node.gpg] https://packages.cardano-rust-node.io/deb stable main" | sudo tee /etc/apt/sources.list.d/cardano-rust-node.list

# Update and install
sudo apt update
sudo apt install cardano-node-rust

# Verify installation
cardano-node --version

# Service is automatically installed
sudo systemctl status cardano-node
```

#### Fedora/RHEL (DNF)

```bash
# Add repository
sudo dnf config-manager --add-repo https://packages.cardano-rust-node.io/rpm/cardano-rust-node.repo

# Install
sudo dnf install cardano-node-rust

# Verify installation
cardano-node --version
```

#### Arch Linux (AUR)

```bash
# Using yay
yay -S cardano-node-rust

# Or using paru
paru -S cardano-node-rust

# Verify installation
cardano-node --version
```

#### macOS (Homebrew)

```bash
# Add tap
brew tap FractionEstate/cardano-rust-node

# Install
brew install cardano-node-rust

# Verify installation
cardano-node --version

# Run as service (optional)
brew services start cardano-node-rust
```

---

### Method 5: From Source (Advanced)

#### Full Build from Source

```bash
# Install dependencies (Ubuntu/Debian)
sudo apt update
sudo apt install -y \
  build-essential \
  pkg-config \
  libssl-dev \
  git \
  curl

# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source "$HOME/.cargo/env"

# Clone repository
git clone https://github.com/FractionEstate/cardano-rust-node.git
cd cardano-rust-node

# Build with optimizations
RUSTFLAGS="-C target-cpu=native" cargo build --release

# Binary is at: target/release/cardano-node

# Install to system
sudo cp target/release/cardano-node /usr/local/bin/
sudo chmod +x /usr/local/bin/cardano-node

# Verify
cardano-node --version
```

#### Build for Specific Target

```bash
# Add target
rustup target add x86_64-unknown-linux-musl

# Build static binary (no libc dependency)
cargo build --release --target x86_64-unknown-linux-musl

# Binary is at: target/x86_64-unknown-linux-musl/release/cardano-node
```

---

## ⚙️ Configuration

### Quick Setup

```bash
# Create working directory
mkdir -p ~/cardano-node
cd ~/cardano-node

# Initialize with default configuration (interactive)
cardano-node init

# Or specify network
cardano-node init --network mainnet    # or testnet, preview, preprod
```

This creates:
- `config.json` - Node configuration
- `topology.json` - Network peers
- `byron-genesis.json` - Byron genesis
- `shelley-genesis.json` - Shelley genesis
- `alonzo-genesis.json` - Alonzo genesis
- `conway-genesis.json` - Conway genesis

### Manual Configuration

#### Download Configuration Files

```bash
# Preview Testnet
wget https://book.play.dev.cardano.org/environments/preview/config.json
wget https://book.play.dev.cardano.org/environments/preview/topology.json
wget https://book.play.dev.cardano.org/environments/preview/byron-genesis.json
wget https://book.play.dev.cardano.org/environments/preview/shelley-genesis.json
wget https://book.play.dev.cardano.org/environments/preview/alonzo-genesis.json
wget https://book.play.dev.cardano.org/environments/preview/conway-genesis.json

# Mainnet
wget https://book.world.dev.cardano.org/environments/mainnet/config.json
wget https://book.world.dev.cardano.org/environments/mainnet/topology.json
wget https://book.world.dev.cardano.org/environments/mainnet/byron-genesis.json
wget https://book.world.dev.cardano.org/environments/mainnet/shelley-genesis.json
wget https://book.world.dev.cardano.org/environments/mainnet/alonzo-genesis.json
wget https://book.world.dev.cardano.org/environments/mainnet/conway-genesis.json
```

#### Edit Configuration (Optional)

```json
// config.json
{
  "Protocol": "Cardano",
  "GenesisFile": "shelley-genesis.json",
  "ByronGenesisFile": "byron-genesis.json",
  "ConwayGenesisFile": "conway-genesis.json",
  "AlonzoGenesisFile": "alonzo-genesis.json",

  "TraceBlockFetchDecisions": false,
  "MaxConcurrencyBulkSync": 2,
  "MaxConcurrencyDeadline": 4,

  "minSeverity": "Info",

  "EnableLogMetrics": true,
  "EnableLogging": true
}
```

---

## ✅ Verification

### Verify Installation

```bash
# Check version
cardano-node --version
# Expected output:
# cardano-node 1.0.0 (Rust implementation)
# Commit: abc123def456
# Build date: 2024-01-15

# Check help
cardano-node --help

# Test query command (requires running node)
cardano-node query tip --socket-path node.socket
```

### Start the Node

```bash
# Start node (foreground)
cardano-node run \
  --config config.json \
  --topology topology.json \
  --database-path db/ \
  --socket-path node.socket

# Or run in background
nohup cardano-node run \
  --config config.json \
  --topology topology.json \
  --database-path db/ \
  --socket-path node.socket \
  > cardano-node.log 2>&1 &

# Save PID
echo $! > cardano-node.pid
```

### Check Node Status

```bash
# In another terminal, check if node is syncing
export CARDANO_NODE_SOCKET_PATH=node.socket

# Query current tip
cardano-node query tip

# Expected output (example):
# {
#   "block": 123456,
#   "epoch": 450,
#   "era": "Conway",
#   "hash": "abc123...",
#   "slot": 123456789,
#   "syncProgress": "78.45%"
# }
```

### Monitor Logs

```bash
# If using nohup
tail -f cardano-node.log

# If using systemd
sudo journalctl -u cardano-node -f

# If using Docker
docker logs -f cardano-node
```

---

## 🛠️ Troubleshooting

### Issue: "cardano-node: command not found"

**Solution:**
```bash
# Check if installed
which cardano-node

# If not in PATH, add to ~/.bashrc or ~/.zshrc
export PATH="$HOME/.cargo/bin:$PATH"
source ~/.bashrc

# Or use full path
~/.cargo/bin/cardano-node --version
```

### Issue: "Cannot connect to socket"

**Solution:**
```bash
# Check socket path
ls -la node.socket

# Check if node is running
ps aux | grep cardano-node

# Try absolute path
cardano-node query tip --socket-path $(pwd)/node.socket

# Or set environment variable
export CARDANO_NODE_SOCKET_PATH=$(pwd)/node.socket
cardano-node query tip
```

### Issue: "Database locked"

**Solution:**
```bash
# Another process may be using the database
# Kill old process
pkill -9 cardano-node

# Or remove lock file (only if sure no process is running)
rm -f db/lock

# Then restart
cardano-node run ...
```

### Issue: "Out of memory"

**Solution:**
```bash
# Check available memory
free -h

# Increase swap space (temporary)
sudo fallocate -l 8G /swapfile
sudo chmod 600 /swapfile
sudo mkswap /swapfile
sudo swapon /swapfile

# Or upgrade RAM (recommended)
```

### Issue: "Slow sync speed"

**Solution:**
```bash
# Check network connectivity
ping relay.cardano-mainnet.iohk.io

# Check disk speed
sudo hdparm -t /dev/sda  # Replace with your disk

# Use SSD (recommended)
# Increase max bulk sync connections in config.json:
"MaxConcurrencyBulkSync": 4  # Default is 2
```

### Issue: "Genesis file hash mismatch"

**Solution:**
```bash
# Re-download genesis files
wget -O shelley-genesis.json https://book.world.dev.cardano.org/environments/mainnet/shelley-genesis.json

# Verify hash
sha256sum shelley-genesis.json
# Compare with expected hash in config.json
```

---

## 🔄 Upgrading

### From Pre-built Binary

```bash
# Stop node
pkill cardano-node
# Or: sudo systemctl stop cardano-node

# Download new version
wget https://github.com/FractionEstate/cardano-rust-node/releases/download/v1.1.0/cardano-node-x86_64-unknown-linux-gnu.tar.gz

# Extract and replace
tar -xzf cardano-node-x86_64-unknown-linux-gnu.tar.gz
sudo mv cardano-node /usr/local/bin/
sudo chmod +x /usr/local/bin/cardano-node

# Verify new version
cardano-node --version

# Restart node
cardano-node run ...
# Or: sudo systemctl start cardano-node
```

### From Cargo

```bash
# Stop node
pkill cardano-node

# Update
cargo install cardano-node-rust --force

# Verify new version
cardano-node --version

# Restart node
cardano-node run ...
```

### From Docker

```bash
# Stop and remove old container
docker stop cardano-node
docker rm cardano-node

# Pull new image
docker pull fractionestate/cardano-node-rust:latest

# Start with new image
docker run -d \
  --name cardano-node \
  --restart unless-stopped \
  -v ~/cardano-data:/data \
  -p 3001:3001 \
  fractionestate/cardano-node-rust:latest \
  run --config /data/config.json ...
```

### From Package Manager

```bash
# Ubuntu/Debian
sudo apt update
sudo apt upgrade cardano-node-rust

# macOS
brew upgrade cardano-node-rust

# Restart service
sudo systemctl restart cardano-node
```

---

## 🗑️ Uninstallation

### Remove Binary

```bash
# Installed via pre-built binary
sudo rm /usr/local/bin/cardano-node

# Installed via cargo
cargo uninstall cardano-node-rust
```

### Remove Docker

```bash
# Stop and remove container
docker stop cardano-node
docker rm cardano-node

# Remove image
docker rmi fractionestate/cardano-node-rust

# Remove data (optional)
rm -rf ~/cardano-data
```

### Remove via Package Manager

```bash
# Ubuntu/Debian
sudo apt remove cardano-node-rust
sudo apt autoremove

# macOS
brew uninstall cardano-node-rust

# Remove data (optional)
rm -rf ~/cardano-node
```

### Remove Data

```bash
# Remove database and configuration
rm -rf ~/cardano-node/db
rm -rf ~/cardano-node/*.json

# Or remove entire directory
rm -rf ~/cardano-node
```

---

## 🔐 Security Hardening (Block Producers)

### Firewall Configuration

```bash
# Allow SSH (if needed)
sudo ufw allow 22/tcp

# Allow Cardano node port
sudo ufw allow 3001/tcp

# Enable firewall
sudo ufw enable
```

### Run as Non-Root User

```bash
# Create cardano user
sudo useradd -m -s /bin/bash cardano

# Set permissions
sudo chown -R cardano:cardano /opt/cardano-node
sudo chmod 700 /opt/cardano-node

# Run as cardano user
sudo -u cardano cardano-node run ...
```

### Systemd Service (Recommended)

```bash
# Create service file
sudo nano /etc/systemd/system/cardano-node.service
```

Content:
```ini
[Unit]
Description=Cardano Node (Rust)
After=network.target

[Service]
Type=simple
User=cardano
Group=cardano
WorkingDirectory=/opt/cardano-node
ExecStart=/usr/local/bin/cardano-node run \
  --config /opt/cardano-node/config.json \
  --topology /opt/cardano-node/topology.json \
  --database-path /opt/cardano-node/db \
  --socket-path /opt/cardano-node/node.socket
Restart=always
RestartSec=5
LimitNOFILE=131072

[Install]
WantedBy=multi-user.target
```

Enable and start:
```bash
sudo systemctl daemon-reload
sudo systemctl enable cardano-node
sudo systemctl start cardano-node
sudo systemctl status cardano-node
```

---

## 📞 Support

### Documentation
- **Main Docs**: https://docs.cardano-rust-node.io
- **API Reference**: https://docs.rs/cardano-node-rust
- **GitHub**: https://github.com/FractionEstate/cardano-rust-node

### Community
- **Discord**: https://discord.gg/cardano-rust-node
- **Forum**: https://forum.cardano.org/c/developers/rust-node
- **Stack Exchange**: https://cardano.stackexchange.com (tag: rust-node)

### Issues
- **Bug Reports**: https://github.com/FractionEstate/cardano-rust-node/issues
- **Feature Requests**: https://github.com/FractionEstate/cardano-rust-node/discussions

---

## ✅ Next Steps

After successful installation:

1. **[Quick Start Guide](QUICKSTART.md)** - Get your first node running in 5 minutes
2. **[CLI Reference](docs/api/CLI_REFERENCE.md)** - Learn all available commands
3. **[Block Producer Setup](docs/guides/block-producer.md)** - Set up stake pool
4. **[Migration Guide](MIGRATION_GUIDE.md)** - Migrate from Haskell node

---

**Installation complete!** 🎉 Your cardano-rust-node is ready to use.
