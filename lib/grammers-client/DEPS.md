# Dependencies

## grammers-crypto

Used for supporting logging in to accounts with 2-factor authentication enabled.

## grammers-mtproto

Used to configure the underlying protocol and transport used.

## grammers-mtsender

Used to drive the network connection to Telegram.

## grammers-session

Used to load and store session data, such as authorization key or current user identifier.

It also contains the logic needed to correctly process updates.

## grammers-tl-types

Used everywhere to invoke the "raw Telegram's API". It is the implementation of all the friendly
client methods.

## os_info

Telegram requires clients to send some basic system information when connecting to the server,
such as OS type or system version. If these values are not explicitly provided by the user, the
crate is used to load the expected values.

## locate-locale

Similar rationale to `os_info`, Telegram expects a system language code used by the client
(presumably for things such as localized service messages among others).

## pulldown-cmark

Enables the user to use markdown text to send formatted messages.

## html5ever

Enables the user to use HTML text to send formatted messages.

## tokio

Used to coordinate the asynchronous methods of the client.

## log

Used to log the execution of the client to help debug issues.

## md5

Needed when uploading files to Telegram.

## mime_guess

Used to guess the mime-type of uploaded files when sending media unless the user explicitly sets
the mime-type themselves. The mime-type is required by Telegram.

## chrono

Used for defining date types (for example, accessing the date of when a message was sent).

## simple_logger

Used by the examples to showcase how one may configure logging for more information.

## toml

Used to test that this file lists all dependencies from `Cargo.toml`.

## pin-project-lite

Used for return custom types that `impl Future` so that the requests can be further configured
without having to use `Box`.

## futures-util

Provides useful functions for working with futures/tasks.
