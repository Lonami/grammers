# gramme.rs

A set of Rust libraries to interact with Telegram's API,
hence the name *(tele)gramme.rs*.

## Libraries

The following libraries can be used to work with Telegram in some way:

* **[grammers-client]**: high-level API.
* **[grammers-crypto]**: cryptography-related methods.
* **[grammers-mtproto]**: implementation of the [Mobile Transport Protocol].
* **[grammers-mtsender]**: network connection to Telegram.
* **[grammers-tl-parser]**: a [Type Language] parser.
* **[grammers-tl-types]**: generated Rust types for a certain layer.

[Mobile Transport Protocol]: https://core.telegram.org/mtproto
[Type Language]: https://core.telegram.org/mtproto/TL
[grammers-client]: lib/grammers-client/
[grammers-crypto]: lib/grammers-crypto/
[grammers-mtproto]: lib/grammers-mtproto/
[grammers-mtsender]: lib/grammers-mtsender/
[grammers-tl-parser]: lib/grammers-tl-parser/
[grammers-tl-types]: lib/grammers-tl-types/
