#!/usr/bin/env bash
set -euo pipefail

PROJECT_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$PROJECT_ROOT"

# Ensure the workspace is using locked dependencies for reproducibility
cargo build --locked --release -p cardano-node

HOST_TRIPLE=$(rustc -vV | grep ^host: | awk '{print $2}')
VERSION=$(cargo pkgid -p cardano-node | awk '{print $2}')

DIST_DIR="$PROJECT_ROOT/dist"
PKG_DIR="$DIST_DIR/cardano-node-$VERSION-$HOST_TRIPLE"
BIN_DIR="$PKG_DIR/bin"
CONFIG_DIR="$PKG_DIR/configuration"
DOC_DIR="$PKG_DIR/docs"

rm -rf "$PKG_DIR"
mkdir -p "$BIN_DIR" "$CONFIG_DIR" "$DOC_DIR"

# Copy the compiled binary and default configurations
cp "$PROJECT_ROOT/target/release/cardano-node" "$BIN_DIR/"
rsync -a "$PROJECT_ROOT/configuration/" "$CONFIG_DIR/"

# Ship essential docs for operators
rsync -a "$PROJECT_ROOT/docs/deployment.md" "$DOC_DIR/" 2>/dev/null || true
rsync -a "$PROJECT_ROOT/docs/configuration.md" "$DOC_DIR/" 2>/dev/null || true
rsync -a "$PROJECT_ROOT/docs/quickstart.md" "$DOC_DIR/" 2>/dev/null || true

# Generate checksums for integrity verification
pushd "$PKG_DIR" >/dev/null
find . -type f ! -name "SHA256SUMS" -print0 | sort -z | xargs -0 sha256sum > SHA256SUMS
popd >/dev/null

# Create versioned tarball
mkdir -p "$DIST_DIR"
TARBALL="$DIST_DIR/cardano-node-$VERSION-$HOST_TRIPLE.tar.gz"
rm -f "$TARBALL"

tar -C "$DIST_DIR" -czf "$TARBALL" "cardano-node-$VERSION-$HOST_TRIPLE"

echo "Created distribution package: $TARBALL"
