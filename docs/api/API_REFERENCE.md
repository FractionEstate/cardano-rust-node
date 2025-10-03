# Cardano Node API Documentation

REST API and WebSocket interface for Cardano Node interactions.

## Table of Contents

- [Overview](#overview)
- [REST API](#rest-api)
- [WebSocket API](#websocket-api)
- [Local Socket Protocol](#local-socket-protocol)
- [Examples](#examples)

## Overview

The Cardano Node provides multiple API interfaces for programmatic interaction:

1. **REST API**: HTTP/HTTPS endpoints for querying and control
2. **WebSocket API**: Real-time updates and streaming data
3. **Local Socket**: Unix domain socket for local IPC (cardano-cli compatible)

### Base Configuration

```yaml
# In config.json
"hasEKG": true,
"hasPrometheus": true,
"setupBackends": ["KatipBK"],
"setupScribes": [{
  "scKind": "StdoutSK",
  "scName": "stdout",
  "scFormat": "ScJson"
}]
```

## REST API

### Base URL

```
http://localhost:12798/api/v1
```

### Authentication

Currently no authentication required for local access. Production deployments should use API keys or mTLS.

### Endpoints

#### Node Information

##### `GET /node/info`

Get basic node information.

**Response:**
```json
{
  "version": "8.7.3",
  "network": "mainnet",
  "protocolVersion": {"major": 8, "minor": 0},
  "uptime": 3600,
  "syncProgress": 0.95
}
```

##### `GET /node/status`

Get detailed node status.

**Response:**
```json
{
  "chainTip": {
    "slot": 98765432,
    "hash": "abc123...",
    "blockNo": 12345678
  },
  "peers": {
    "connected": 15,
    "available": 20
  },
  "mempool": {
    "txCount": 1234,
    "bytes": 524288
  },
  "resources": {
    "cpu": 25.5,
    "memory": 2048,
    "disk": 50000
  }
}
```

#### Blockchain Queries

##### `GET /chain/tip`

Get current chain tip.

**Response:**
```json
{
  "slot": 98765432,
  "hash": "abc123...",
  "blockNo": 12345678,
  "epoch": 450
}
```

##### `GET /chain/block/:hash`

Get block by hash.

**Parameters:**
- `:hash` - Block hash

**Response:**
```json
{
  "hash": "abc123...",
  "slot": 98765432,
  "blockNo": 12345678,
  "previousHash": "def456...",
  "transactions": 25,
  "size": 65536
}
```

##### `GET /chain/block/:number`

Get block by number.

**Parameters:**
- `:number` - Block number

##### `GET /protocol/parameters`

Get current protocol parameters.

**Response:**
```json
{
  "protocolVersion": {"major": 8, "minor": 0},
  "minFeeA": 44,
  "minFeeB": 155381,
  "maxTxSize": 16384,
  "maxBlockBodySize": 90112,
  "maxBlockHeaderSize": 1100,
  "keyDeposit": 2000000,
  "poolDeposit": 500000000
}
```

#### Address Queries

##### `GET /address/:address/utxos`

Get UTxOs for an address.

**Parameters:**
- `:address` - Cardano address

**Response:**
```json
{
  "address": "addr1q...",
  "utxos": [
    {
      "txHash": "abc123...",
      "outputIndex": 0,
      "amount": {
        "lovelace": 1000000
      },
      "datumHash": null
    }
  ]
}
```

##### `GET /address/:address/transactions`

Get transaction history for an address.

**Parameters:**
- `:address` - Cardano address
- `limit` (optional) - Max results (default: 50)
- `offset` (optional) - Pagination offset (default: 0)

#### Transaction Operations

##### `POST /transaction/submit`

Submit a signed transaction.

**Request Body:**
```json
{
  "type": "Tx ConwayEra",
  "description": "",
  "cborHex": "84a3..."
}
```

**Response:**
```json
{
  "txHash": "abc123...",
  "status": "accepted"
}
```

##### `GET /transaction/:txhash`

Get transaction details.

**Parameters:**
- `:txhash` - Transaction hash

**Response:**
```json
{
  "txHash": "abc123...",
  "block": "def456...",
  "slot": 98765432,
  "inputs": [...],
  "outputs": [...],
  "fee": 170000
}
```

##### `GET /mempool/transactions`

Get current mempool transactions.

**Response:**
```json
{
  "count": 1234,
  "transactions": [
    {
      "txHash": "abc123...",
      "fee": 170000,
      "size": 450
    }
  ]
}
```

#### Stake Pool Information

##### `GET /stake-pools`

List all stake pools.

**Query Parameters:**
- `limit` - Max results (default: 100)
- `offset` - Pagination offset (default: 0)

**Response:**
```json
{
  "pools": [
    {
      "poolId": "pool1...",
      "ticker": "POOL1",
      "pledge": 500000000000,
      "margin": 0.02,
      "fixedCost": 340000000
    }
  ],
  "total": 3000
}
```

##### `GET /stake-pool/:poolId`

Get stake pool details.

**Parameters:**
- `:poolId` - Stake pool ID

**Response:**
```json
{
  "poolId": "pool1...",
  "ticker": "POOL1",
  "name": "My Stake Pool",
  "description": "...",
  "homepage": "https://...",
  "pledge": 500000000000,
  "margin": 0.02,
  "fixedCost": 340000000,
  "owners": ["stake1..."],
  "relays": [...],
  "metadataHash": "abc123..."
}
```

#### Metrics

##### `GET /metrics`

Get Prometheus-format metrics.

**Response:**
```
# HELP cardano_blocks_total Total number of blocks processed
# TYPE cardano_blocks_total counter
cardano_blocks_total 12345678

# HELP cardano_transactions_total Total number of transactions processed
# TYPE cardano_transactions_total counter
cardano_transactions_total 98765432

# HELP cardano_peers_connected Number of connected peers
# TYPE cardano_peers_connected gauge
cardano_peers_connected 15
```

##### `GET /metrics/ekg`

Get EKG-format metrics (JSON).

**Response:**
```json
{
  "cardano.node.metrics.epoch": 450,
  "cardano.node.metrics.slot": 98765432,
  "cardano.node.metrics.blockNum": 12345678,
  "cardano.node.metrics.txsProcessed": 98765432,
  "cardano.node.metrics.peersConnected": 15
}
```

## WebSocket API

### Connection

```javascript
const ws = new WebSocket('ws://localhost:12798/ws');
```

### Message Format

All messages are JSON:

```json
{
  "type": "subscribe|unsubscribe|data|error",
  "channel": "blocks|transactions|mempool|peers",
  "data": { ... }
}
```

### Channels

#### `blocks` - Block Updates

Subscribe to new blocks.

**Subscribe:**
```json
{
  "type": "subscribe",
  "channel": "blocks"
}
```

**Data:**
```json
{
  "type": "data",
  "channel": "blocks",
  "data": {
    "hash": "abc123...",
    "slot": 98765432,
    "blockNo": 12345678,
    "transactions": 25
  }
}
```

#### `transactions` - Transaction Updates

Subscribe to new transactions.

**Subscribe:**
```json
{
  "type": "subscribe",
  "channel": "transactions",
  "filter": {
    "address": "addr1q..."  // optional
  }
}
```

**Data:**
```json
{
  "type": "data",
  "channel": "transactions",
  "data": {
    "txHash": "abc123...",
    "inputs": [...],
    "outputs": [...],
    "fee": 170000
  }
}
```

#### `mempool` - Mempool Updates

Subscribe to mempool changes.

**Subscribe:**
```json
{
  "type": "subscribe",
  "channel": "mempool"
}
```

**Data:**
```json
{
  "type": "data",
  "channel": "mempool",
  "data": {
    "added": ["abc123..."],
    "removed": ["def456..."],
    "count": 1234
  }
}
```

#### `peers` - Peer Updates

Subscribe to peer connection events.

**Subscribe:**
```json
{
  "type": "subscribe",
  "channel": "peers"
}
```

**Data:**
```json
{
  "type": "data",
  "channel": "peers",
  "data": {
    "event": "connected|disconnected",
    "peer": {
      "address": "192.168.1.100:3001",
      "latency": 25
    }
  }
}
```

### Error Handling

```json
{
  "type": "error",
  "message": "Error description",
  "code": "ERROR_CODE"
}
```

## Local Socket Protocol

Compatible with cardano-cli for local IPC.

### Connection

```bash
export CARDANO_NODE_SOCKET_PATH=/path/to/node.socket
```

### Protocol

Uses CBOR-encoded messages over Unix domain socket.

#### Query Chain Tip

**Request:**
```cbor
[0, "chain-tip"]
```

**Response:**
```cbor
{
  "slot": 98765432,
  "hash": "abc123...",
  "blockNo": 12345678
}
```

#### Submit Transaction

**Request:**
```cbor
[1, "submit-tx", <tx-cbor>]
```

**Response:**
```cbor
{
  "success": true,
  "txHash": "abc123..."
}
```

## Examples

### REST API

#### Python

```python
import requests

# Get node info
response = requests.get('http://localhost:12798/api/v1/node/info')
info = response.json()
print(f"Node version: {info['version']}")

# Query chain tip
response = requests.get('http://localhost:12798/api/v1/chain/tip')
tip = response.json()
print(f"Current block: {tip['blockNo']}")

# Submit transaction
tx_data = {
    "type": "Tx ConwayEra",
    "cborHex": "84a3..."
}
response = requests.post('http://localhost:12798/api/v1/transaction/submit',
                         json=tx_data)
result = response.json()
print(f"Transaction submitted: {result['txHash']}")
```

#### JavaScript/Node.js

```javascript
const axios = require('axios');

const API_BASE = 'http://localhost:12798/api/v1';

// Get protocol parameters
async function getProtocolParams() {
  const response = await axios.get(`${API_BASE}/protocol/parameters`);
  return response.data;
}

// Query address UTxOs
async function getAddressUtxos(address) {
  const response = await axios.get(`${API_BASE}/address/${address}/utxos`);
  return response.data.utxos;
}

// Usage
(async () => {
  const params = await getProtocolParams();
  console.log('Min fee A:', params.minFeeA);

  const utxos = await getAddressUtxos('addr1q...');
  console.log('UTxOs:', utxos.length);
})();
```

#### Rust

```rust
use reqwest;
use serde_json::Value;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();

    // Get node status
    let status: Value = client
        .get("http://localhost:12798/api/v1/node/status")
        .send()
        .await?
        .json()
        .await?;

    println!("Chain tip: {}", status["chainTip"]["blockNo"]);
    println!("Peers: {}", status["peers"]["connected"]);

    Ok(())
}
```

### WebSocket API

#### JavaScript

```javascript
const ws = new WebSocket('ws://localhost:12798/ws');

ws.onopen = () => {
  // Subscribe to new blocks
  ws.send(JSON.stringify({
    type: 'subscribe',
    channel: 'blocks'
  }));

  // Subscribe to transactions for specific address
  ws.send(JSON.stringify({
    type: 'subscribe',
    channel: 'transactions',
    filter: { address: 'addr1q...' }
  }));
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);

  if (msg.type === 'data') {
    if (msg.channel === 'blocks') {
      console.log('New block:', msg.data.blockNo);
    } else if (msg.channel === 'transactions') {
      console.log('New transaction:', msg.data.txHash);
    }
  }
};

ws.onerror = (error) => {
  console.error('WebSocket error:', error);
};
```

#### Python

```python
import asyncio
import websockets
import json

async def subscribe_blocks():
    uri = "ws://localhost:12798/ws"
    async with websockets.connect(uri) as websocket:
        # Subscribe to blocks
        await websocket.send(json.dumps({
            'type': 'subscribe',
            'channel': 'blocks'
        }))

        # Listen for updates
        while True:
            message = await websocket.recv()
            data = json.loads(message)

            if data['type'] == 'data' and data['channel'] == 'blocks':
                print(f"New block: {data['data']['blockNo']}")

asyncio.run(subscribe_blocks())
```

## Rate Limiting

Default rate limits:

- REST API: 100 requests/minute per IP
- WebSocket: 10 subscriptions per connection
- Transaction submission: 10 tx/minute per IP

## Error Codes

| Code | Description |
|------|-------------|
| 400 | Bad Request - Invalid parameters |
| 404 | Not Found - Resource doesn't exist |
| 429 | Too Many Requests - Rate limit exceeded |
| 500 | Internal Server Error |
| 503 | Service Unavailable - Node not synced |

## See Also

- [CLI Reference](CLI_REFERENCE.md) - Command-line interface
- [README.md](../README.md) - Project overview
- [PRODUCTION_READY.md](../PRODUCTION_READY.md) - Deployment guide
