# gramme.rs

A set of Rust libraries to interact with Telegram's API, hence the name *(tele)gramme.rs*.

## Current status

It works! The high-level interface is slowly taking shape, and it can already be used to [build
real projects], such as [RSS bots].

For an up-to-date taste on how the library looks like, refer to the [client examples] folder.

For more documentation, please refer to <https://docs.rs/grammers-client/>.

## Libraries

The following libraries under [`lib/`] can be used to work with Telegram in some way:

* **[grammers-client]**: high-level API.
* **[grammers-crypto]**: cryptography-related methods.
* **[grammers-mtproto]**: implementation of the [Mobile Transport Protocol].
* **[grammers-mtsender]**: network connection to Telegram.
* **[grammers-session]**: session storages for the client.
* **[grammers-tl-gen]**: Rust code generator from TL definitions.
* **[grammers-tl-parser]**: a [Type Language] parser.
* **[grammers-tl-types]**: generated Rust types for a certain layer.

## Binaries

The following auxiliary CLI tools are available in the [`bin/`] folder:

* **[scrape-docs]**: scrape Telegram's website to obtain raw API documentation.
* **[tl-to-json]**: tool to read `.tl` and output `.json`, equivalent to
  [Telegram's JSON schema][tl-json].

## Security

It is recommended to always use [cargo-crev] to verify the trustworthiness of each of your
dependencies, including this one.

As far as I know, this code has not been audited, so if, for any reason, you're using this crate
where security is critical, I strongly encourage you to review at least `grammers-crypto` and the
authentication part of `grammers-mtproto`. I am not a security expert, although I trust my code
enough to use it myself.

If you know about some published audit for this crate, please let me know, so that I can link it
here and review the issues found.

## License

All the libraries and binaries contained in this repository are licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE] or
  http://www.apache.org/licenses/LICENSE-2.0)

* MIT license ([LICENSE-MIT] or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Thank you for considering to contribute! I'll try my best to provide quick, constructive feedback
on your issues or pull requests. Please do call me out if you think my behaviour is not acceptable
at any time. I will try to keep the discussion as technical as possible. Similarly, I will not
tolerate poor behaviour from your side towards other people (including myself).

If you don't have the time to [contribute code], you may contribute by [reporting issues] or
feature ideas. Please note that every feature added will increase maintenance burden on my part,
so be mindful when suggesting things. It may be possible that your idea could exist as its own
crate, offered as [extensions to grammers].

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

[build real projects]: https://github.com/Lonami/grammers/wiki/Real-world-projects
[RSS bots]: https://github.com/Lonami/srsrssrsbot
[client examples]: lib/grammers-client/examples
[Mobile Transport Protocol]: https://core.telegram.org/mtproto
[Type Language]: https://core.telegram.org/mtproto/TL
[`lib/`]: lib/
[grammers-client]: lib/grammers-client/
[grammers-crypto]: lib/grammers-crypto/
[grammers-mtproto]: lib/grammers-mtproto/
[grammers-mtsender]: lib/grammers-mtsender/
[grammers-session]: lib/grammers-session/
[grammers-tl-gen]: lib/grammers-tl-gen/
[grammers-tl-parser]: lib/grammers-tl-parser/
[grammers-tl-types]: lib/grammers-tl-types/
[`bin/`]: bin/
[scrape-docs]: bin/scrape-docs/
[tl-to-json]: bin/tl-to-json/
[tl-json]: https://core.telegram.org/schema/json
[cargo-crev]: https://github.com/crev-dev/cargo-crev
[LICENSE-APACHE]: LICENSE-APACHE
[LICENSE-MIT]: LICENSE-MIT
[contribute code]: https://github.com/Lonami/grammers/compare
[reporting issues]: https://github.com/Lonami/grammers/issues/new
[extensions to grammers]: https://github.com/Lonami/grammers/wiki/Client-extensions
