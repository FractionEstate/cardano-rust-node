# Testing Handshake Protocol with Preview Testnet

## Overview

This guide walks through testing the handshake protocol implementation against the Cardano preview testnet.

## Prerequisites

1. ✅ Handshake protocol implemented and tested (21/21 tests passing)
2. ✅ ConnectionManager integration complete
3. ✅ ChainSync protocol already implemented
4. 🔄 Preview testnet peer addresses

## Preview Testnet Configuration

### Network Parameters
- **Network Magic:** `1097911063` (0x41717069)
- **Protocol Version:** V15 (Conway era)
- **Target Peers:** Official IOG relays

### Known Preview Testnet Relays

```
preview-node.world.dev.cardano.org:30002
preview-node.play.dev.cardano.org:3001
relays-new.cardano-preview.iohk.io:3001
```

## Step 1: Update Topology Configuration

Create `/workspaces/universal/config/preview-topology.json`:

```json
{
  "bootstrapPeers": [],
  "localRoots": [
    {
      "accessPoints": [
        {
          "address": "preview-node.world.dev.cardano.org",
          "port": 30002
        }
      ],
      "advertise": false,
      "trustable": true,
      "valency": 1
    }
  ],
  "publicRoots": [
    {
      "accessPoints": [
        {
          "address": "relays-new.cardano-preview.iohk.io",
          "port": 3001
        }
      ],
      "advertise": false
    }
  ],
  "useLedgerAfterSlot": 0
}
```

## Step 2: Update Node Configuration

The ConnectionManager needs to be configured with preview testnet network magic:

```rust
// In ConnectionManager::new() or node initialization
let mut config = ConnectionConfig::default();
config.network_magic = NetworkMagic::PREVIEW_TESTNET; // 1097911063
```

## Step 3: Build and Run

```bash
# Build in debug mode for detailed logging
cd /workspaces/universal
cargo build --package cardano-node

# Run with debug logging
RUST_LOG=debug cargo run --bin cardano-node -- run \
  --config config/test-config.json \
  --topology config/preview-topology.json
```

## Expected Log Output

### Successful Handshake

```
INFO  cardano_network::connection::manager: Establishing connection to peer
    connection_id=ConnectionId(...)
    peer_address=preview-node.world.dev.cardano.org:30002

DEBUG cardano_network::connection::manager: TCP connection established, creating multiplexer
    connection_id=ConnectionId(...)

DEBUG cardano_network::connection::manager: Registering protocol handler
    connection_id=ConnectionId(...)
    protocol_id=ProtocolId(0)
    name="Handshake"

DEBUG cardano_network::connection::manager: Registering protocol handler
    connection_id=ConnectionId(...)
    protocol_id=ProtocolId(2)
    name="ChainSync"

INFO  cardano_network::connection::manager: Starting handshake protocol
    connection_id=ConnectionId(...)

DEBUG cardano_network::protocols::handshake::handler: Handling handshake message
    connection_id=ConnectionId(...)
    protocol="Handshake"

DEBUG cardano_network::protocols::handshake::state: Received MsgAcceptVersion
    version=V15
    network_magic=1097911063

INFO  cardano_network::connection::manager: Handshake completed successfully
    connection_id=ConnectionId(...)

INFO  cardano_network::connection::manager: Connection fully established and authenticated
    connection_id=ConnectionId(...)
```

### Expected Handshake Flow

1. **Send MsgProposeVersions:**
   ```
   [0, {14: version_data, 15: version_data}]
   ```

2. **Receive MsgAcceptVersion:**
   ```
   [1, 15, {network_magic: 1097911063, ...}]
   ```

3. **Validate:**
   - Version: V15 (Conway)
   - Network magic: 1097911063 (preview)
   - Diffusion mode: InitiatorAndResponder
   - Peer sharing: Disabled

## Step 4: Verify Handshake Success

### Check Logs for:

✅ **TCP Connection Established**
```
TCP connection established, creating multiplexer
```

✅ **Handshake Started**
```
Starting handshake protocol
```

✅ **Version Negotiated**
```
Received MsgAcceptVersion version=V15
```

✅ **Network Magic Validated**
```
network_magic=1097911063
```

✅ **Connection Authenticated**
```
Connection fully established and authenticated
```

### Check for Errors:

❌ **Timeout**
```
ERROR Handshake timeout
```
**Solution:** Check network connectivity, peer address, firewall

❌ **Version Mismatch**
```
ERROR Handshake failed: VersionMismatch
```
**Solution:** Update supported versions, check peer is on preview testnet

❌ **Network Magic Mismatch**
```
ERROR Network magic mismatch: expected 1097911063, got X
```
**Solution:** Verify peer is on preview testnet, not mainnet/preprod

## Step 5: Test ChainSync (After Handshake)

Once handshake completes, ChainSync should automatically begin:

```
DEBUG cardano_network::protocols::chainsync: Processing ChainSync message
    connection_id=ConnectionId(...)

DEBUG cardano_network::protocols::chainsync: Sending MsgRequestNext

DEBUG cardano_network::protocols::chainsync: Received MsgRollForward
    slot=123456
    height=1000
```

## Troubleshooting

### DNS Resolution Issues

If hostname resolution fails:
```bash
# Test DNS resolution
nslookup preview-node.world.dev.cardano.org

# Use IP address directly if needed
ping preview-node.world.dev.cardano.org
```

### Connection Timeout

```bash
# Check if port is accessible
nc -zv preview-node.world.dev.cardano.org 30002

# Check firewall rules
iptables -L -n | grep 30002
```

### Version Negotiation Failure

- Peer may only support older versions (V13, V14)
- Update `build_version_table()` in handshake/state.rs if needed
- Check Cardano network upgrade status

### Network Magic Issues

- Ensure using `NetworkMagic::PREVIEW_TESTNET`
- Do NOT use `NetworkMagic::MAINNET` for testing
- Verify peer is actually on preview network

## Alternative Test Approach: Local Mock Server

If external connectivity is limited, create a mock server:

```bash
# In another terminal, run a simple server that accepts V15
cargo test --package cardano-network -- --ignored test_live_handshake
```

This test would:
1. Start local TCP server on random port
2. Accept connection
3. Receive MsgProposeVersions
4. Send MsgAcceptVersion with V15
5. Verify handshake completes

## Success Criteria

- [x] TCP connection established
- [x] Handshake initiated (MsgProposeVersions sent)
- [x] Version negotiated (MsgAcceptVersion received)
- [x] Network magic validated (1097911063)
- [x] Connection state: Authenticated
- [ ] No timeout errors
- [ ] No version mismatch errors
- [ ] ChainSync begins after handshake

## Next Steps After Successful Handshake

1. **Monitor ChainSync:** Verify block headers being downloaded
2. **Check Block Heights:** Confirm sync progress
3. **Test Chain Selection:** Verify correct chain followed during forks
4. **Implement BlockFetch:** Download full blocks after headers
5. **Add Validation:** Verify VRF proofs, signatures, etc.

## Performance Metrics

Expected handshake performance:
- **TCP Connection:** 50-200ms (depending on geographic distance)
- **Handshake RTT:** 50-200ms
- **Total Connection Setup:** 100-400ms
- **Timeout:** 30 seconds (configurable)

## References

- Handshake implementation: `crates/cardano-network/src/protocols/handshake/`
- ConnectionManager: `crates/cardano-network/src/connection/manager.rs`
- Network magic constants: `crates/cardano-network/src/protocols/handshake/types.rs`
- Ouroboros spec: [IOHK Technical Spec]

---

**Status:** Ready for testing
**Last Updated:** October 3, 2025
