// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use std::io;
use std::net::SocketAddr;

use crate::Session;

/// A basic session implementation, kept only in-memory.
pub struct MemorySession {
    user_dc: Option<(i32, SocketAddr)>,
    auth_key_data: Option<[u8; 256]>,
}

impl MemorySession {
    /// Create a new session instance.
    pub fn new() -> Self {
        Self {
            user_dc: None,
            auth_key_data: None,
        }
    }
}

impl Session for MemorySession {
    fn set_user_datacenter(&mut self, dc_id: i32, dc_addr: &SocketAddr) {
        self.user_dc = Some((dc_id, dc_addr.clone()));
    }

    fn set_auth_key_data(&mut self, _dc_id: i32, data: &[u8; 256]) {
        self.auth_key_data = Some(data.clone());
    }

    fn get_user_datacenter(&self) -> Option<(i32, SocketAddr)> {
        self.user_dc.clone()
    }

    fn get_auth_key_data(&self, _dc_id: i32) -> Option<[u8; 256]> {
        self.auth_key_data.clone()
    }

    fn save(&mut self) -> io::Result<()> {
        Ok(())
    }
}
