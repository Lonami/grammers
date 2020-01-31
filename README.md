# gramme.rs

A set of Rust libraries to interact with Telegram's API,
hence the name *(tele)gramme.rs*.

## Libraries

The following libraries can be used to work with Telegram in some way:

* **[grammers-client]**: high-level API.
* **[grammers-crypto]**: cryptography-related methods.
* **[grammers-mtproto]**: implementation of the [Mobile Transport Protocol].
* **[grammers-mtsender]**: network connection to Telegram.
* **[grammers-session]**: session storages for the client.
* **[grammers-tl-parser]**: a [Type Language] parser.
* **[grammers-tl-types]**: generated Rust types for a certain layer.

## License

All the libraries contained in this repository are licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE] or
  http://www.apache.org/licenses/LICENSE-2.0)

* MIT license ([LICENSE-MIT] or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[Mobile Transport Protocol]: https://core.telegram.org/mtproto
[Type Language]: https://core.telegram.org/mtproto/TL
[grammers-client]: lib/grammers-client/
[grammers-crypto]: lib/grammers-crypto/
[grammers-mtproto]: lib/grammers-mtproto/
[grammers-mtsender]: lib/grammers-mtsender/
[grammers-session]: lib/grammers-session/
[grammers-tl-parser]: lib/grammers-tl-parser/
[grammers-tl-types]: lib/grammers-tl-types/
[LICENSE-APACHE]: LICENSE-APACHE
[LICENSE-MIT]: LICENSE-MIT
