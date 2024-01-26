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

[dep-git]: https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#specifying-dependencies-from-git-repositories
