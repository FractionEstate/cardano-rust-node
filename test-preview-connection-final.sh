#!/bin/bash
# Quick test script for Cardano Node Rust - Preview Testnet Connection

set -e

echo "=================================="
echo "Cardano Node Rust - Preview Testnet"
echo "=================================="
echo ""

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if binary exists
if [ ! -f "./target/release/cardano-node" ]; then
    echo "${BLUE}Building cardano-node (release mode)...${NC}"
    cargo build --release
    echo ""
fi

# Run tests
echo "${BLUE}Running protocol tests...${NC}"
cargo test --package cardano-network --lib protocols --quiet
echo "${GREEN}✓ All protocol tests passed${NC}"
echo ""

# Show configuration
echo "${BLUE}Configuration:${NC}"
echo "  Network: Preview Testnet"
echo "  Magic: 2"
echo "  Endpoint: preview-node.play.dev.cardano.org:3001"
echo "  Versions: V14, V15"
echo ""

# Run node
echo "${BLUE}Starting Cardano Node...${NC}"
echo "Press Ctrl+C to stop"
echo ""

RUST_LOG=info ./target/release/cardano-node run \
  --config config/test-config.json \
  --topology config/preview-topology-play.json
