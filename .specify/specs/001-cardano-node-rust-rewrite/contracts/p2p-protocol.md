# Cardano P2P Network Protocol Contracts

## Mini-Protocol Definitions

### Handshake Protocol
**Purpose**: Initial connection establishment and capability negotiation

#### Messages
```rust
enum HandshakeMessage {
    ProposeVersions {
        versions: HashMap<ProtocolVersion, VersionData>,
    },
    AcceptVersion {
        version: ProtocolVersion,
        version_data: VersionData,
    },
    Refuse {
        reason: RefuseReason,
    },
}

struct VersionData {
    network_magic: u32,
    initiator_only: bool,
}

enum RefuseReason {
    VersionMismatch,
    HandshakeDecodeError(String),
    Refused(String),
}
```

#### State Machine
```
Initial -> ProposeVersions -> AcceptVersion -> Done
       |                  -> Refuse -> Failed
```

#### Validation Rules
- Must propose at least one supported protocol version
- Network magic must match expected network (mainnet/testnet)
- Version negotiation follows semantic versioning compatibility

### ChainSync Protocol
**Purpose**: Synchronize blockchain state between peers

#### Messages
```rust
enum ChainSyncMessage {
    // Client -> Server
    RequestNext,
    FindIntersect {
        points: Vec<Point>,
    },

    // Server -> Client
    RollForward {
        header: BlockHeader,
        tip: Tip,
    },
    RollBackward {
        point: Point,
        tip: Tip,
    },
    IntersectFound {
        point: Point,
        tip: Tip,
    },
    IntersectNotFound {
        tip: Tip,
    },
}

struct Point {
    slot: Slot,
    hash: BlockHash,
}

struct Tip {
    slot: Slot,
    hash: BlockHash,
    height: BlockHeight,
}
```

#### State Machine
```
Idle -> RequestNext -> RollForward/RollBackward -> Idle
     -> FindIntersect -> IntersectFound/NotFound -> Idle
```

#### Validation Rules
- Points in FindIntersect must be in descending slot order
- RollForward must advance slot number
- RollBackward must reference valid previous point

### BlockFetch Protocol
**Purpose**: Efficient block content retrieval

#### Messages
```rust
enum BlockFetchMessage {
    // Client -> Server
    RequestRange {
        range: ChainRange,
    },
    ClientDone,

    // Server -> Client
    StartBatch,
    Block {
        body: BlockBody,
    },
    BatchDone,
    NoBlocks,
}

struct ChainRange {
    from: Point,
    to: Point,
}
```

#### State Machine
```
Idle -> RequestRange -> StartBatch -> Block* -> BatchDone -> Idle
                     |              |
                     -> NoBlocks ----> Idle
```

#### Validation Rules
- Range must be valid (from <= to in slot order)
- Block order must match requested range
- All blocks in range must be provided or NoBlocks sent

### TxSubmission Protocol
**Purpose**: Transaction propagation across network

#### Messages
```rust
enum TxSubmissionMessage {
    // Client -> Server
    RequestTxIds {
        blocking: bool,
        ack: u16,
        req: u16,
    },
    ReplyTxIds {
        tx_ids: Vec<(TxId, TxSize)>,
    },
    RequestTxs {
        tx_ids: Vec<TxId>,
    },
    ReplyTxs {
        txs: Vec<Transaction>,
    },

    // Server -> Client
    RequestTxIds { /* same as above */ },
    ReplyTxIds { /* same as above */ },
    RequestTxs { /* same as above */ },
    ReplyTxs { /* same as above */ },
}
```

#### State Machine
```
Idle <-> RequestTxIds -> ReplyTxIds -> Idle
     <-> RequestTxs -> ReplyTxs -> Idle
```

#### Validation Rules
- Ack/req numbers must be consistent with protocol flow
- Requested tx_ids must exist in mempool
- Transaction CBOR must be valid

### KeepAlive Protocol
**Purpose**: Connection health monitoring

#### Messages
```rust
enum KeepAliveMessage {
    KeepAlive {
        cookie: u16,
    },
    KeepAliveResponse {
        cookie: u16,
    },
}
```

#### Timing Rules
- KeepAlive sent every 60 seconds of inactivity
- Response must be received within 30 seconds
- Cookie must match in response

## Connection Management

### PeerSelection
**Purpose**: Maintain optimal set of connections

#### Peer Categories
- **Cold**: Known peers not connected
- **Warm**: Connected but not in use
- **Hot**: Actively exchanging data

#### Selection Policies
```rust
struct PeerSelectionPolicy {
    target_hot_peers: usize,      // 20
    target_warm_peers: usize,     // 100
    target_cold_peers: usize,     // 1000
    max_peer_share_threshold: f64, // 0.2
}
```

### Gossip Protocol
**Purpose**: Peer address advertisement

#### Messages
```rust
enum GossipMessage {
    PeerAdvertisement {
        peers: Vec<PeerAddress>,
        ttl: Duration,
    },
    PeerRequest,
}

struct PeerAddress {
    ip: IpAddr,
    port: u16,
    valency: u8,
}
```

#### Validation Rules
- TTL must be reasonable (< 24 hours)
- IP addresses must be routable
- Valency indicates connection priority

## Error Handling

### Protocol Errors
```rust
enum ProtocolError {
    HandshakeFailure(HandshakeError),
    DecodingError(String),
    ProtocolViolation(String),
    TimeoutError,
    ConnectionClosed,
}

enum HandshakeError {
    UnsupportedVersion,
    NetworkMismatch,
    MalformedMessage,
}
```

### Error Recovery
- Handshake errors: Close connection, blacklist temporarily
- Decoding errors: Log error, close connection gracefully
- Protocol violations: Penalize peer reputation, potential ban
- Timeouts: Retry with backoff, eventual disconnection

## CBOR Encoding

### Message Framing
```
Frame := [MessageType, MessageBody]
MessageType := Integer (0-255)
MessageBody := CBOR-encoded message data
```

### Version Compatibility
- Backward compatibility for minor version differences
- Unknown fields ignored (forward compatibility)
- Required field removal breaks compatibility

## Security Considerations

### DoS Protection
- Rate limiting per peer and globally
- Maximum message sizes enforced
- Connection limits per IP address
- Reputation-based throttling

### Authentication
- No authentication at protocol level
- Peer identification through blockchain activity
- Reputation system based on behavior

### Privacy
- No PII transmitted in protocol messages
- IP addresses visible to direct peers only
- Transaction origin obfuscated through mixing

## Testing Contracts

### Protocol Compliance Tests
Each mini-protocol must pass:
- State machine validation
- Message serialization round-trip
- Error condition handling
- Timeout behavior
- Resource exhaustion scenarios

### Interoperability Tests
- Connect to Haskell cardano-node
- Exchange messages successfully
- Handle version negotiation
- Maintain long-lived connections

### Performance Tests
- Message throughput benchmarks
- Connection establishment latency
- Memory usage under load
- CPU usage during sync
