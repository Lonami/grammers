# Dependencies

## grammers-mtproto

Contains the actual implementation of the protocol without performing any IO. This crate's job
is to make use of said protocol over an actual network, and to coordinate sending messages.

## grammers-tl-types

Used to be able to execute certain protocol functions and to refer to the items produced by it.

## tokio

Primarly used for its asynchronous `TcpStream`, although its channels are also used in order to
communicate with the sender.

## bytes

Used for input and output buffers.

## log

Used to log what's going on during the lifetime of the sender.

## simple_logger

Used in the tests in order to debug with more information when things go wrong.

## toml

Used to test that this file lists all dependencies from `Cargo.toml`.
