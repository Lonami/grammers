# grammers-client examples

This folder contains various simple, self-contained examples that you can use to base your
projects on. If you would like to see any other example that could be useful for others, or
you think these could be improved, feel free to contribute.

For the time being, the examples are licensed in the same way as the libraries.

## [ping.rs]

Pings Telegram by using raw API. This is the simplest possible example, and requires no valid
API ID or hash, since it's not logging in to any account.

## [echo.rs]

Simple echo-bot. It will respond to plain text messages with the same content. The updates will
be processed concurrently to showcase how one might use multiple handles the same time. In a
real-world scenario, you would probably want to use some form of task pool to spawn tasks within
limits. Or you might not need this at all if you want to process updates in order.

## [dialogs.rs]

Logs in to a user account and prints the title and ID of all the dialogs (chats they have joined
to and private conversations). It shows how to spawn a task to handle the network in the
background and using a single client handle to actually perform remote calls. If the client was
not running and thus not handling the network, no response would ever arrive to the handles!

This separation between client, which deals with all network events, and client handles, enables
you to have as many handles as you need, for as many tasks as you need (and enabling things like
iterators which keep their own handle). It requires some boilerplate to setup, but it's a very
powerful approach.

[ping.rs]: ping.rs
[echo.rs]: echo.rs
[dialogs.rs]: dialogs.rs
