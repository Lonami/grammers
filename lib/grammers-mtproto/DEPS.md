# Dependencies

## crc32fast

Needed by the full transport mode.

## getrandom

Needed to generate secure values, such as nonces, during the generation of an authorization key.

## grammers-crypto

Mainly used to encrypt and decrypt messages exchanged with Telegram's servers, but also contains
other miscellaneous functions such as integer factorization.

## grammers-tl-types

Used to serialize and deserialize the messages exchanged with Telegram's servers.

## flate2

Messages may be gzip-encoded to reduce bandwidth, so this crate is used for both decompressing
incoming messages and compressing them when it is worth it (based on some simple heuristics).

## num-bigint

Used during the generation of the authorization key.

## sha1

Used during the generation of the authorization key.

## bytes

Used for the input and output buffers.

## toml

Used to test that this file lists all dependencies from `Cargo.toml`.
