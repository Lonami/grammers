.PHONY: help build test fmt lint clean audit doc release check-all

# Default target
help:
	@echo "Available targets:"
	@echo "  build       - Build all crates in release mode"
	@echo "  test        - Run all tests"
	@echo "  fmt         - Format code with rustfmt"
	@echo "  lint        - Run clippy lints"
	@echo "  clean       - Clean build artifacts"
	@echo "  audit       - Run security audit"
	@echo "  doc         - Build documentation"
	@echo "  release     - Build optimized release binaries"
	@echo "  check-all   - Run all checks (fmt, lint, test, audit)"

# Build all crates
build:
	cargo build --all-features

# Run tests
test:
	cargo test --all-features

# Format code
fmt:
	cargo fmt --all

# Run clippy
lint:
	cargo clippy --all-targets --all-features -- -D warnings

# Clean build artifacts
clean:
	cargo clean

# Security audit
audit:
	cargo audit

# Build documentation
doc:
	cargo doc --no-deps --all-features --open

# Build release binaries
release:
	cargo build --release --all-features

# Run all checks
check-all: fmt lint test audit
	@echo "All checks passed!"

# Development setup
setup:
	rustup component add rustfmt clippy
	cargo install cargo-audit cargo-deny
	@echo "Development environment ready!"

# Run example
run-example:
	@echo "Set TG_ID and TG_HASH environment variables first"
	cargo run --example echo

# Check dependencies
check-deps:
	cargo tree --duplicate
	cargo deny check 