# Dependencies

## grammers-tl-types

Used for dealing with correct update processing.

The serialization and deserialization traits are also used for storing and loading the session
into and from bytes.

## grammers-crypto

Used for utility functions such as converting to and from hexadecimal strings.

## grammers-tl-gen

Used to generate Rust code for a custom Type Language definition which defines the serialized
session format.

## grammers-tl-parser

Used to parse the custom Type Language definition used for the session itself.

## log

Used to log messages during update processing.

## toml

Used to test that this file lists all dependencies from `Cargo.toml`.

## web-time

Used for its web-friendly clock and timer as a replacement for `std::time` in the library.
Automatically falls back to `std::time` when we're not targeting web.
