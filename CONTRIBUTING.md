# Contributing to grammers

Thank you for your interest in contributing to grammers! This guide will help you get started.

## Code of Conduct

Please be respectful and constructive in all interactions. We will not tolerate poor behavior.

## Development Process

### Before You Start

1. Check existing issues and PRs to avoid duplicate work
2. For major changes, open an issue first to discuss the approach
3. Ensure your development environment is properly set up

### Development Setup

```bash
# Clone the repository
git clone https://github.com/Lonami/grammers.git
cd grammers

# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install development tools
rustup component add rustfmt clippy

# Run tests to verify setup
cargo test --all-features
```

### Code Standards

#### Error Handling

- **AVOID** using `.unwrap()` or `.expect()` in library code
- Use proper error types and propagation with `?`
- Only use `panic!()` for truly unrecoverable errors
- Document all error conditions

Example:
```rust
// Good
pub fn process_data(data: &[u8]) -> Result<ProcessedData, Error> {
    let header = Header::parse(data)?;
    // ...
}

// Avoid
pub fn process_data(data: &[u8]) -> ProcessedData {
    let header = Header::parse(data).unwrap();
    // ...
}
```

#### Testing

- Write tests for all new functionality
- Aim for high test coverage
- Include both unit tests and integration tests
- Test error cases, not just happy paths

#### Documentation

- All public APIs must have documentation
- Include examples in doc comments
- Update CHANGELOG.md for notable changes
- Keep README.md current

### Making Changes

1. **Create a branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**
   - Follow the existing code style
   - Add tests for new functionality
   - Update documentation as needed

3. **Run quality checks**
   ```bash
   # Format code
   cargo fmt

   # Run clippy
   cargo clippy --all-targets --all-features -- -D warnings

   # Run tests
   cargo test --all-features

   # Build documentation
   cargo doc --no-deps --all-features
   ```

4. **Commit your changes**
   - Use clear, descriptive commit messages
   - Reference issues when applicable (e.g., "Fix #123")

5. **Push and create a PR**
   - Push your branch to your fork
   - Create a pull request with a clear description
   - Ensure all CI checks pass

### Pull Request Guidelines

- **Title**: Clear and descriptive
- **Description**: Explain what changes and why
- **Testing**: Describe how you tested the changes
- **Breaking changes**: Clearly marked and justified

### Review Process

1. Automated checks must pass (formatting, linting, tests)
2. At least one maintainer review required
3. Address all feedback constructively
4. Squash commits if requested

## Areas Needing Attention

Based on the current codebase analysis, these areas need improvement:

1. **Error Handling**: Replace `unwrap()`/`expect()` with proper error handling
2. **TODO Items**: Many incomplete features marked with TODO comments
3. **Documentation**: Some APIs lack comprehensive documentation
4. **Test Coverage**: Additional test cases needed, especially for error paths

## Release Process

1. Update version numbers in all `Cargo.toml` files
2. Update CHANGELOG.md with release notes
3. Create and push a git tag
4. Publish to crates.io (maintainers only)

## Questions?

Feel free to open an issue for any questions about contributing. 