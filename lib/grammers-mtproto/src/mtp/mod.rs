// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implementation of the [Mobile Transport Protocol]. This layer is
//! responsible for converting zero or more input requests into outgoing
//! messages, and to process the response.
//!
//! A distinction between plain and encrypted is made for simplicity (the
//! plain hardly requires to process any state) and to help prevent invalid
//! states (encrypted communication cannot be made without an authorization
//! key).
//!
//! [Mobile Transport Protocol]: https://core.telegram.org/mtproto/description
mod plain;
//mod encrypted;

use crate::errors::DeserializeError;
use crate::MsgId;
pub use plain::Plain;
//pub use encrypted::Encrypted;

/// Results from the deserialization of a response.
pub struct Deserialization {
    /// Result bodies to Remote Procedure Calls.
    pub rpc_results: Vec<(MsgId, Vec<u8>)>,
    /// Updates that came in the response.
    pub updates: Vec<Vec<u8>>,
}

/// The trait used by the [Mobile Transport Protocol] to serialize outgoing
/// messages and deserialize incoming ones into proper responses.
///
/// [Mobile Transport Protocol]: https://core.telegram.org/mtproto/description
pub trait Mtp {
    /// Serializes zero or more requests into a single outgoing message that
    /// should be sent over a transport.
    ///
    /// Note that even if there are no requests to serialize, the protocol may
    /// produce data that has to be sent after deserializing incoming messages.
    ///
    /// Returns the assigned message IDs to each request in order. If a request
    /// does not have a corresponding message ID, it has not been enqueued, and
    /// must be resent.
    ///
    /// # Panics
    ///
    /// The method panics if the body length is not padded to 4 bytes. The
    /// serialization of requests will always be correctly padded, so adding
    /// an error case for this rare case (impossible with the expected inputs)
    /// would simply be unnecessary.
    ///
    /// The method also panics if the body length is too large for similar
    /// reasons. It is not reasonable to construct huge requests (although
    /// possible) because they would likely fail with a RPC error anyway,
    /// so we avoid another error case by simply panicking.
    ///
    /// The definition of "too large" is roughly 1MB, so as long as the
    /// payload is below that mark, it's safe to call.
    fn serialize(
        &mut self,
        requests: &Vec<Vec<u8>>,
        output: &mut Vec<u8>,
    ) -> Result<Vec<MsgId>, ()>;

    /// Deserializes a single incoming message payload into zero or more responses.
    fn deserialize(&mut self, payload: &[u8]) -> Result<Deserialization, DeserializeError>;
}
