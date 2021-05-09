# Dependencies

## aes

Needed for its AES-256 cipher, which is used to build the AES-IGE mode used by Telegram.

## getrandom

Used to generate secure padding when encrypting outgoing messages.

## num-bigint

Used for hand-rolled RSA encryption, which is used during the generation of an authorization key.

This *may* make the second part of the authorization key vulnerable to a certain type of timing
attack, although I'm not sure how dangerous it is in practice.

If this concerns you, please propose a fix and send a pull request.

## sha1

Used in certain functions that require a certain AES key.

## sha2

Used for calculating the AES key given an authorization key, and also for 2FA.

## pbkdf2

Used for methods relied on by the 2-factor offered by Telegram.

## hmac

Used for methods relied on by the 2-factor offered by Telegram.

## glass_pumpkin

Used for methods relied on by the 2-factor offered by Telegram.

## bencher

Used for benchmarking the encryption and decryption methods.

## toml

Used to test that this file lists all dependencies from `Cargo.toml`.
