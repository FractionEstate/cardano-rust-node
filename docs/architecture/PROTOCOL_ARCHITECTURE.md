# Ouroboros Network Protocol Architecture

## Overview

This document specifies the implementation of Ouroboros mini-protocols for cardano-node-rust. These protocols enable block synchronization, transaction propagation, and network coordination between Cardano nodes.

## Available Rust Components

From `cardano-base-rust` (already a project dependency):

- **cardano-binary**: CBOR serialization/deserialization (byte-compatible with Haskell)
- **cardano-crypto-class**: VRF (Praos, batch-compat), Ed25519, Blake2 hashing
- **cardano-slotting**: `EpochNo`, `SlotNo`, `BlockNo`, epoch/time conversions
- **cardano-vrf-pure**: Pure Rust VRF implementations (draft-03, draft-13)

## Protocol Stack Architecture

```text
┌─────────────────────────────────────────────────┐
│           Application Layer                      │
│  (ChainSync, BlockFetch, TxSubmission)          │
└─────────────────────────────────────────────────┘
                    ↓↑
┌─────────────────────────────────────────────────┐
│         Multiplexer Layer                        │
│  (Protocol ID routing, message framing)         │
└─────────────────────────────────────────────────┘
                    ↓↑
┌─────────────────────────────────────────────────┐
│      Handshake Protocol (mandatory first)        │
│  (Version negotiation, network magic)           │
└─────────────────────────────────────────────────┘
                    ↓↑
┌─────────────────────────────────────────────────┐
│          TCP Transport Layer                     │
│  (Already working in v10.5.1)                   │
└─────────────────────────────────────────────────┘
```text

## Protocol Specifications

### 1. Multiplexer (Mux)

**Purpose**: Route multiple protocol conversations over a single TCP connection.

**Message Format**:
```text
┌──────────────┬────────────┬────────────┬──────────────┐
│ Transmission │ Protocol   │ Payload    │ Payload      │
│ Time (4B)    │ ID (2B)    │ Length (2B)│ (variable)   │
└──────────────┴────────────┴────────────┴──────────────┘
```text

**Protocol IDs**:
```rust
pub const PROTOCOL_HANDSHAKE: u16 = 0;
pub const PROTOCOL_CHAINSYNC: u16 = 2;
pub const PROTOCOL_BLOCKFETCH: u16 = 3;
pub const PROTOCOL_TXSUBMISSION: u16 = 4;
pub const PROTOCOL_KEEPALIVE: u16 = 8;
```text

**Rust Implementation**:
```rust
pub struct MuxMessage {
    pub timestamp: u32,       // Transmission time (milliseconds since connection)
    pub protocol_id: u16,     // Which protocol this message belongs to
    pub payload: Vec<u8>,     // CBOR-encoded protocol message
}

pub struct Multiplexer {
    tcp_stream: TcpStream,
    protocol_handlers: HashMap<u16, Box<dyn ProtocolHandler>>,
    send_queue: VecDeque<MuxMessage>,
    recv_buffer: BytesMut,
}
```text

**State Machine**:
- `Idle`: Waiting for messages
- `Receiving`: Reading message header/payload
- `Sending`: Writing message to TCP stream
- `Error`: Connection error, cleanup required

### 2. Handshake Protocol

**Purpose**: Negotiate protocol versions and verify network compatibility before any other communication.

**Protocol ID**: `0`

**Message Types** (CBOR encoded):
```rust
pub enum HandshakeMessage {
    /// Propose acceptable protocol versions
    ProposeVersions {
        versions: HashMap<u16, VersionData>,
    },
    /// Accept a specific version
    AcceptVersion {
        version: u16,
        version_data: VersionData,
    },
    /// Reject handshake (incompatible versions)
    Refuse {
        reasons: Vec<RefuseReason>,
    },
}

pub struct VersionData {
    pub network_magic: u32,           // Network identifier (preview = 2)
    pub initiator_only_diffusion: bool, // P2P mode flag
}

pub enum RefuseReason {
    VersionMismatch { offered: Vec<u16> },
    HandshakeDecodeError { message: String },
    Refused { reason: String },
}
```text

**CBOR Encoding** (using `cardano-binary`):
```rust
use cardano_binary::{serialize, deserialize};

// ProposeVersions CBOR structure:
// [0, { version_num: version_data, ... }]
//
// AcceptVersion CBOR structure:
// [1, version_num, version_data]
//
// Refuse CBOR structure:
// [2, [refuse_reasons]]
```text

**State Machine**:
```text
Client:
  Start → ProposeVersions → AwaitAccept → Done
                               ↓
                            Refused → Closed

Server:
  Start → AwaitProposal → AcceptVersion → Done
                          ↓
                       Refuse → Closed
```text

**Rust Implementation**:
```rust
pub struct HandshakeProtocol {
    state: HandshakeState,
    supported_versions: HashMap<u16, VersionData>,
    agreed_version: Option<u16>,
    network_magic: u32,
}

pub enum HandshakeState {
    Start,
    AwaitProposal,
    AwaitAccept,
    Done { version: u16 },
    Failed { reason: String },
}

impl ProtocolHandler for HandshakeProtocol {
    fn protocol_id(&self) -> u16 { 0 }

    fn handle_message(&mut self, payload: &[u8]) -> Result<Option<Vec<u8>>, ProtocolError>;

    fn next_message(&mut self) -> Result<Option<Vec<u8>>, ProtocolError>;
}
```text

**Critical Requirements**:
- MUST be the first protocol to complete
- Blocks all other protocols until `Done` state
- Network magic verification prevents wrong-network connections
- Version negotiation ensures protocol compatibility

### 3. ChainSync Protocol

**Purpose**: Synchronize blockchain headers between nodes. Used to discover the best chain and request block headers.

**Protocol ID**: `2`

**Message Types**:
```rust
pub enum ChainSyncMessage {
    /// Request next block header
    RequestNext,

    /// Response: no more headers available
    AwaitReply,

    /// Response: here's the next header
    RollForward {
        header: BlockHeader,
        tip: Tip,
    },

    /// Response: chain rolled back to this point
    RollBackward {
        point: Point,
        tip: Tip,
    },

    /// Find intersection with peer's chain
    FindIntersect {
        points: Vec<Point>,
    },

    /// Intersection found response
    IntersectFound {
        point: Point,
        tip: Tip,
    },

    /// Intersection not found
    IntersectNotFound {
        tip: Tip,
    },

    /// Client finished syncing
    Done,
}

pub struct BlockHeader {
    pub slot: SlotNo,           // From cardano-slotting
    pub hash: Hash,             // Blake2b-256
    pub block_no: BlockNo,      // From cardano-slotting
    pub prev_hash: Hash,
    pub vrf_vkey: VrfVerificationKey,  // From cardano-crypto-class
    pub vrf_result: VrfOutput,         // From cardano-crypto-class
    pub block_body_size: u64,
    // ... additional consensus fields
}

pub struct Point {
    pub slot: SlotNo,
    pub hash: Hash,
}

pub struct Tip {
    pub point: Point,
    pub block_no: BlockNo,
}
```text

**CBOR Encoding**:
```rust
// RequestNext: [0]
// AwaitReply: [1]
// RollForward: [2, header, tip]
// RollBackward: [3, point, tip]
// FindIntersect: [4, [points]]
// IntersectFound: [5, point, tip]
// IntersectNotFound: [6, tip]
// Done: [7]
```text

**State Machine**:
```text
Client:
  Idle → FindIntersect → AwaitIntersect → IntersectFound → CanAwaitReply
                                           ↓
                                      IntersectNotFound → Idle

  CanAwaitReply → RequestNext → AwaitReply → RollForward → CanAwaitReply
                                            → RollBackward → CanAwaitReply

  * → Done → StDone

Server:
  Idle → AwaitRequest → ProcessRequest → Idle
```text

**Rust Implementation**:
```rust
pub struct ChainSyncClient {
    state: ChainSyncState,
    chain_tip: Option<Tip>,
    intersection_points: Vec<Point>,
    received_headers: VecDeque<BlockHeader>,
}

pub enum ChainSyncState {
    Idle,
    AwaitIntersect,
    CanAwaitReply,
    AwaitReply,
    Done,
}

impl ProtocolHandler for ChainSyncClient {
    fn protocol_id(&self) -> u16 { 2 }

    fn handle_message(&mut self, payload: &[u8]) -> Result<Option<Vec<u8>>, ProtocolError> {
        let msg: ChainSyncMessage = deserialize(payload)?;

        match (&self.state, msg) {
            (ChainSyncState::AwaitIntersect, ChainSyncMessage::IntersectFound { point, tip }) => {
                self.chain_tip = Some(tip);
                self.state = ChainSyncState::CanAwaitReply;
                Ok(Some(serialize(&ChainSyncMessage::RequestNext)?))
            }
            (ChainSyncState::AwaitReply, ChainSyncMessage::RollForward { header, tip }) => {
                self.received_headers.push_back(header);
                self.chain_tip = Some(tip);
                self.state = ChainSyncState::CanAwaitReply;
                Ok(None) // Process header, will request next later
            }
            // ... other state transitions
            _ => Err(ProtocolError::UnexpectedMessage),
        }
    }
}
```text

**Integration with Slotting**:
```rust
use cardano_slotting::{SlotNo, BlockNo, EpochNo, EpochInfo};

// Validate header slot progression
fn validate_header_slot(header: &BlockHeader, prev_header: &BlockHeader) -> bool {
    header.slot > prev_header.slot
}

// Calculate epoch from slot
fn get_epoch(slot: SlotNo, epoch_info: &EpochInfo) -> EpochNo {
    // Use cardano-slotting's epoch_info functions
    epoch_info_epoch(epoch_info, slot)
}
```text

### 4. BlockFetch Protocol

**Purpose**: Download full block bodies after ChainSync has identified which blocks are needed.

**Protocol ID**: `3`

**Message Types**:
```rust
pub enum BlockFetchMessage {
    /// Request range of blocks
    RequestRange {
        from: Point,
        to: Point,
    },

    /// Client has no more requests
    ClientDone,

    /// Response: starting to send blocks
    StartBatch,

    /// Response: no blocks available
    NoBlocks,

    /// Response: here's a block
    Block {
        body: Vec<u8>,  // CBOR-encoded block
    },

    /// Response: all blocks sent
    BatchDone,
}
```text

**CBOR Encoding**:
```rust
// RequestRange: [0, from_point, to_point]
// ClientDone: [1]
// StartBatch: [2]
// NoBlocks: [3]
// Block: [4, block_cbor_bytes]
// BatchDone: [5]
```text

**State Machine**:
```text
Client:
  Idle → RequestRange → AwaitStartBatch → StartBatch → AwaitBlock
                                        → NoBlocks → Idle

  AwaitBlock → Block → AwaitBlock
             → BatchDone → Idle

  * → ClientDone → Done

Server:
  Idle → AwaitRequest → ProcessRequest → SendingBatch → Idle
```text

**Rust Implementation**:
```rust
pub struct BlockFetchClient {
    state: BlockFetchState,
    pending_requests: VecDeque<(Point, Point)>,
    received_blocks: VecDeque<Vec<u8>>,
}

pub enum BlockFetchState {
    Idle,
    AwaitStartBatch,
    AwaitBlock { expected_count: usize },
    Done,
}
```text

**Pipelining**:
```rust
// Allow multiple outstanding block requests
pub const MAX_PIPELINED_REQUESTS: usize = 10;

impl BlockFetchClient {
    fn can_pipeline(&self) -> bool {
        self.pending_requests.len() < MAX_PIPELINED_REQUESTS
    }

    fn request_block_range(&mut self, from: Point, to: Point) -> Result<()> {
        if !self.can_pipeline() {
            return Err(ProtocolError::PipelineFull);
        }
        self.pending_requests.push_back((from, to));
        Ok(())
    }
}
```text

### 5. TxSubmission Protocol

**Purpose**: Propagate transactions through the network. Nodes announce available transactions and request ones they don't have.

**Protocol ID**: `4`

**Message Types**:
```rust
pub enum TxSubmissionMessage {
    /// Request transaction IDs from peer
    RequestTxIds {
        blocking: bool,    // Wait for new txs if none available
        ack: u16,          // Number of txs acknowledged
        req: u16,          // Number of tx IDs requested
    },

    /// Response: here are some tx IDs
    ReplyTxIds {
        txs: Vec<(TxId, TxSize)>,
    },

    /// Request full transaction bodies
    RequestTxs {
        tx_ids: Vec<TxId>,
    },

    /// Response: here are the transactions
    ReplyTxs {
        txs: Vec<Vec<u8>>,  // CBOR-encoded transactions
    },

    /// Client finished
    Done,
}

pub type TxId = [u8; 32];  // Blake2b-256 hash
pub type TxSize = u32;     // Size in bytes
```text

**CBOR Encoding**:
```rust
// RequestTxIds: [0, blocking, ack, req]
// ReplyTxIds: [1, [(txid, size), ...]]
// RequestTxs: [2, [txids]]
// ReplyTxs: [3, [tx_cbor_bytes, ...]]
// Done: [4]
```text

**State Machine**:
```text
Client:
  Idle → RequestTxIds → AwaitTxIds → ReplyTxIds → Idle
                                                 → RequestTxs → AwaitTxs → ReplyTxs → Idle

  * → Done → StDone

Server:
  Idle → AwaitRequest → ProcessRequest → Idle
```text

## Implementation Plan

### Phase 1: Foundation (Current)
1. ✅ Network connectivity (TCP, DNS, P2P topology)
2. ✅ Research available Rust components
3. 🔄 Design protocol architecture (this document)

### Phase 2: Core Protocols
1. Implement Multiplexer
   - Message framing
   - Protocol ID routing
   - Send/receive queues

2. Implement Handshake Protocol
   - CRITICAL: Must complete before any other protocol
   - Version negotiation
   - Network magic verification
   - CBOR encoding using `cardano-binary`

3. Implement ChainSync Protocol
   - Header synchronization
   - Chain intersection finding
   - Rollback handling
   - Use `cardano-slotting` for slot/epoch handling

4. Implement BlockFetch Protocol
   - Block body retrieval
   - Range requests
   - Pipelining support

### Phase 3: Integration
1. Wire protocols to node runtime
2. Implement block validation using `cardano-crypto-class` (VRF verification)
3. Chain selection logic
4. Storage integration

### Phase 4: Transaction Support
1. Implement TxSubmission Protocol
2. Mempool management
3. Transaction validation

## File Structure

```text
crates/cardano-network/src/
├── protocols/
│   ├── mod.rs                  # Protocol trait definitions
│   ├── multiplexer.rs          # Mux implementation
│   ├── handshake.rs            # Handshake protocol
│   ├── chainsync.rs            # ChainSync protocol
│   ├── blockfetch.rs           # BlockFetch protocol
│   └── txsubmission.rs         # TxSubmission protocol
├── messages/
│   ├── mod.rs
│   ├── handshake_messages.rs   # Handshake CBOR types
│   ├── chainsync_messages.rs   # ChainSync CBOR types
│   ├── blockfetch_messages.rs  # BlockFetch CBOR types
│   └── txsubmission_messages.rs # TxSubmission CBOR types
└── lib.rs
```text

## Testing Strategy

### Unit Tests
- Protocol state machine transitions
- CBOR encoding/decoding (verify byte-compatibility)
- Message validation

### Integration Tests
- Connect to preview testnet
- Complete handshake
- Sync first 1000 blocks
- Validate block headers (VRF, signatures)

### Compatibility Tests
- Verify protocol messages match Haskell node format
- Test with cardano-node v9.x and v10.x

## References

- **Ouroboros Network Spec**: [Input Output HK - Cardano Network Documentation]
- **Haskell Implementation**: `ouroboros-network` package
- **CBOR RFC**: RFC 8949
- **cardano-base-rust**: https://github.com/FractionEstate/cardano-base-rust
- **VRF Spec**: draft-irtf-cfrg-vrf-03, draft-irtf-cfrg-vrf-13

## Next Steps

1. ✅ Create this design document
2. Implement Multiplexer with protocol routing
3. Implement Handshake protocol (CRITICAL FIRST)
4. Implement ChainSync protocol
5. Test sync with preview testnet
