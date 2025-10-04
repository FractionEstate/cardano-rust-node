# Cardano-Base-Rust Integration Checklist

Use this checklist for periodic audits or when updating dependencies.

## Quick Verification Checklist

### 1. Dependencies Configuration ✅
- [ ] `Cargo.toml` (root) has cardano-vrf-pure workspace dependency
- [ ] `Cargo.toml` (root) has cardano-crypto-class workspace dependency
- [ ] Both dependencies point to https://github.com/FractionEstate/cardano-base-rust
- [ ] `crates/cardano-crypto/Cargo.toml` uses `.workspace = true` for both

### 2. VRF Implementation ✅
- [ ] `crates/cardano-crypto/src/vrf/backend.rs` imports `cardano_vrf_pure::draft03::VrfDraft03`
- [ ] VRF prove function uses `VrfDraft03::prove()`
- [ ] VRF verify function uses `VrfDraft03::verify()`
- [ ] VRF proof_to_hash uses `VrfDraft03::proof_to_hash()`
- [ ] No custom VRF implementations present

### 3. Build Verification ✅
- [ ] `cargo build --package cardano-crypto` succeeds
- [ ] `cargo test -p cardano-crypto --lib vrf` passes
- [ ] No compilation warnings about unused cardano-base-rust imports

### 4. Dependency Tree ✅
- [ ] `cargo tree -p cardano-crypto | grep cardano-vrf-pure` shows cardano-base-rust source
- [ ] `cargo tree -p cardano-crypto | grep cardano-crypto-class` shows cardano-base-rust source
- [ ] No duplicate VRF implementations in dependency tree

### 5. Code Quality ✅
- [ ] No `use ed25519_dalek` in VRF code (should use cardano-vrf-pure)
- [ ] No custom curve25519 implementations in VRF code
- [ ] Secret keys are properly zeroized
- [ ] Error handling uses proper Result types

## Automated Verification

Run the automated verification script:

```bash
./verify_cardano_base_rust.sh
```

Expected result: All checks pass ✅

## Manual Verification Commands

### Check Workspace Dependencies
```bash
grep -A2 "cardano-vrf-pure" Cargo.toml
grep -A2 "cardano-crypto-class" Cargo.toml
```

Expected:
```toml
cardano-vrf-pure = { git = "https://github.com/FractionEstate/cardano-base-rust", branch = "master" }
cardano-crypto-class = { git = "https://github.com/FractionEstate/cardano-base-rust", branch = "master" }
```

### Check VRF Backend Implementation
```bash
grep "use cardano_vrf_pure" crates/cardano-crypto/src/vrf/backend.rs
```

Expected:
```rust
use cardano_vrf_pure::draft03::VrfDraft03;
```

### Check Dependency Tree
```bash
cargo tree -p cardano-crypto | grep -E "cardano-(vrf|crypto-class)" | head -5
```

Expected output includes cardano-base-rust git references.

### Build Test
```bash
cargo clean -p cardano-crypto
cargo build --package cardano-crypto 2>&1 | grep -E "(Compiling cardano-(vrf|crypto)|Finished)"
```

Expected: Compiles cardano-vrf-pure and cardano-crypto-class from cardano-base-rust.

## Update Procedure

When updating cardano-base-rust:

1. **Update dependencies**
   ```bash
   cargo update -p cardano-vrf-pure
   cargo update -p cardano-crypto-class
   ```

2. **Check for breaking changes**
   ```bash
   cd /tmp
   git clone https://github.com/FractionEstate/cardano-base-rust
   cd cardano-base-rust
   git log --oneline | head -20
   cat CHANGELOG.md  # if available
   ```

3. **Test the update**
   ```bash
   cargo test -p cardano-crypto
   cargo test -p cardano-consensus
   ./verify_cardano_base_rust.sh
   ```

4. **Update documentation**
   - Update version/commit in CARDANO_BASE_RUST_AUDIT.md
   - Note any API changes
   - Update this checklist if needed

## Red Flags 🚩

Watch out for these issues:

- 🚩 Custom VRF implementation code in crates/cardano-crypto/src/vrf/
- 🚩 Direct use of curve25519-dalek in VRF code (should be via cardano-vrf-pure)
- 🚩 Missing cardano-base-rust in `cargo tree` output
- 🚩 VRF functions not using VrfDraft03::* methods
- 🚩 Build warnings about unused cardano-base-rust dependencies
- 🚩 Duplicate VRF implementations

## Success Criteria ✅

All of the following must be true:

1. ✅ cardano-vrf-pure appears in dependency tree from cardano-base-rust
2. ✅ VRF backend.rs imports and uses VrfDraft03
3. ✅ cargo build succeeds for cardano-crypto
4. ✅ No duplicate VRF implementations
5. ✅ verify_cardano_base_rust.sh passes all checks

## Troubleshooting

### Issue: cardano-base-rust not in dependency tree

**Solution:**
```bash
cargo clean
cargo update -p cardano-vrf-pure -p cardano-crypto-class
cargo build --package cardano-crypto
```

### Issue: Build fails with cardano-base-rust errors

**Solution:**
1. Check cardano-base-rust repository status
2. Verify branch is still "master"
3. Try pinning to a known-good commit:
   ```toml
   cardano-vrf-pure = { git = "https://github.com/FractionEstate/cardano-base-rust", rev = "fdd8a884" }
   ```

### Issue: VRF tests fail

**Solution:**
1. Verify VrfDraft03 API hasn't changed
2. Check test vectors match IETF draft-03 spec
3. Ensure no breaking changes in cardano-vrf-pure

## Audit History

| Date | Auditor | Status | Notes |
|------|---------|--------|-------|
| 2025-10-04 | GitHub Copilot | ✅ PASSED | Initial integration audit. All checks passed. |

## Next Audit

**Recommended Frequency**: Quarterly or after major dependency updates

**Next Audit Date**: January 2026

**Trigger Events for Immediate Re-audit**:
- Major cardano-base-rust version update
- Cardano protocol changes affecting VRF
- Security advisories related to VRF
- Changes to crates/cardano-crypto/src/vrf/

---

Last Updated: October 4, 2025
Audit Status: ✅ PASSED
