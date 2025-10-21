# Dependencies

## grammers-crypto

Used for its `RingBuffer` type.

## grammers-mtproto

Contains the actual implementation of the protocol without performing any IO. This crate's job
is to make use of said protocol over an actual network, and to coordinate sending messages.

## grammers-tl-types

Used to be able to execute certain protocol functions and to refer to the items produced by it.

## grammers-session

Update handling is very spread across Telegram's entire protocol.
Notable exceptions are types which aren't updates but affect them,
and partial updates that depend on the request that produced them.

Depending on the session means the bulk of that logic can remain separate.

## os_info

Telegram requires clients to send some basic system information when connecting to the server,
such as OS type or system version. If these values are not explicitly provided by the user, the
crate is used to load the expected values.

## locate-locale

Similar rationale to `os_info`, Telegram expects a system language code used by the client
(presumably for things such as localized service messages among others).

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

## url

Used to parse the optional proxy URL.

## hickory-resolver

Used to look up the IP address of the proxy host if a domain is provided.

## futures-util

Provides useful functions for working with futures/tasks.

## tokio-socks

SOCKS5 proxy support.

## web-time

Used for its web-friendly clock and timer as a replacement for `std::time` in the library.
Automatically falls back to `std::time` when we're not targeting web.

## web-sys

Only used when targeting `wasm32-unknown-unknown`. Used by the `Timeout` implementation to
call `setTimeout` and `clearTimeout` in the browser.

## wasm-bindgen-futures

Only used when targeting `wasm32-unknown-unknown`. Used by the `Timeout` implementation to
convert a `Promise` into a `Future`.

## ws_stream_wasm

Only used when targeting `wasm32-unknown-unknown`. Used to create a WebSocket connection
and get a byte stream from it.

## async_io_stream

Only used when targeting `wasm32-unknown-unknown`. Used to create a tokio-compatible stream
from a WebSocket connection.
