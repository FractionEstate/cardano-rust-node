# CI/CD Configuration - Comprehensive Guide

## Overview

The Cardano Node Rust project uses GitHub Actions for continuous integration and deployment with a comprehensive set of checks to ensure code quality, security, and compatibility.

---

## 🔄 CI/CD Pipeline

### Triggers
- **Push**: `main`, `develop`, `001-*` branches
- **Pull Request**: `main`, `develop` branches
- **Schedule**: Weekly security audits (Mondays at 00:00 UTC)

### Environment
```yaml
CARGO_TERM_COLOR: always
RUST_BACKTRACE: 1
RUSTFLAGS: -D warnings
```

---

## ✅ CI Jobs

### 1. Test Suite (`test`)
**Purpose**: Run all tests across multiple platforms and Rust versions

**Matrix**:
- OS: Ubuntu, Windows, macOS
- Rust: stable, 1.75.0

**Steps**:
1. Checkout code
2. Install Rust toolchain
3. Cache cargo registry, index, and build
4. Check build (`cargo check`)
5. Run unit tests (`cargo test --workspace`)
6. Run doc tests

**Caching Strategy**:
- Registry: `~/.cargo/registry`
- Index: `~/.cargo/git`
- Build: `target/`
- Keys include OS and Rust version for isolation

**Result**: **330 tests passing** ✅

---

### 2. Code Formatting (`fmt`)
**Purpose**: Ensure consistent code style

**Tool**: `rustfmt`

**Command**:
```bash
cargo fmt --all -- --check
```

**Enforcement**: Fails on formatting violations

---

### 3. Linting (`clippy`)
**Purpose**: Catch common mistakes and enforce best practices

**Tool**: `clippy`

**Command**:
```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::all
```

**Strictness**: All warnings treated as errors

---

### 4. Documentation (`docs`)
**Purpose**: Verify documentation builds correctly

**Steps**:
1. Build docs with `cargo doc`
2. Fail on documentation warnings
3. Upload docs artifact for main branch

**Command**:
```bash
cargo doc --workspace --no-deps --all-features
```

**Environment**:
```yaml
RUSTDOCFLAGS: -D warnings
```

---

### 5. Dependency Check (`dependency-check`)
**Purpose**: Verify dependencies for security and licensing

**Tool**: `cargo-deny`

**Configuration**: `deny.toml`

**Checks**:
- ✅ Security vulnerabilities (deny)
- ✅ Yanked crates (deny)
- ✅ Unmaintained crates (warn)
- ✅ License compliance
- ✅ Multiple versions of same crate (warn)
- ✅ Unknown sources (deny)

**Allowed Licenses**:
- MIT, Apache-2.0, BSD-2/3-Clause
- ISC, CC0-1.0, MPL-2.0
- Unlicense, Zlib

**Denied Licenses**:
- GPL-*, AGPL-*

**Allowed Git Sources**:
- https://github.com/FractionEstate/cardano-base-rust

---

### 6. VRF Migration Verification (`vrf-migration-check`) 🎉
**Purpose**: Verify successful migration to cardano-base-rust

**Checks**:
1. ✅ No local `curve25519-dalek/` fork
2. ✅ No `[patch.crates-io]` section in Cargo.toml
3. ✅ `cardano-vrf-pure` dependency present
4. ✅ `cardano-crypto-class` dependency present

**Impact**: Ensures clean migration from private APIs to official library

---

### 7. Haskell Compatibility (`haskell-compat`)
**Purpose**: Verify compatibility with Haskell cardano-node

**Steps**:
1. Download mainnet configuration files
2. Run compatibility tests

**Configuration Sources**:
- https://book.world.dev.cardano.org/environments/mainnet/config.json
- https://book.world.dev.cardano.org/environments/mainnet/topology.json

---

### 8. Security Audit (`security-audit`)
**Purpose**: Identify security vulnerabilities in dependencies

**Tool**: `cargo-audit`

**Command**:
```bash
cargo audit --deny warnings
```

**Database**: RustSec Advisory Database

**Frequency**:
- Every PR/push
- Weekly scheduled run

---

### 9. Code Coverage (`coverage`)
**Purpose**: Track test coverage

**Tool**: `cargo-tarpaulin`

**Command**:
```bash
cargo tarpaulin --workspace --out Xml --output-dir coverage --all-features
```

**Upload**: Codecov.io

**Enforcement**: Non-blocking (informational)

---

### 10. Release Build (`build-release`)
**Purpose**: Verify release builds work on all platforms

**Matrix**:
- Ubuntu, Windows, macOS

**Command**:
```bash
cargo build --release --package cardano-node
```

**Artifacts**: Upload binaries for each platform

---

### 11. Status Check (`status-check`)
**Purpose**: Final validation of all checks

**Dependencies**: All previous jobs

**Logic**: Fails if any required job fails

**Required Jobs**:
- test
- fmt
- clippy
- docs
- dependency-check
- vrf-migration-check
- security-audit

---

## 🔒 Security Features

### 1. Dependency Security
- ✅ Automated vulnerability scanning
- ✅ RustSec Advisory Database
- ✅ Weekly scheduled audits
- ✅ Deny warnings mode

### 2. License Compliance
- ✅ Automated license checking
- ✅ GPL/AGPL denied
- ✅ OSI/FSF free software licenses allowed

### 3. Source Verification
- ✅ Only trusted registries allowed
- ✅ Git sources explicitly whitelisted
- ✅ FractionEstate GitHub org trusted

### 4. Migration Verification
- ✅ No local forks allowed
- ✅ No private API patches
- ✅ Official libraries enforced

---

## 📊 Performance Optimizations

### Caching Strategy
```yaml
Cache Keys:
  - Registry: $OS-$RUST-cargo-registry-$LOCKFILE_HASH
  - Index: $OS-$RUST-cargo-git-$LOCKFILE_HASH
  - Build: $OS-$RUST-cargo-build-$LOCKFILE_HASH

Restore Keys:
  - Fallback to latest cache for same OS/Rust combination
```

### Benefits
- ⚡ Faster CI runs (cache hits)
- 💾 Reduced bandwidth usage
- 🔄 Incremental builds

### Matrix Strategy
- `fail-fast: false` - Continue testing other combinations on failure
- Parallel execution across OS and Rust versions

---

## 🔧 Local Development

### Run All Checks Locally

```bash
# Format check
cargo fmt --all -- --check

# Linting
cargo clippy --workspace --all-targets --all-features -- -D warnings

# Tests
cargo test --workspace --all-features

# Documentation
cargo doc --workspace --no-deps --all-features

# Security audit
cargo install cargo-audit
cargo audit

# Dependency check
cargo install cargo-deny
cargo deny check

# Coverage (optional)
cargo install cargo-tarpaulin
cargo tarpaulin --workspace --all-features
```

### Pre-commit Hook
Create `.git/hooks/pre-commit`:
```bash
#!/bin/bash
set -e

echo "Running pre-commit checks..."

# Format check
cargo fmt --all -- --check

# Quick test
cargo test --workspace

echo "✅ Pre-commit checks passed"
```

Make it executable:
```bash
chmod +x .git/hooks/pre-commit
```

---

## 📈 Continuous Improvement

### Current Status
- ✅ 330 tests passing
- ✅ Zero warnings
- ✅ Zero security vulnerabilities
- ✅ 100% official dependencies
- ✅ Clean migration verified

### Future Enhancements
1. **Performance Benchmarking**: Add criterion benchmarks to CI
2. **Integration Tests**: Extended Haskell compatibility tests
3. **Docker Builds**: Automated container builds
4. **Release Automation**: Automated GitHub releases
5. **Nightly Builds**: Test against Rust nightly

---

## 🚀 Release Process

### Automated Steps
1. All CI checks pass
2. Release binaries built for all platforms
3. Artifacts uploaded to GitHub

### Manual Steps (to be automated)
1. Create GitHub release
2. Tag version
3. Publish to crates.io (if applicable)
4. Update documentation
5. Announce release

---

## 📝 Troubleshooting

### Common Issues

**Cache Invalidation**
```bash
# Clear all caches in GitHub Actions:
# Settings → Actions → Caches → Delete
```

**Flaky Tests**
```yaml
# Add retries to flaky tests
strategy:
  fail-fast: false
```

**Dependency Conflicts**
```bash
# Update all dependencies
cargo update

# Check for issues
cargo tree --duplicates
```

**Security Audit Failures**
```bash
# Check specific advisory
cargo audit --deny warnings --ignore RUSTSEC-XXXX-XXXX
```

---

## 🎯 Best Practices

### For Contributors
1. ✅ Run `cargo fmt` before committing
2. ✅ Run `cargo clippy` to catch issues early
3. ✅ Run `cargo test` to verify tests pass
4. ✅ Keep dependencies up to date
5. ✅ Write tests for new features

### For Maintainers
1. ✅ Review security audit results weekly
2. ✅ Update dependencies regularly
3. ✅ Monitor CI performance
4. ✅ Keep documentation current
5. ✅ Enforce CI checks on all PRs

---

## 📚 References

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny)
- [cargo-audit](https://github.com/RustSec/rustsec/tree/main/cargo-audit)
- [cargo-tarpaulin](https://github.com/xd009642/tarpaulin)
- [RustSec Advisory Database](https://rustsec.org/)

---

**Last Updated**: January 10, 2025
**Status**: ✅ Production Ready
