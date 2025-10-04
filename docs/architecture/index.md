---
layout: default
title: Architecture
nav_order: 5
has_children: true
permalink: /architecture/
---

# Architecture & Design
{: .no_toc }

## Table of contents
{: .no_toc .text-delta }

1. TOC
{:toc}

---

System design, protocol specifications, and technical architecture documentation.

## Core Architecture

### [Architecture Overview](ARCHITECTURE.html)
Complete system design and architecture documentation covering:
- System components
- Module interactions
- Data flow
- Design patterns

### [Protocol Architecture](PROTOCOL_ARCHITECTURE.html)
Protocol-level design specifications:
- Cardano protocol implementation
- Network layer design
- Consensus mechanisms
- State management

### [Protocol Design](PROTOCOL_DESIGN.html)
Detailed protocol specifications and design decisions.

### [Ledger State Alignment](LEDGER_STATE_ALIGNMENT.html)
Ledger implementation details and state management.

---

## Compatibility & Integration

### [Haskell Compatibility Verified](HASKELL_COMPATIBILITY_VERIFIED.html)
Complete compatibility verification with the official Haskell node:
- Feature parity analysis
- Test results
- Compatibility matrix

### [Haskell Compatibility Gaps](HASKELL_COMPATIBILITY_GAPS.html)
Known differences and limitations:
- Feature differences
- API variations
- Migration considerations

### [Alignment with Official Node](ALIGNMENT_WITH_OFFICIAL.html)
Feature alignment status with the official Cardano node.

---

## Protocol Implementation

### [ChainSync Integration](CHAINSYNC_INTEGRATION.html)
ChainSync protocol implementation details:
- Protocol overview
- Implementation approach
- Performance optimizations

### [Cardano Base Rust Migration](CARDANO_BASE_RUST_MIGRATION.html)
Migration from Haskell to Rust documentation:
- Migration strategy
- Progress tracking
- Challenges and solutions

---

## Technical Details

### [Curve25519 Dalek Explanation](CURVE25519_DALEK_EXPLANATION.html)
Cryptographic library usage and implementation details.

### [Distribution Strategy](distribution.html)
Software distribution and packaging approach.

### [Sync Roadmap](sync-roadmap.html)
Blockchain synchronization strategy and roadmap.

---

## Quick Links

- [API Reference](../api/API_REFERENCE.html) - Rust API documentation
- [CLI Reference](../api/CLI_REFERENCE.html) - Command-line interface
- [User Guides](../guides/) - Practical tutorials
- [Reports](../reports/) - Status and audit reports
