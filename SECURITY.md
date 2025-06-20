# Security Policy

## Supported Versions

Currently supported versions for security updates:

| Version | Supported          |
| ------- | ------------------ |
| 0.7.x   | :white_check_mark: |
| < 0.7   | :x:                |

## Reporting a Vulnerability

We take security seriously. If you discover a security vulnerability, please follow these steps:

1. **DO NOT** create a public GitHub issue for the vulnerability
2. Email security details to the maintainers (see repository contacts)
3. Include:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if available)

### Response Timeline

- **Initial Response**: Within 48 hours
- **Status Update**: Within 7 days
- **Resolution Target**: Within 30 days for critical issues

## Security Considerations

### Cryptography
- This library implements cryptographic protocols for Telegram's MTProto
- The cryptographic code has **NOT** been formally audited
- For security-critical applications, we recommend:
  - Reviewing the `grammers-crypto` module thoroughly
  - Conducting your own security audit
  - Testing extensively in a sandboxed environment

### Known Security Areas

1. **Authentication**: Review authentication flow in `grammers-mtproto`
2. **Session Storage**: Ensure secure storage of session files
3. **Network Communication**: All traffic should use TLS/MTProto encryption
4. **Input Validation**: User inputs should be validated before use

### Best Practices

When using grammers in production:

1. **Session Security**:
   - Store session files with restricted permissions (600 on Unix)
   - Never commit session files to version control
   - Rotate sessions periodically

2. **Error Handling**:
   - Avoid exposing internal errors to end users
   - Log security events appropriately
   - Handle panics gracefully in production

3. **Dependencies**:
   - Regularly update dependencies
   - Monitor security advisories via `cargo audit`
   - Use exact version pinning for critical deployments

4. **API Keys**:
   - Never hardcode API keys in source code
   - Use environment variables or secure vaults
   - Rotate keys regularly

## Security Tools

This project uses:
- `cargo audit` for dependency vulnerability scanning
- `clippy` with security-focused lints
- Automated CI/CD security checks

## Disclaimer

This software is provided "as is" without warranty. Users are responsible for:
- Conducting their own security assessments
- Implementing appropriate security controls
- Complying with relevant regulations 