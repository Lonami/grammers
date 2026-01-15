# Dependencies

## grammers-tl-types

Used for dealing with correct update processing.

The serialization and deserialization traits are also used for storing and loading the session
into and from bytes.

## futures-core

Used to make the `Session` trait dyn-compatible.

## log

Used to log messages during update processing.

## toml

Used to test that this file lists all dependencies from `Cargo.toml`.

## libsql

SQLite-based session storage.

## serde

_Optional._ Enables serialization and deserialization of configuration and session-related types

## serde_with

_Optional._ Provides custom serialization helpers

## tokio

Asynchronous locks and runtime for testing asynchronous `Session` implementations.
