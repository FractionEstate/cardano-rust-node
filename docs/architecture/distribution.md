# Cardano Node Rust Distribution Guide

This document captures the steps we follow to produce a distributable build of the Rust Cardano node. The workflow assumes you are working inside the repository root (`cardano-node-rust/`) on a Linux host, but the commands are portable to any platform with the required toolchain installed.

## 1. Prerequisites

- **Rust toolchain**: Use the workspace toolchain specified in `rust-toolchain.toml` (currently Rust 1.75 or newer).
- **Cargo**: Ships with Rust; verify with `cargo --version`.
- **Build dependencies**: The default Dev Container already contains the required native packages. On bare-metal Linux ensure `clang`, `llvm`, and `pkg-config` are available.
- **Optional**: `gzip`/`tar` for packaging artifacts into an archive.

## 2. Clean workspace (optional but recommended)

This ensures no stale artifacts leak into the release build.

```bash
cargo clean
```text

## 3. Build the release artifacts

Compile every crate in the workspace with optimizations. This produces the `cardano-node` binary alongside supporting libraries under `target/release/`.

```bash
cargo build --workspace --release
```text

Key outputs:
- `target/release/cardano-node`
- Supporting dynamic libraries (if your platform emits them)

## 4. Run the verification suite

Execute the full test matrix to confirm the release build matches the Haskell reference behaviour.

```bash
cargo test --workspace
```text

For quicker spot checks during iterative work, you can target individual packages:

```bash
cargo test --package cardano-ledger
cargo test --package cardano-consensus
```text

## 5. Bundle configuration and metadata

1. Create a staging directory for the release bundle:
   ```bash
   mkdir -p dist/cardano-node
   ```
2. Copy the compiled binary:
   ```bash
   cp target/release/cardano-node dist/cardano-node/
   ```
3. Include runtime configuration, topology, and documentation as needed. Example:
   ```bash
   cp -r configuration dist/cardano-node/configuration
   cp README.md dist/cardano-node/  # if present
   ```
4. Capture the current git revision for traceability:
   ```bash
   git rev-parse HEAD > dist/cardano-node/VERSION
   ```

## 6. Package the release bundle

Create a compressed archive that can be distributed to operators.

```bash
tar -C dist -czf cardano-node-rust.tar.gz cardano-node
```text

You may also produce platform-specific archives, e.g. `cardano-node-rust-x86_64-unknown-linux-gnu.tar.gz` to clarify the target triple.

## 7. (Optional) Generate checksums and signatures

For public releases, generate SHA256 digests and sign them:

```bash
shasum -a 256 cardano-node-rust.tar.gz > cardano-node-rust.tar.gz.sha256
gpg --armor --detach-sign cardano-node-rust.tar.gz
```text

Share the checksum and signature alongside the archive so consumers can verify integrity and authenticity.

## 8. Smoke-test the packaged binary

Before publishing, perform a quick runtime sanity check using the artefact from the tarball:

```bash
mkdir -p dist/tmp
cp cardano-node-rust.tar.gz dist/tmp/
cd dist/tmp
 tar -xzf cardano-node-rust.tar.gz
./cardano-node/cardano-node --help
```text

If the help text prints without errors, the binary is correctly linked and executable in a clean environment.

## 9. Cleanup

Remove intermediate staging directories once the release is published:

```bash
rm -rf dist/tmp
```text

Keep the `dist/cardano-node` directory if you plan further validation or operator-specific packaging.

---

Following the steps above results in a reproducible, fully-tested release bundle that mirrors the expectations of the upstream Haskell Cardano node distribution process.
