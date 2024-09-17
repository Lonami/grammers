# grammers-client

This library is a higher-level interface to interact with Telegram's API.

It contains the code necessary to create a client, connect to the API and
make Remote Procedure Calls (RPC) to it, such as signing in or sending a
message.

The library is in development, but new releases are only cut rarely.
[Specifying the dependency from the git repository][dep-git] is recommended:

```toml
grammers-client = { git = "https://github.com/Lonami/grammers" }
```

Please note that traits across versions are not always compatible.
If you depend on other `grammers-` crates, be sure all of them use
a compatible version (e.g. all of them using `git`).

Note that `grammers-tl-types` (needed to `client.invoke` "raw" functions)
is currently re-exported from within this crates, so it's easier to use the
re-export than to depend on the crate separatedly:

```rust
use grammers_client::grammers_tl_types as tl;
```

[dep-git]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#specifying-dependencies-from-git-repositories
