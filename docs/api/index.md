---
layout: default
title: API Reference
nav_order: 6
has_children: true
permalink: /api/
---

# API & CLI Reference
{: .no_toc }

## Table of contents
{: .no_toc .text-delta }

1. TOC
{:toc}

---

Complete API and command-line interface documentation for Cardano Rust Node.

## CLI Reference

### [Command-Line Interface](CLI_REFERENCE.html)

Comprehensive CLI documentation including:

- **Node Commands** - Start, stop, configure node
- **Query Commands** - Query blockchain state
- **Transaction Commands** - Create and submit transactions
- **Stake Pool Commands** - SPO operations
- **Key Management** - Generate and manage keys
- **Utility Commands** - Various utility functions

Example usage:
```bash
# Start a node
cardano-node run --config config.json --topology topology.json

# Query tip
cardano-cli query tip --testnet-magic 2

# Create transaction
cardano-cli transaction build --tx-in <txid>#<idx> --tx-out <addr>+<amount>
```

[View Full CLI Reference →](CLI_REFERENCE.html)

---

## Rust API

### [API Documentation](API_REFERENCE.html)

Complete Rust API documentation covering:

- **Core Types** - Fundamental data types
- **Transaction API** - Transaction building and signing
- **Ledger API** - Ledger state access
- **Network API** - Network protocol interface
- **Cryptography API** - Cryptographic operations
- **Storage API** - Blockchain storage

Example usage:
```rust
use cardano_node::api::{Node, Config};

// Create and start a node
let config = Config::from_file("config.json")?;
let node = Node::new(config)?;
node.run().await?;
```

[View Full API Documentation →](API_REFERENCE.html)

---

## Quick Reference

### [Command Cheat Sheet](../reference/QUICK_REFERENCE.html)

Quick reference for common commands and operations:
- Common node operations
- Frequently used queries
- Transaction shortcuts
- Key management commands

---

## Related Documentation

- [Getting Started Guide](../guides/GETTING_STARTED.html) - Setup and configuration
- [Architecture](../architecture/ARCHITECTURE.html) - System design
- [Operations Guide](../operations/MONITORING_AND_METRICS.html) - Production operations
- [FAQ](../FAQ.html) - Common questions

---

## API Compatibility

{: .note }
> **Haskell Node Compatibility**: Our API is designed to be compatible with the official Haskell node. See [Compatibility Report](../architecture/HASKELL_COMPATIBILITY_VERIFIED.html) for details.

### Supported Networks

- ✅ **Mainnet** - Full support
- ✅ **Preview Testnet** - Full support
- ✅ **Preprod Testnet** - Full support
- ✅ **Local Development** - Full support

---

## Examples

Check out our [examples directory](https://github.com/FractionEstate/cardano-rust-node/tree/main/examples) for:
- Node setup examples
- Transaction examples
- Stake pool examples
- Integration examples

---

## Getting Help

- 📖 [Documentation](../) - Full documentation
- 💬 [Discussions](https://github.com/FractionEstate/cardano-rust-node/discussions) - Ask questions
- 🐛 [Issues](https://github.com/FractionEstate/cardano-rust-node/issues) - Report bugs
