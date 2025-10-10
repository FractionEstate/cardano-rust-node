# Cardano Node (Rust)

[![CI](https://github.com/cardano-rust/cardano-node-rust/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/cardano-rust/cardano-node-rust/actions/workflows/ci.yml)
[![Integration Tests](https://github.com/cardano-rust/cardano-node-rust/actions/workflows/integration-tests.yml/badge.svg?branch=main)](https://github.com/cardano-rust/cardano-node-rust/actions/workflows/integration-tests.yml)
[![Docs](https://github.com/cardano-rust/cardano-node-rust/actions/workflows/docs.yml/badge.svg?branch=main)](https://github.com/cardano-rust/cardano-node-rust/actions/workflows/docs.yml)
[![Release](https://github.com/cardano-rust/cardano-node-rust/actions/workflows/release.yml/badge.svg)](https://github.com/cardano-rust/cardano-node-rust/actions/workflows/release.yml)
[![Security](https://github.com/cardano-rust/cardano-node-rust/actions/workflows/security.yml/badge.svg?branch=main)](https://github.com/cardano-rust/cardano-node-rust/actions/workflows/security.yml)

The Cardano Rust node is a full proof-of-stake blockchain node engineered around the Ouroboros family of consensus protocols. This workspace brings the ledger, network, storage, tracing, and CLI layers together in idiomatic Rust while maintaining compatibility with the broader Cardano ecosystem.

## Highlights

- ✨ **Full multi-crate workspace** covering consensus, ledger, networking, storage, tracing, API, node runtime, and testnet utilities.
- 🔁 **End-to-end automation** with build, test, security, documentation, and release pipelines that mirror production expectations.
- 🧪 **Comprehensive testing** including unit, doc, property, integration, performance, and nightly mainnet sync coverage.
- 📦 **First-class packaging** via Docker images, multi-target binaries, and staged crates.io publications.
- 🧱 **Modular architecture** designed for reuse and community contributions across Cardano services.

## Repository layout

| Path | Purpose |
|------|---------|
| `crates/cardano-node` | CLI entrypoint, runtime wiring, dashboards, and administrative commands. |
| `crates/cardano-consensus` | Ouroboros consensus algorithms and block validation logic. |
| `crates/cardano-ledger` | Ledger rules, UTxO accounting, certificates, and governance primitives. |
| `crates/cardano-network` | Multiplexed network stack, handshake negotiation, and peer selection. |
| `crates/cardano-storage` | Ledger database, LMDB backend, and snapshot handling. |
| `crates/cardano-crypto` | Cryptographic primitives, VRFs, and signature utilities. |
| `crates/cardano-tracing` | Structured logging, telemetry sinks, and observability helpers. |
| `crates/cardano-api` | Client-facing gRPC/REST adapters and wallet bridges. |
| `crates/cardano-testnet` | Local testnet orchestration helpers and fixtures. |
| `config/` | Example configuration and topology files for running preview/test environments. |
| `scripts/` | Developer ergonomics: readiness checks, preview connectivity, and release helpers. |
| `docs/` | Source for the GitHub Pages developer documentation site (Jekyll). |
| `tests/` | Cross-crate integration tests, network scenarios, and regression suites. |

## Getting started

### Prerequisites

- Rust toolchain `1.80` or newer (install via [`rustup`](https://rustup.rs)).
- `cargo`, `rustfmt`, and `clippy` components (installed through `rustup component add`).
- Optional: Docker (for container builds) and Ruby 3.1 with Bundler (for docs site work).

### Bootstrap the workspace

1. Clone the repository and enter the workspace:
   - `git clone https://github.com/cardano-rust/cardano-node-rust.git`
   - `cd cardano-node-rust`
2. Ensure the recommended toolchain is active: `rustup toolchain install 1.80 && rustup default 1.80`.
3. Build the full workspace: `cargo build --workspace --all-features`.

### Run a local node

The CLI exposes several commands (see `cardano-node --help`). A minimal preview run looks like:

- `cargo run --release --bin cardano-node -- run \
    --config config/test-config.json \
    --topology config/test-topology.json \
    --database-path ./tmp/db \
    --socket-path ./tmp/node.socket`

You can validate configuration files before running using `cardano-node validate --config path/to/config.json`.

## Development workflow

### Formatting and linting

- `cargo fmt --all` – format the entire workspace (CI enforces `--check`).
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` – static analysis matching CI.
- `cargo deny check` – dependency policy checks (matches CI dependency-check).

### Testing

- `cargo test --workspace --all-features` – unit and doc tests across every crate.
- `cargo test --doc --workspace` – doctests only (mirrors CI `docs` job).
- Property tests live under each crate’s `tests/` or `proptest` modules and can be run with `cargo test -p <crate> proptest`.

### Integration & system tests

- `cargo test --test integration mainnet_sync -- --ignored --nocapture`
- `cargo test --test integration chain_reorg -- --ignored --nocapture`
- `cargo test --test integration snapshot_recovery -- --ignored --nocapture`
- `cargo test --test network_integration -- --ignored --nocapture` (network handshake flows)

The integration suites are marked `ignored` to avoid long runtimes; remove `--ignored` to skip heavy scenarios locally. Use the helper scripts in `scripts/` (e.g., `verify_readiness.sh`, `test-preview-connection.sh`) for higher-level environment validation.

### Benchmarks & coverage

- Benchmarks: `cargo bench -p cardano-storage` (adjust crate as needed).
- Coverage: `cargo tarpaulin --workspace --out Html --all-features` (mirrors CI coverage job).

## Documentation

The `docs/` directory houses the GitHub Pages developer portal. To build locally:

1. Install Ruby 3.1 and Bundler.
2. `cd docs`
3. `bundle install`
4. `bundle exec jekyll serve --livereload`

The Docs workflow automatically builds and deploys the site whenever `docs/` changes land on the deployment branch.

Key narrative documents currently live in:

- `docs/architecture/HASKELL_COMPATIBILITY_GAPS.md`
- `docs/guides/BLOCK_PRODUCTION_RUNTIME_EXAMPLE.md`

## Automation quick reference

| Workflow | Trigger | What it verifies |
|----------|---------|------------------|
| **CI** (`ci.yml`) | PRs, main/develop pushes, weekly cron | Formatting, lint, tests, docs, dependency hygiene, coverage, release builds. |
| **Integration Tests** (`integration-tests.yml`) | PRs, main/develop pushes, nightly cron | End-to-end sync, reorg, snapshot, network, and performance scenarios. |
| **Deploy Jekyll Documentation** (`docs.yml`) | Docs branch changes, manual | Builds and publishes the GitHub Pages documentation site. |
| **Release** (`release.yml`) | Semver tags (`v*`) | Creates GitHub Releases, publishes binaries, Docker images, and crates.io packages. |
| **Security Check** (`security-check.yml`) | PRs, main pushes | Strict clippy security lints, panic/unwrap scans, cargo-audit, unsafe usage review. |
| **Security** (`security.yml`) | Daily cron, main pushes/PRs | Automated cargo-audit, cargo-deny, dependency review, and outdated dependency reports. |

## Release process

1. Bump crate versions and workspace metadata as needed.
2. Push a tag following the `vMAJOR.MINOR.PATCH` convention.
3. The Release workflow will:
   - Create a GitHub Release with notes.
   - Build and attach binaries for Linux (x86_64/aarch64), Windows, and macOS (x86_64/aarch64).
   - Publish Docker images to DockerHub and GHCR.
   - Publish all crates to crates.io in dependency order.

## Useful scripts

- `scripts/verify_readiness.sh` – run a comprehensive workspace health check before opening PRs.
- `scripts/test-preview-connection.sh` – exercise preview topology connectivity with mock peers.
- `scripts/build-release.sh` – local helper mirroring the release job targets.

## Contributing & support

We welcome issues and pull requests. Before submitting changes:

- Run the readiness script or at least `cargo fmt`, `cargo clippy -- -D warnings`, and `cargo test --workspace`.
- Update or add documentation whenever behavior or interfaces change.
- Follow the Cardano Rust coding guidelines (idiomatic async Rust, structured tracing, error-first design).

Security-related reports should be sent privately following the guidance in the repository’s security policy (or via the Cardano security disclosure process if no file is present).

## License

Licensed under the [Apache License, Version 2.0](./LICENSE).
