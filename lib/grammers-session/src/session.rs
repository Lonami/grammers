// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use std::io;
use std::net::SocketAddr;

/// The trait used by session implementations.
///
/// The creation and loading of sessions is left up to each concrete
/// implementation, since they may choose different ways of being loaded.
///
/// However, all of them should store the required information to allow
/// saving themselves whenever the client requests to do so.
pub trait Session {
    /// Save the user's main datacenter to the session.
    fn set_user_datacenter(&mut self, dc_id: i32, dc_addr: &SocketAddr);

    /// Save the authorization key data to the session.
    fn set_auth_key_data(&mut self, dc_id: i32, data: &[u8; 256]);

    /// Return the user's main datacenter, if any.
    fn get_user_datacenter(&self) -> Option<(i32, SocketAddr)>;

    /// Return the authorization key, if any.
    fn get_auth_key_data(&self, dc_id: i32) -> Option<[u8; 256]>;

    /// Persist the data to disk.
    fn save(&mut self) -> io::Result<()>;
}
