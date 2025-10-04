#!/bin/bash
# Quick test for block producer configuration validation

set -e

echo "==================================="
echo "Block Producer Configuration Test"
echo "==================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Test directory
TEST_DIR="/tmp/cardano-bp-test-$$"
mkdir -p "$TEST_DIR/keys"
cd "$TEST_DIR"

echo "Test directory: $TEST_DIR"
echo ""

# Generate test VRF key (minimal valid structure)
echo "${YELLOW}[1/4] Creating test VRF key...${NC}"
cat > keys/vrf.skey << 'EOF'
{
  "type": "VrfSigningKey_PraosVRF",
  "description": "VRF Signing Key",
  "cborHex": "5820000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f"
}
EOF
echo "${GREEN}✓ VRF key created${NC}"

# Generate test KES key (minimal valid structure)
echo "${YELLOW}[2/4] Creating test KES key...${NC}"
cat > keys/kes.skey << 'EOF'
{
  "type": "KesSigningKey_6_6",
  "description": "KES Signing Key",
  "cborHex": "582000112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
}
EOF
echo "${GREEN}✓ KES key created${NC}"

# Generate test operational certificate (minimal valid structure)
echo "${YELLOW}[3/4] Creating test operational certificate...${NC}"
cat > keys/node.cert << 'EOF'
{
  "type": "NodeOperationalCertificate",
  "description": "Operational Certificate",
  "cborHex": "828400582000112233445566778899aabbccddeeff00112233445566778899aabbccddeeff00582040506070809000000000000000000000000000000000000000000000000000005840ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"
}
EOF
echo "${GREEN}✓ Operational certificate created${NC}"

# Create minimal configuration
echo "${YELLOW}[4/4] Creating block producer configuration...${NC}"
cat > config.json << EOF
{
  "block_producer": {
    "enabled": true,
    "vrf_key": {
      "signing_key_file": "$TEST_DIR/keys/vrf.skey",
      "format": "cardano-cli"
    },
    "kes_key": {
      "signing_key_file": "$TEST_DIR/keys/kes.skey",
      "kes_period": 0,
      "max_kes_evolutions": 62,
      "start_kes_period": 0,
      "format": "cardano-cli"
    },
    "operational_cert": {
      "cert_file": "$TEST_DIR/keys/node.cert",
      "issue_counter": 0
    },
    "forging_behavior": {
      "forging_delay_ms": 100,
      "max_txs_per_block": 10000,
      "max_block_size_bytes": 90112,
      "prefer_high_fees": true,
      "include_txs": true,
      "continue_on_tx_validation_failure": true
    },
    "leader_schedule": {
      "schedule_lookahead_epochs": 2,
      "log_schedule": false,
      "schedule_refresh_interval": 2160
    }
  }
}
EOF
echo "${GREEN}✓ Configuration created${NC}"

echo ""
echo "${GREEN}==================================="
echo "Test Setup Complete!"
echo "===================================${NC}"
echo ""
echo "Test artifacts created:"
echo "  - Config: $TEST_DIR/config.json"
echo "  - VRF key: $TEST_DIR/keys/vrf.skey"
echo "  - KES key: $TEST_DIR/keys/kes.skey"
echo "  - Op cert: $TEST_DIR/keys/node.cert"
echo ""
echo "Configuration file contents:"
echo "${YELLOW}---${NC}"
cat config.json
echo "${YELLOW}---${NC}"
echo ""
echo "${GREEN}✓ All files created successfully${NC}"
echo ""
echo "Note: These are TEST keys only - DO NOT use in production!"
echo ""
echo "To test with actual cardano-cli keys:"
echo "  1. Generate keys with cardano-cli"
echo "  2. Update paths in the configuration"
echo "  3. Run: cardano-node run --config config.json"
echo ""

# Keep test directory for inspection
echo "Test directory preserved at: $TEST_DIR"
echo "Clean up with: rm -rf $TEST_DIR"
