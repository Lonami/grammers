// Copyright 2024 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use super::Client;

impl Client {
    /// Provide access to the packed chat by ID using the "chat hash cache"
    /// maintained by the client.
    ///
    /// ## Warning
    ///
    /// Note that this is merely in-memory cache, and all the cached data
    /// is lost upon program termination. Relying on this cache is thus only
    /// advised when the program does not use session persistence, and will
    /// reload and process the whole updates history from scratch every time
    /// it runs - as only then the cache will be fully seeded with the data
    /// for all the chats.
    pub fn packed_by_id(&self, id: i64) -> Option<grammers_session::PackedChat> {
        let state = self.0.state.read().unwrap();
        state.chat_hashes.get(id)
    }
}
