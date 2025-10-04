# Ouroboros Protocol Implementation Design

## Overview

This document describes the Rust implementation of Ouroboros mini-protocols for Cardano node-to-node (N2N) communication. The design is based on the official Haskell ouroboros-network implementation.

## Architecture

### Protocol Stack

```
┌────────────────────────────────────────┐
│     Application Layer (Sync Logic)    │
├────────────────────────────────────────┤
│         Mini-Protocols Layer           │
│  ┌──────────┬──────────┬──────────┐   │
│  │Handshake │ChainSync │BlockFetch│   │
│  │  (N2N)   │ Protocol │ Protocol │   │
│  └──────────┴──────────┴──────────┘   │
├────────────────────────────────────────┤
│       Multiplexer (Mux/Demux)          │
│    Protocol ID-based Routing           │
├────────────────────────────────────────┤
│         Transport Layer (TCP)          │
└────────────────────────────────────────┘
```

### Key Components

1. **Mini-Protocol Framework**: Trait-based abstraction for protocol state machines
2. **Message Codec**: CBOR encoding/decoding using `minicbor` crate
3. **Multiplexer**: Protocol ID-based message routing over single TCP connection
4. **State Machine**: Async state transitions with tokio

## Protocol Specifications

### 1. Handshake Protocol (Node-to-Node)

**Purpose**: Negotiate protocol version and network parameters before other protocols can run.

#### State Machine

```
StPropose ─MsgProposeVersions─→ StConfirm
                                    │
                 ┌──────────────────┼────────────────┐
                 │                  │                │
        MsgAcceptVersion    MsgReplyVersions   MsgRefuse
                 │                  │                │
                 ↓                  ↓                ↓
              StDone             StDone           StDone
```

#### Message Types

```rust
enum HandshakeMessage {
    MsgProposeVersions {
        versions: Map<NodeToNodeVersion, VersionData>
    },
    MsgReplyVersions {
        versions: Map<NodeToNodeVersion, VersionData>
    },
    MsgAcceptVersion {
        version: NodeToNodeVersion,
        version_data: VersionData
    },
    MsgRefuse {
        reason: RefuseReason
    },
    MsgQueryReply {
        versions: Map<NodeToNodeVersion, VersionData>
    }
}
```

#### Version Data (NodeToNodeVersionData)

```rust
struct NodeToNodeVersionData {
    network_magic: NetworkMagic,        // u32: 764824073 (mainnet) | 1097911063 (preview)
    diffusion_mode: DiffusionMode,      // InitiatorAndResponder | InitiatorOnly
    peer_sharing: PeerSharing,          // Enabled | Disabled
    query: bool                         // true for query mode
}
```

#### Supported Versions

- **NodeToNodeV_14**: Introduced for Conway era
- **NodeToNodeV_15**: Latest (SRV support), current requirement for preview testnet

#### CBOR Encoding

**Version Number**: `TInt(14)` or `TInt(15)`

**Version Data**: `TList [TInt(network_magic), TBool(diffusion), TInt(peer_sharing), TBool(query)]`

**Message Format**:
```
MsgProposeVersions:
  [0, {version_number: version_data, ...}]  # List len 2, tag 0, map of versions

MsgAcceptVersion:
  [1, version_number, version_data]  # List len 3, tag 1

MsgRefuse:
  [2, refuse_reason]  # List len 2, tag 2
```

#### Refuse Reasons

```rust
enum RefuseReason {
    VersionMismatch {
        proposed: Vec<NodeToNodeVersion>,
        unknown_tags: Vec<i64>
    },
    HandshakeDecodeError {
        version: NodeToNodeVersion,
        error: String
    },
    Refused {
        version: NodeToNodeVersion,
        reason: String
    }
}
```

#### Implementation Strategy

1. **Send `MsgProposeVersions`** with supported versions (V_14, V_15)
2. **Receive response**: `MsgAcceptVersion` or `MsgRefuse`
3. **Validate network magic**: Must match testnet (1097911063)
4. **Store negotiated version** for subsequent protocol use
5. **Transition to StDone** and enable mini-protocols

#### Error Handling

- **Early EOF**: Protocol mismatch or incomplete handshake
- **Version Mismatch**: No common version between client/server
- **Network Magic Mismatch**: Different network (mainnet vs testnet)
- **Decode Error**: Invalid CBOR or unsupported encoding

### 2. ChainSync Protocol

**Purpose**: Synchronize blockchain headers and handle chain forks.

#### State Machine

```
StIdle ─MsgRequestNext─→ StNext(CanAwait)
  ↑                          │
  │         ┌────────────────┼────────────┐
  │         │                │            │
  │   MsgAwaitReply   MsgRollForward  MsgRollBackward
  │         │                │            │
  │         ↓                ↓            ↓
  └─── StNext(MustReply) ─→ StIdle ←─ StIdle

StIdle ─MsgFindIntersect─→ StIntersect
  ↑                            │
  │         ┌──────────────────┼──────────────┐
  │         │                  │              │
  │  MsgIntersectFound  MsgIntersectNotFound │
  │         │                  │              │
  │         ↓                  ↓              ↓
  └────── StIdle ←─────────── StIdle ←───── StIdle
```

#### Message Types

```rust
enum ChainSyncMessage {
    // Client → Server
    MsgRequestNext,
    MsgFindIntersect { points: Vec<Point> },
    MsgDone,

    // Server → Client
    MsgAwaitReply,
    MsgRollForward { header: BlockHeader, tip: Tip },
    MsgRollBackward { point: Point, tip: Tip },
    MsgIntersectFound { point: Point, tip: Tip },
    MsgIntersectNotFound { tip: Tip }
}
```

#### Data Types

```rust
struct Point {
    slot: SlotNo,      // u64
    hash: Blake2b256Hash  // 32 bytes
}

struct Tip {
    slot: SlotNo,
    hash: Blake2b256Hash,
    height: BlockNo    // u64
}

struct BlockHeader {
    slot: SlotNo,
    prev_hash: Blake2b256Hash,
    issuer_vkey: Ed25519KeyHash,
    vrf_proof: VrfProof,
    vrf_output: VrfOutput,
    block_body_hash: Blake2b256Hash,
    block_size: u32,
    operational_cert: OperationalCertificate,
    protocol_magic: u32
}
```

#### CBOR Encoding

**Point**: `[slot: uint, hash: bytes]`
**Tip**: `[slot: uint, hash: bytes, height: uint]`
**Header**: `[slot, prev_hash, issuer_vkey, vrf_proof, vrf_output, body_hash, size, op_cert, magic]`

**Messages**:
```
MsgRequestNext:         [0]
MsgAwaitReply:          [1]
MsgRollForward:         [2, header, tip]
MsgRollBackward:        [3, point, tip]
MsgFindIntersect:       [4, [points...]]
MsgIntersectFound:      [5, point, tip]
MsgIntersectNotFound:   [6, tip]
MsgDone:                [7]
```

#### Protocol Flow

1. **Find Intersection**:
   - Client sends `MsgFindIntersect` with known points (newest → oldest)
   - Server responds with `MsgIntersectFound(point, tip)` or `MsgIntersectNotFound(tip)`
   - Establishes common chain ancestor

2. **Request Next Block**:
   - Client sends `MsgRequestNext`
   - Server may send `MsgAwaitReply` if no blocks ready
   - Server sends `MsgRollForward(header, tip)` with next block
   - OR `MsgRollBackward(point, tip)` if chain reorganization

3. **Handle Rollback**:
   - Client reverts to specified point
   - Discards blocks after rollback point
   - Resumes with `MsgRequestNext`

#### Implementation Strategy

1. **Start at Genesis**: Begin sync from slot 0
2. **Find Intersection**: Send known points to establish common ancestor
3. **Pipeline Requests**: Send multiple `MsgRequestNext` without waiting
4. **Validate Headers**: Check VRF proofs, operational certificates, signatures
5. **Track Chain Tip**: Monitor tip updates to know sync progress
6. **Handle Forks**: Process rollbacks and apply forward from new point

### 3. BlockFetch Protocol

**Purpose**: Download block bodies after headers are validated.

#### State Machine

```
StIdle ─MsgRequestRange─→ StBusy
  ↑                          │
  │         ┌────────────────┼────────────┐
  │         │                │            │
  │   MsgStartBatch    MsgNoBlocks    MsgBatchDone
  │         │                │            │
  │         ↓                ↓            ↓
  └─── StStreaming ─────→ StIdle ←───── StIdle
           │
    MsgBlock (repeated)
```

#### Message Types

```rust
enum BlockFetchMessage {
    MsgRequestRange { from: Point, to: Point },
    MsgStartBatch,
    MsgNoBlocks,
    MsgBlock { block: Block },
    MsgBatchDone,
    MsgClientDone
}
```

#### Protocol Flow

1. Client requests range of blocks
2. Server responds with batch start
3. Server streams block bodies one by one
4. Server signals batch complete
5. Client can request next range

## Multiplexer Design

### Protocol IDs

```rust
const HANDSHAKE_PROTOCOL_ID: u16 = 0;
const CHAIN_SYNC_PROTOCOL_ID: u16 = 2;
const BLOCK_FETCH_PROTOCOL_ID: u16 = 3;
const TX_SUBMISSION_PROTOCOL_ID: u16 = 4;
const KEEP_ALIVE_PROTOCOL_ID: u16 = 8;
const PEER_SHARING_PROTOCOL_ID: u16 = 11;
```

### Message Framing

Each multiplexed message consists of:
```
┌──────────┬──────────┬──────────┬─────────────┐
│  Header  │  Proto   │  Length  │   Payload   │
│ (2 byte) │  ID      │ (2 byte) │  (variable) │
│  0x0000  │ (2 byte) │          │             │
└──────────┴──────────┴──────────┴─────────────┘
```

**Header**: Transmission time (unused in basic mode)
**Protocol ID**: Identifies which mini-protocol
**Length**: Payload size in bytes
**Payload**: CBOR-encoded message

### Routing

```rust
impl Multiplexer {
    async fn route_message(&self, proto_id: u16, payload: Bytes) {
        match proto_id {
            HANDSHAKE_PROTOCOL_ID => self.handshake_handler.handle(payload).await,
            CHAIN_SYNC_PROTOCOL_ID => self.chainsync_handler.handle(payload).await,
            BLOCK_FETCH_PROTOCOL_ID => self.blockfetch_handler.handle(payload).await,
            _ => warn!("Unknown protocol ID: {}", proto_id),
        }
    }
}
```

## Implementation Phases

### Phase 1: Handshake Protocol (CURRENT)
- [x] Define `NodeToNodeVersion` enum (V_14, V_15)
- [x] Define `NodeToNodeVersionData` struct
- [ ] Implement CBOR codec for versions and version data
- [ ] Implement state machine (StPropose → StConfirm → StDone)
- [ ] Implement message handlers
- [ ] Network magic validation
- [ ] Version negotiation algorithm
- [ ] Integration with connection manager

### Phase 2: ChainSync Protocol
- [ ] Define message types and state machine
- [ ] Implement CBOR codec for headers, points, tips
- [ ] Implement client state machine
- [ ] Implement intersection finding
- [ ] Implement header validation
- [ ] Handle rollbacks
- [ ] Track chain tip

### Phase 3: BlockFetch Protocol
- [ ] Define message types and state machine
- [ ] Implement block downloading
- [ ] Implement range requests
- [ ] Integrate with storage

### Phase 4: Testing & Integration
- [ ] Unit tests for each protocol
- [ ] Integration tests with preview testnet
- [ ] End-to-end blockchain sync test
- [ ] Performance benchmarks

## Code Structure

```
crates/cardano-network/src/protocols/
├── mod.rs                  # Protocol trait definitions
├── handshake/
│   ├── mod.rs              # Public API
│   ├── types.rs            # Version, VersionData, Messages
│   ├── codec.rs            # CBOR encoding/decoding
│   ├── state.rs            # State machine
│   └── client.rs           # Client implementation
├── chainsync/
│   ├── mod.rs              # Public API
│   ├── types.rs            # Point, Tip, Header, Messages
│   ├── codec.rs            # CBOR encoding/decoding
│   ├── state.rs            # State machine
│   ├── client.rs           # Client implementation
│   └── server.rs           # Server implementation (future)
└── blockfetch/
    ├── mod.rs              # Similar structure
    └── ...
```

## References

- [Ouroboros Network Documentation](https://ouroboros-network.cardano.intersectmbo.org/)
- [Handshake Protocol Type](https://ouroboros-network.cardano.intersectmbo.org/ouroboros-network-framework/Ouroboros-Network-Protocol-Handshake-Type.html)
- [ChainSync Protocol Type](https://ouroboros-network.cardano.intersectmbo.org/ouroboros-network-protocols/Ouroboros-Network-Protocol-ChainSync-Type.html)
- [Network Specification PDF](https://ouroboros-network.cardano.intersectmbo.org/pdfs/network-spec)
- [Haskell Implementation](https://github.com/intersectmbo/ouroboros-network)

## Next Steps

1. ✅ Complete research of protocol specifications
2. 🔄 **Design protocol architecture** (THIS DOCUMENT)
3. ⏳ Implement Handshake protocol with proper version negotiation
4. ⏳ Implement ChainSync protocol client
5. ⏳ Test against preview testnet
