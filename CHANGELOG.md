# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Enhanced CI/CD pipeline with security audits, clippy linting, and multi-OS testing
- Enterprise-grade release profile with optimizations (LTO, opt-level 3, single codegen unit)
- Clippy configuration for stricter code quality standards
- CHANGELOG.md for version tracking
- SECURITY.md for security policy documentation

### Changed
- Improved GitHub Actions workflow with separate jobs for formatting, linting, security, and testing
- Updated CI to test on multiple operating systems (Ubuntu, Windows, macOS)
- Added beta Rust channel to test matrix for early compatibility detection

### Security
- Added cargo-audit integration for automated vulnerability scanning
- Configured stricter compiler warnings and linting rules

## [0.7.0] - Previous Release

### Added
- High-level client API for Telegram interactions
- Session management and storage
- Cryptographic implementations for MTProto
- TL (Type Language) parser and code generator
- Support for various Telegram features including messages, dialogs, and file transfers

### Known Issues
- Multiple TODO items throughout codebase indicating incomplete features
- Extensive use of `.unwrap()` and `.expect()` that could cause panics
- Several explicit `panic!()` calls in error handling paths 