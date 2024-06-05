# grammers-tl-types

This library provides Rust `struct` and `enum` types representing
the definitions from the [Type Language] build-time input files.

In addition, each type has an `impl` on `Serializable` and `Deserializable`,
the former serializes instances into byte arrays as described by the section
on [Binary Data Serialization], and the latter deserializes them.

[Type Language]: https://core.telegram.org/mtproto/TL
[Binary Data Serialization]: https://core.telegram.org/mtproto/serialize
[Latest API TL]: https://github.com/telegramdesktop/tdesktop/blob/dev/Telegram/SourceFiles/mtproto/scheme/api.tl
[Latest Mtproto TL]: https://github.com/telegramdesktop/tdesktop/blob/dev/Telegram/SourceFiles/mtproto/scheme/mtproto.tl
