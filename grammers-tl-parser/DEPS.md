# Dependencies

## crc32fast

The Type Language definitions get assigned a unique identifier, either explicitly or implicitly.

If said identifier is not defined explicitly, it must be calculated by taking the CRC32 of the
definition itself.

This can also be used to assert that the inferred ID matches the explicitly assigned ID.

## toml

Used to test that this file lists all dependencies from `Cargo.toml`.
