#!/usr/bin/env bash
# Test script for connecting to Cardano preview testnet
# This script runs the node with debug logging and monitors the handshake process

set -e

echo "================================================"
echo "Cardano Node - Preview Testnet Connection Test"
echo "================================================"
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if node binary exists
if [ ! -f "target/release/cardano-node" ]; then
    echo -e "${RED}Error: Node binary not found${NC}"
    echo "Building node..."
    cargo build --release --package cardano-node
fi

echo -e "${GREEN}✓${NC} Node binary found"
echo ""

# Check configuration files
if [ ! -f "config/test-config.json" ]; then
    echo -e "${RED}Error: config/test-config.json not found${NC}"
    exit 1
fi
echo -e "${GREEN}✓${NC} Configuration file found"

if [ ! -f "config/preview-topology.json" ]; then
    echo -e "${RED}Error: config/preview-topology.json not found${NC}"
    exit 1
fi
echo -e "${GREEN}✓${NC} Topology file found"

# Test DNS resolution
echo ""
echo "Testing DNS resolution..."
if getent hosts preview-node.world.dev.cardano.org > /dev/null 2>&1; then
    IP=$(getent hosts preview-node.world.dev.cardano.org | head -1 | awk '{print $1}')
    echo -e "${GREEN}✓${NC} DNS resolved: preview-node.world.dev.cardano.org -> $IP"
else
    echo -e "${RED}✗${NC} DNS resolution failed"
    exit 1
fi

# Test TCP connectivity
echo ""
echo "Testing TCP connectivity to port 30002..."
if timeout 5 bash -c 'cat < /dev/null > /dev/tcp/preview-node.world.dev.cardano.org/30002' 2>/dev/null; then
    echo -e "${GREEN}✓${NC} Port 30002 is accessible"
else
    echo -e "${RED}✗${NC} Cannot connect to port 30002"
    exit 1
fi

echo ""
echo "================================================"
echo "Starting Node (Press Ctrl+C to stop)"
echo "================================================"
echo ""
echo "Monitoring for:"
echo "  1. TCP connection establishment"
echo "  2. Handshake initiation"
echo "  3. Version negotiation (V15)"
echo "  4. Network magic validation (1097911063)"
echo "  5. Connection authentication"
echo ""
echo "Log file: node-test.log"
echo ""

# Run the node with debug logging
RUST_LOG=debug,cardano_network::protocols::handshake=trace,cardano_network::connection::manager=trace \
    ./target/release/cardano-node run \
    --config config/test-config.json \
    --topology config/preview-topology.json \
    2>&1 | tee node-test.log | grep --line-buffered -E '(connection|handshake|Establishing|Starting|completed|authenticated|ERROR|WARN)' | while read line; do

    if echo "$line" | grep -qi "error"; then
        echo -e "${RED}$line${NC}"
    elif echo "$line" | grep -qi "warn"; then
        echo -e "${YELLOW}$line${NC}"
    elif echo "$line" | grep -qi "authenticated\|completed successfully"; then
        echo -e "${GREEN}$line${NC}"
    else
        echo "$line"
    fi
done
