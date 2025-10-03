# Security Policy

## Supported Versions

We release patches for security vulnerabilities in the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 8.7.x   | :white_check_mark: |
| < 8.7   | :x:                |

## Reporting a Vulnerability

The Cardano Node Rust team takes security bugs seriously. We appreciate your efforts to responsibly disclose your findings.

### How to Report a Security Vulnerability

**Please do NOT report security vulnerabilities through public GitHub issues.**

Instead, please report them via one of the following methods:

#### Option 1: GitHub Security Advisories (Recommended)

1. Go to the [Security tab](https://github.com/cardano-rust/cardano-node-rust/security)
2. Click "Report a vulnerability"
3. Fill out the vulnerability report form

#### Option 2: Email

Send an email to: **security@cardano-rust.org**

Please include the following information:

* Type of issue (e.g., buffer overflow, SQL injection, cross-site scripting, etc.)
* Full paths of source file(s) related to the issue
* Location of the affected source code (tag/branch/commit or direct URL)
* Any special configuration required to reproduce the issue
* Step-by-step instructions to reproduce the issue
* Proof-of-concept or exploit code (if possible)
* Impact of the issue, including how an attacker might exploit it

### What to Expect

* **Acknowledgment**: We will acknowledge receipt of your vulnerability report within 48 hours
* **Initial Assessment**: We will provide an initial assessment within 5 business days
* **Updates**: We will keep you informed of the progress toward fixing the vulnerability
* **Disclosure Timeline**: We aim to patch critical vulnerabilities within 30 days
* **Credit**: We will credit you in the release notes (unless you prefer to remain anonymous)

## Security Considerations

### Critical Components

The following components have the highest security requirements:

#### 1. Cryptographic Operations (`cardano-crypto`)
- **VRF (Verifiable Random Function)**: Leader election
- **KES (Key Evolving Signature)**: Block signing
- **Ed25519**: Transaction signing
- **Blake2b/SHA256**: Hashing operations
- **BLS12-381**: Aggregated signatures

**Security Requirements**:
- Constant-time operations to prevent timing attacks
- Secure random number generation
- Proper key management and zeroization
- Side-channel attack resistance

#### 2. Consensus Protocol (`cardano-consensus`)
- **Block validation**: Prevents invalid blocks
- **Chain selection**: Ensures longest valid chain
- **Slot leadership**: Validates block production rights
- **Genesis validation**: Verifies initial state

**Security Requirements**:
- Byzantine fault tolerance
- Protection against long-range attacks
- Eclipse attack prevention
- Denial-of-service mitigation

#### 3. Network Layer (`cardano-network`)
- **P2P communication**: Peer discovery and management
- **Message validation**: Prevents malicious messages
- **DDoS protection**: Rate limiting and filtering
- **TLS/encryption**: Secure communication

**Security Requirements**:
- Sybil attack resistance
- Eclipse attack prevention
- Message authentication
- Connection encryption

#### 4. Storage Layer (`cardano-storage`)
- **Database integrity**: Prevents corruption
- **Transaction atomicity**: Ensures consistency
- **Backup and recovery**: Data durability

**Security Requirements**:
- ACID compliance
- Corruption detection
- Access control

### Known Security Considerations

#### Cryptographic Dependencies
- Ed25519: Using `ed25519-dalek` v2.2+ (audited)
- Blake2b: Using `blake2` v0.10+ (well-tested)
- BLS12-381: Using `blstrs` v0.7+ (audited)

#### Network Security
- P2P layer implements rate limiting
- Message size limits enforced
- Invalid message rejection
- Peer reputation system

#### Memory Safety
- Written in Rust (memory-safe by default)
- No unsafe code in critical paths
- Bounds checking on all array access

### Security Best Practices

When deploying Cardano Node Rust:

#### 1. System Hardening
```bash
# Run as non-root user
useradd -r -s /bin/false cardano

# Limit file descriptors
ulimit -n 65536

# Disable core dumps
ulimit -c 0

# Set proper permissions
chmod 600 config.json
chmod 600 topology.json
```

#### 2. Network Security
- Use firewall to restrict access:
  ```bash
  # Allow only P2P port (example: 3001)
  ufw allow 3001/tcp

  # Allow metrics port only from monitoring (example: 12798)
  ufw allow from 10.0.0.0/8 to any port 12798
  ```

#### 3. Key Management
- **Never commit private keys to version control**
- Store KES keys with restrictive permissions (600)
- Rotate KES keys according to operational procedures
- Use hardware security modules (HSM) for production

#### 4. Monitoring
- Enable all security-relevant trace flags
- Monitor for unusual peer behavior
- Track consensus failures
- Alert on validation errors

#### 5. Updates
- Subscribe to security advisories
- Test updates in testnet environment
- Maintain rollback capability
- Follow the upgrade procedure

### Vulnerability Disclosure Policy

We follow a **coordinated disclosure** process:

1. **Day 0**: Vulnerability reported privately
2. **Day 2**: Acknowledgment sent to reporter
3. **Day 7**: Initial assessment and severity rating
4. **Day 30**: Patch developed and tested
5. **Day 35**: Security advisory published (if critical)
6. **Day 40**: Public disclosure with fixed version

For critical vulnerabilities affecting mainnet:
- Expedited patching (target: 7-14 days)
- Coordinated disclosure with Cardano Foundation
- Emergency release process

### Security Audit History

| Version | Audit Date | Auditor | Report |
|---------|-----------|---------|--------|
| 8.7.3   | 2025-10-03 | Internal | Haskell compatibility verified |

### Threat Model

#### In Scope
- Consensus attacks (long-range, nothing-at-stake)
- Network attacks (eclipse, Sybil, DDoS)
- Cryptographic vulnerabilities
- Memory safety issues
- Data corruption
- Information disclosure

#### Out of Scope
- Social engineering attacks
- Physical security
- Attacks requiring physical access
- Attacks on third-party dependencies (report to upstream)

### Bug Bounty Program

We are working on establishing a bug bounty program. Check back for updates.

Planned rewards:
- **Critical**: $5,000 - $10,000
- **High**: $2,000 - $5,000
- **Medium**: $500 - $2,000
- **Low**: $100 - $500

### Security Contacts

- **Security Team**: security@cardano-rust.org
- **Emergency Contact**: (to be established)
- **PGP Key**: (to be published)

### Additional Resources

- [Cardano Security Documentation](https://docs.cardano.org/security)
- [OWASP Secure Coding Practices](https://owasp.org/www-project-secure-coding-practices-quick-reference-guide/)
- [Rust Security Working Group](https://www.rust-lang.org/governance/wgs/wg-security-response)
- [CVE Database](https://cve.mitre.org/)

---

## Security Checklist for Contributors

When contributing code:

- [ ] No hardcoded secrets or private keys
- [ ] Input validation on all external data
- [ ] Proper error handling (no information leakage)
- [ ] Cryptographic operations use audited libraries
- [ ] No unsafe code without justification and review
- [ ] Memory allocations are bounded
- [ ] Network messages are size-limited
- [ ] All file operations check permissions
- [ ] Logging doesn't expose sensitive data
- [ ] Dependencies are from trusted sources
- [ ] Tests include security-relevant scenarios

Thank you for helping keep Cardano Node Rust secure! 🔒
