# Contributing to Cardano Node Rust

First off, thank you for considering contributing to Cardano Node Rust! It's people like you that make this project such a great tool.

## Code of Conduct

This project and everyone participating in it is governed by our commitment to creating a welcoming and inclusive environment. Please be respectful and constructive in all interactions.

## How Can I Contribute?

### Reporting Bugs

Before creating bug reports, please check the issue list as you might find out that you don't need to create one. When you are creating a bug report, please include as many details as possible:

* **Use a clear and descriptive title** for the issue
* **Describe the exact steps to reproduce the problem**
* **Provide specific examples** to demonstrate the steps
* **Describe the behavior you observed** and what you expected to see
* **Include logs and error messages** if applicable
* **Specify your environment**: OS, Rust version, etc.

### Suggesting Enhancements

Enhancement suggestions are tracked as GitHub issues. When creating an enhancement suggestion, please include:

* **Use a clear and descriptive title**
* **Provide a detailed description** of the suggested enhancement
* **Explain why this enhancement would be useful**
* **List any alternative solutions** you've considered

### Pull Requests

* Fill in the required template
* Follow the Rust style guide (use `rustfmt` and `clippy`)
* Include appropriate test coverage
* Update documentation as needed
* End all files with a newline

## Development Process

### Setting Up Your Development Environment

1. **Install Rust 1.75 or later**:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   rustup update
   ```

2. **Clone the repository**:
   ```bash
   git clone https://github.com/cardano-rust/cardano-node-rust.git
   cd cardano-node-rust
   ```

3. **Build the project**:
   ```bash
   cargo build --workspace
   ```

4. **Run tests**:
   ```bash
   cargo test --workspace
   ```

### Development Workflow

1. **Create a feature branch**:
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes** following our coding standards:

   ```bash
   # Format code
   cargo fmt --all

   # Run clippy (strict mode)
   cargo clippy --workspace -- -D warnings

   # Run tests
   cargo test --workspace

   # Build documentation
   cargo doc --workspace --no-deps
   ```

3. **Commit your changes**:
   ```bash
   git add .
   git commit -m "feat: add amazing feature"
   ```

   Follow [Conventional Commits](https://www.conventionalcommits.org/):
   - `feat:` - New features
   - `fix:` - Bug fixes
   - `docs:` - Documentation changes
   - `test:` - Test additions or changes
   - `refactor:` - Code refactoring
   - `perf:` - Performance improvements
   - `chore:` - Build process or auxiliary tool changes

4. **Push to your fork**:
   ```bash
   git push origin feature/your-feature-name
   ```

5. **Open a Pull Request** against the `main` branch

## Coding Standards

### Rust Style Guide

* Use `rustfmt` for formatting: `cargo fmt --all`
* Use `clippy` for linting: `cargo clippy --workspace -- -D warnings`
* Follow Rust naming conventions:
  - `snake_case` for functions, variables, and modules
  - `PascalCase` for types and traits
  - `SCREAMING_SNAKE_CASE` for constants

### Code Structure

* **Modular design**: Each crate should have a clear, single responsibility
* **Error handling**: Use `Result<T, Error>` types, avoid panics in library code
* **Async code**: Use `async/await` for all I/O operations
* **Documentation**: Document all public APIs with doc comments

### Testing Requirements

* **Unit tests**: Test individual functions and methods
* **Integration tests**: Test crate interactions in the `tests/` directory
* **Property-based tests**: Use `proptest` for cryptographic and consensus operations
* **Test coverage**: Aim for >80% coverage for new code

Example test:
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Arrange
        let input = "test";

        // Act
        let result = function_under_test(input);

        // Assert
        assert_eq!(result, expected_value);
    }
}
```

### Haskell Compatibility

When modifying configuration or consensus code:

* **Verify against official configs**: Test with mainnet/preprod/preview configs
* **CBOR compatibility**: Ensure byte-identical serialization with Haskell node
* **Genesis validation**: Verify cryptographic hash checking
* **Protocol compliance**: Follow official Cardano specifications

## Project Architecture

```
cardano-node-rust/
├── crates/
│   ├── cardano-crypto/      # Cryptographic primitives
│   ├── cardano-consensus/   # Ouroboros consensus
│   ├── cardano-ledger/      # Ledger state and validation
│   ├── cardano-network/     # P2P networking
│   ├── cardano-storage/     # Database and persistence
│   ├── cardano-tracing/     # Logging and metrics
│   ├── cardano-api/         # External API
│   ├── cardano-node/        # Main executable
│   └── cardano-testnet/    # Testing utilities
└── tests/                   # Integration tests
```

### Adding a New Feature

1. **Identify the right crate** for your feature
2. **Check Haskell implementation** for reference (if applicable)
3. **Write tests first** (TDD approach)
4. **Implement the feature** following our standards
5. **Update documentation** including:
   - Code documentation (doc comments)
   - README if needed
   - CHANGELOG entry
   - HASKELL_COMPATIBILITY_VERIFIED.md if relevant

### Performance Considerations

* **Benchmark critical paths**: Use `cargo bench` for performance testing
* **Profile memory usage**: Use tools like `valgrind` or `heaptrack`
* **Optimize hot paths**: Focus on consensus and block validation
* **Async efficiency**: Minimize context switching and blocking

## Documentation

### Code Documentation

* Use `///` for public API documentation
* Include examples in doc comments where appropriate:

```rust
/// Validates a block according to consensus rules.
///
/// # Arguments
/// * `block` - The block to validate
/// * `state` - The current ledger state
///
/// # Returns
/// * `Ok(())` if valid
/// * `Err(ValidationError)` if invalid
///
/// # Example
/// ```
/// let result = validate_block(&block, &state)?;
/// ```
pub fn validate_block(block: &Block, state: &State) -> Result<()> {
    // ...
}
```

### Updating Documentation

When making changes, update:
- Inline code documentation
- README.md (if adding major features)
- CHANGELOG.md (always)
- HASKELL_COMPATIBILITY_VERIFIED.md (if relevant)

## Verification Process

Before submitting a PR, ensure:

```bash
# 1. Format code
cargo fmt --all

# 2. Lint with clippy (strict)
cargo clippy --workspace -- -D warnings

# 3. Run all tests
cargo test --workspace

# 4. Build release binary
cargo build --release

# 5. Generate documentation
cargo doc --workspace --no-deps

# 6. Test against official configs (if config changes)
cargo test --test test_haskell_compatibility -- --ignored
```

## Review Process

1. **Automated checks**: CI will run tests, clippy, and formatting checks
2. **Code review**: At least one maintainer will review your code
3. **Haskell compatibility**: Config/consensus changes verified against official node
4. **Documentation review**: Ensure all docs are updated
5. **Final approval**: Maintainer approval required before merge

## Release Process

Releases follow semantic versioning (MAJOR.MINOR.PATCH):

* **MAJOR**: Incompatible API changes
* **MINOR**: New features (backward compatible)
* **PATCH**: Bug fixes (backward compatible)

Maintainers handle releases, but contributors should:
* Update CHANGELOG.md with changes
* Follow version compatibility guidelines
* Note any breaking changes clearly

## Getting Help

* **Documentation**: Check the [README.md](README.md) and inline docs
* **Issues**: Search existing issues or create a new one
* **Discussions**: Use GitHub Discussions for questions
* **Discord/Slack**: Join our community channels (if available)

## Recognition

Contributors are recognized in:
* Git commit history
* Release notes
* Project documentation

Thank you for contributing to Cardano Node Rust! 🚀

---

## Quick Checklist for Contributors

- [ ] Code follows Rust style guide (`cargo fmt`)
- [ ] No clippy warnings (`cargo clippy -- -D warnings`)
- [ ] All tests pass (`cargo test --workspace`)
- [ ] New features have tests
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Haskell compatibility verified (if applicable)
- [ ] Commit messages follow Conventional Commits
- [ ] PR description is clear and complete
