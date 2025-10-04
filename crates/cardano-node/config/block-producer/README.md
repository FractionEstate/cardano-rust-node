# Example Block Producer Configuration for Cardano Node (Rust)

This directory contains examples of how to configure the Cardano Node Rust implementation
as a block producer (stake pool operator).

## Quick Start

1. **Generate Keys** (if you don't have them already):
   ```bash
   # Generate VRF keys
   cardano-cli node key-gen-VRF \
     --verification-key-file vrf.vkey \
     --signing-key-file vrf.skey

   # Generate KES keys
   cardano-cli node key-gen-KES \
     --verification-key-file kes.vkey \
     --signing-key-file kes.skey

   # Generate cold keys
   cardano-cli node key-gen \
     --cold-verification-key-file cold.vkey \
     --cold-signing-key-file cold.skey \
     --operational-certificate-issue-counter-file cold.counter

   # Generate operational certificate
   cardano-cli node issue-op-cert \
     --kes-verification-key-file kes.vkey \
     --cold-signing-key-file cold.skey \
     --operational-certificate-issue-counter cold.counter \
     --kes-period 0 \
     --out-file node.cert
   ```

2. **Configure the Node** - Add block producer configuration to your node config:
   ```json
   {
     "block_producer": {
       "enabled": true,
       "pool_id": "pool1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
       "vrf_key": {
         "signing_key_file": "keys/vrf.skey",
         "verification_key_file": "keys/vrf.vkey",
         "format": "cardano-cli"
       },
       "kes_key": {
         "signing_key_file": "keys/kes.skey",
         "verification_key_file": "keys/kes.vkey",
         "kes_period": 0,
         "max_kes_evolutions": 62,
         "start_kes_period": 0,
         "format": "cardano-cli"
       },
       "operational_cert": {
         "cert_file": "keys/node.cert",
         "issue_counter": 0,
         "counter_file": "keys/cold.counter"
       }
     }
   }
   ```

3. **Start the Node**:
   ```bash
   cardano-node run \
     --config config/mainnet-config.json \
     --topology config/mainnet-topology.json \
     --database-path db/ \
     --socket-path db/node.socket
   ```

## Configuration Files

### block-producer-config.json

Complete example showing all block producer configuration options with detailed comments.
This is a production-ready configuration template.

### block-producer-minimal.json

Minimal block producer configuration with only required fields.
Good starting point for testing.

### block-producer-testnet.json

Block producer configuration for preview/preprod testnets.
Includes testnet-specific settings.

## Key Management

### Key Files Required

For block production, you need three types of cryptographic keys:

1. **VRF Key (Verifiable Random Function)**
   - Used for the slot leader election lottery
   - Determines if your pool is elected to produce a block in a given slot
   - Files: `vrf.skey` (private), `vrf.vkey` (public)

2. **KES Key (Key Evolving Signature)**
   - Used for signing blocks
   - Evolves over time for forward security
   - Must be rotated every ~90 days (62 periods on mainnet)
   - Files: `kes.skey` (private), `kes.vkey` (public)

3. **Operational Certificate**
   - Binds your KES key to your stake pool's cold key
   - Signed by the cold key
   - File: `node.cert`

4. **Cold Key** (optional, for key rotation)
   - Your stake pool's master key
   - Should be kept offline and secure
   - Only needed for generating operational certificates
   - Files: `cold.skey` (private - HIGHLY SENSITIVE), `cold.vkey` (public)

### Key Formats

The Rust node supports multiple key formats:

- `cardano-cli`: JSON format used by Haskell cardano-cli (default, recommended)
- `raw-hex`: Hex-encoded raw key bytes
- `raw-binary`: Raw binary key bytes

Example cardano-cli format (vrf.skey):
```json
{
  "type": "VrfSigningKey_PraosVRF",
  "description": "VRF Signing Key",
  "cborHex": "5820abc123..."
}
```

### KES Key Rotation

KES keys must be rotated periodically:

1. **Check current KES period**:
   ```bash
   cardano-cli query tip --testnet-magic 2
   # Calculate: KES period = slot / slots_per_kes_period
   ```

2. **Generate new KES key**:
   ```bash
   cardano-cli node key-gen-KES \
     --verification-key-file kes-new.vkey \
     --signing-key-file kes-new.skey
   ```

3. **Issue new operational certificate**:
   ```bash
   cardano-cli node issue-op-cert \
     --kes-verification-key-file kes-new.vkey \
     --cold-signing-key-file cold.skey \
     --operational-certificate-issue-counter cold.counter \
     --kes-period <CURRENT_KES_PERIOD> \
     --out-file node-new.cert
   ```

4. **Update configuration and restart node**

### Auto-rotation (Optional)

The Rust node supports automatic KES rotation:

```json
{
  "kes_key": {
    "auto_rotation": {
      "enabled": true,
      "rotation_margin_periods": 5,
      "rotation_dir": "keys/rotated",
      "alert_margin_periods": 10
    }
  }
}
```

This will:
- Alert you when 10 periods remain
- Auto-rotate when 5 periods remain
- Save old keys to `keys/rotated/`

## Block Forging Behavior

Configure how your node produces blocks:

```json
{
  "forging_behavior": {
    "forging_delay_ms": 100,
    "max_txs_per_block": 10000,
    "max_block_size_bytes": 90112,
    "prefer_high_fees": true,
    "include_txs": true,
    "continue_on_tx_validation_failure": true
  }
}
```

- `forging_delay_ms`: Wait this long before forging to gather more transactions
- `max_txs_per_block`: Maximum transactions to include
- `max_block_size_bytes`: Maximum block size (protocol limit: 90,112 bytes)
- `prefer_high_fees`: Prioritize higher fee transactions
- `include_txs`: Include mempool transactions (false = empty blocks only)
- `continue_on_tx_validation_failure`: Keep forging even if some txs are invalid

## Leader Schedule

Pre-calculate which slots you'll be leader in:

```json
{
  "leader_schedule": {
    "schedule_lookahead_epochs": 2,
    "log_schedule": false,
    "export_schedule_file": "leader-schedule.json",
    "schedule_refresh_interval": 2160
  }
}
```

- `schedule_lookahead_epochs`: Calculate schedule this many epochs ahead
- `log_schedule`: Log your leader schedule (SECURITY: reveals slot leadership)
- `export_schedule_file`: Export schedule to file
- `schedule_refresh_interval`: Recalculate every N slots

## Security Considerations

1. **Cold Key Security**:
   - NEVER store cold key on the block producer node
   - Keep cold key on an air-gapped machine
   - Use cold key only for generating operational certificates

2. **VRF Key Protection**:
   - Keep VRF signing key file permissions restricted: `chmod 400 vrf.skey`
   - VRF key should never leave the block producer node

3. **KES Key Rotation**:
   - Set up monitoring for KES expiration
   - Plan rotation 5-10 periods in advance
   - Test rotation procedure on testnet first

4. **Operational Certificate**:
   - Track issue counter carefully
   - Never reuse old operational certificates
   - Keep counter file in sync

5. **File Permissions**:
   ```bash
   chmod 400 keys/*.skey       # Private keys: read-only for owner
   chmod 444 keys/*.vkey       # Public keys: read-only for all
   chmod 400 keys/node.cert    # Certificate: read-only for owner
   chmod 600 keys/*.counter    # Counter: read-write for owner
   ```

## Monitoring

Key metrics to monitor:

1. **KES Expiration**: Check remaining KES periods daily
2. **Block Production**: Verify blocks are produced when elected leader
3. **Slot Leadership**: Confirm slot leadership calculation matches network
4. **Operational Certificate**: Ensure cert is valid and counter is correct

## Troubleshooting

### "KES key has expired"
- Current KES period exceeds max evolutions
- Rotate KES key immediately

### "VRF signing key file not found"
- Check file path in configuration
- Verify file permissions

### "Invalid operational certificate"
- Certificate may be for wrong KES key
- Issue counter may be incorrect
- KES period in cert may be wrong

### "Not producing blocks"
- Verify pool has sufficient stake
- Check VRF calculations
- Confirm node is synced
- Verify operational certificate is valid

## Testing on Preview Testnet

Before running on mainnet, test on preview testnet:

1. Get test ADA from faucet
2. Register stake pool
3. Configure block producer with testnet keys
4. Monitor block production
5. Test KES rotation procedure

## References

- [Cardano Stake Pool Course](https://cardano-foundation.gitbook.io/stake-pool-course/)
- [Cardano Developer Docs](https://developers.cardano.org/)
- [CIP-0009: Shelley Protocol Parameters](https://cips.cardano.org/cips/cip9/)
- [Cardano CLI Reference](https://github.com/IntersectMBO/cardano-cli)
