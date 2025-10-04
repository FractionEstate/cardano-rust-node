---
layout: default
title: Quick Start
nav_order: 3
description: "Get started with Cardano Rust Node in 10 minutes"
permalink: /quickstart/
---

# Quick Start Guide
{: .fs-9 }

Get your Cardano Rust Node up and running in 10 minutes.
{: .fs-6 .fw-300 }

---

{: .note }
This guide assumes you have Docker or Rust installed. See the [Installation Guide]({% link INSTALLATION_GUIDE.md %}) for detailed setup instructions.

## Prerequisites

- **Docker** (recommended) OR **Rust 1.70+** and **Cargo**
- **4 GB RAM** minimum (8 GB recommended)
- **50 GB disk space** for blockchain data
- **Linux, macOS, or Windows** with WSL2

---

## Method 1: Using Docker (Recommended)

The fastest way to get started:

```bash
# Pull the latest image
docker pull fractionestate/cardano-rust-node:latest

# Create directories for node data
mkdir -p ~/cardano-node/{db,config}

# Download configuration files
cd ~/cardano-node/config
wget https://book.world.dev.cardano.org/environments/preview/config.json
wget https://book.world.dev.cardano.org/environments/preview/topology.json

# Run the node
docker run -d \
  --name cardano-node \
  -p 3001:3001 \
  -v ~/cardano-node/db:/data \
  -v ~/cardano-node/config:/config \
  fractionestate/cardano-rust-node:latest \
  run \
  --config /config/config.json \
  --topology /config/topology.json \
  --database-path /data \
  --socket-path /data/node.socket
```

---

## Method 2: Using Cargo

Build and run from source:

```bash
# Clone the repository
git clone https://github.com/FractionEstate/cardano-rust-node.git
cd cardano-rust-node

# Build with optimizations
cargo build --release

# Run the node
./target/release/cardano-node run \
  --config config/preview-config.json \
  --topology config/preview-topology.json \
  --database-path ./db \
  --socket-path ./node.socket
```

---

## Verify Installation

Check that your node is running:

```bash
# Check node status
cardano-cli query tip --testnet-magic 2

# View logs (Docker)
docker logs -f cardano-node

# View sync progress
cardano-cli query tip --testnet-magic 2 | jq .syncProgress
```

---

## Next Steps

### 📚 Learn More

- [Installation Guide]({% link INSTALLATION_GUIDE.md %}) - Detailed installation methods
- [Getting Started]({% link guides/GETTING_STARTED.md %}) - Complete setup guide
- [Configuration]({% link guides/GETTING_STARTED.md %}#configuration) - Configure your node

### 🔧 Operations

- [Monitoring]({% link operations/MONITORING_AND_METRICS.md %}) - Set up monitoring
- [CLI Reference]({% link api/CLI_REFERENCE.md %}) - All available commands
- [FAQ]({% link FAQ.md %}) - Common questions

### 🏊 For Stake Pool Operators

- [SPO Setup Guide]({% link guides/GETTING_STARTED.md %}#stake-pool-setup) - Run a stake pool
- [Block Production]({% link reports/MISSION_ACCOMPLISHED.md %}) - Block forging
- [Dashboard]({% link guides/DASHBOARD_VISUAL_GUIDE.md %}) - Monitoring dashboard

---

## Getting Help

{: .important }
If you encounter issues, check the [FAQ]({% link FAQ.md %}) or open an issue on [GitHub](https://github.com/FractionEstate/cardano-rust-node/issues).

---

[View Full Documentation]({% link README.md %}){: .btn .btn-primary }
[API Reference]({% link api/CLI_REFERENCE.md %}){: .btn }
