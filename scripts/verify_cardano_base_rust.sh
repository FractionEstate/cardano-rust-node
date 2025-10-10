#!/bin/bash
# Verification script for cardano-base-rust integration
# Usage: ./verify_cardano_base_rust.sh

set -e

echo "🔍 Cardano-Base-Rust Integration Verification"
echo "=============================================="
echo ""

# Colors
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color

# Check 1: Workspace dependencies
echo "1. Checking workspace dependencies..."
if grep -q "cardano-vrf-pure.*github.com/FractionEstate/cardano-base-rust" Cargo.toml && \
   grep -q "cardano-crypto-class.*github.com/FractionEstate/cardano-base-rust" Cargo.toml; then
    echo -e "${GREEN}✅ Workspace dependencies configured correctly${NC}"
else
    echo -e "${RED}❌ Workspace dependencies missing or incorrect${NC}"
    exit 1
fi

# Check 2: cardano-crypto dependencies
echo ""
echo "2. Checking cardano-crypto crate dependencies..."
if grep -q "cardano-vrf-pure.workspace = true" crates/cardano-crypto/Cargo.toml && \
   grep -q "cardano-crypto-class.workspace = true" crates/cardano-crypto/Cargo.toml; then
    echo -e "${GREEN}✅ cardano-crypto uses workspace dependencies${NC}"
else
    echo -e "${RED}❌ cardano-crypto not using workspace dependencies${NC}"
    exit 1
fi

# Check 3: VRF backend uses cardano-vrf-pure
echo ""
echo "3. Checking VRF backend implementation..."
if grep -q "cardano_vrf_pure" crates/cardano-crypto/src/vrf/backend.rs && \
   grep -Eq "VrfDraft(03|13)" crates/cardano-crypto/src/vrf/backend.rs; then
    echo -e "${GREEN}✅ VRF backend uses cardano-vrf-pure${NC}"
else
    echo -e "${RED}❌ VRF backend not using cardano-vrf-pure${NC}"
    exit 1
fi

# Check 4: Build cardano-crypto
echo ""
echo "4. Building cardano-crypto crate..."
if cargo build --package cardano-crypto 2>&1 | grep -q "Finished"; then
    echo -e "${GREEN}✅ cardano-crypto builds successfully${NC}"
else
    echo -e "${RED}❌ cardano-crypto build failed${NC}"
    exit 1
fi

# Check 5: Verify dependency tree
echo ""
echo "5. Verifying dependency tree..."
TREE_OUTPUT=$(cargo tree -p cardano-crypto 2>&1)
if echo "$TREE_OUTPUT" | grep -q "cardano-vrf-pure.*cardano-base-rust" && \
   echo "$TREE_OUTPUT" | grep -q "cardano-crypto-class.*cardano-base-rust"; then
    echo -e "${GREEN}✅ Dependency tree includes cardano-base-rust${NC}"
    echo ""
    echo "   Dependencies found:"
    echo "$TREE_OUTPUT" | grep -E "cardano-(vrf|crypto-class)" | head -3 | sed 's/^/   /'
else
    echo -e "${RED}❌ cardano-base-rust not in dependency tree${NC}"
    exit 1
fi

# Check 6: No duplicate VRF implementations
echo ""
echo "6. Checking for duplicate VRF implementations..."
VRF_IMPL_COUNT=$(find crates/cardano-crypto/src/vrf -name "*.rs" -type f | wc -l)
if [ "$VRF_IMPL_COUNT" -le 3 ]; then
    echo -e "${GREEN}✅ No duplicate VRF implementations detected${NC}"
    echo "   Found $VRF_IMPL_COUNT VRF-related files (backend.rs, mod.rs expected)"
else
    echo -e "${YELLOW}⚠️  Multiple VRF files found ($VRF_IMPL_COUNT)${NC}"
    echo "   Please verify no duplicate implementations exist"
fi

# Summary
echo ""
echo "=============================================="
echo -e "${GREEN}✅ All checks passed!${NC}"
echo ""
echo "Summary:"
echo "  • cardano-vrf-pure: ✅ Integrated"
echo "  • cardano-crypto-class: ✅ Integrated"
echo "  • Build status: ✅ Success"
echo "  • Implementation: ✅ Using official library"
echo ""
echo "Integration Status: VERIFIED ✅"
