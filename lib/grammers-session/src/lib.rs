// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod generated;

pub use generated::LAYER as VERSION;
use generated::{enums, types};
use grammers_crypto::auth_key::AuthKey;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::net::Ipv4Addr;
use std::path::Path;

// Needed for auto-generated definitions.
use grammers_tl_types::{deserialize, serialize, Deserializable, Identifiable, Serializable};

pub struct Session {
    file: File,
    pub session: types::Session,
}

impl Session {
    /// Loads or creates a new session file.
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        if let Ok(instance) = Self::load(path.as_ref()) {
            Ok(instance)
        } else {
            Self::create(path)
        }
    }

    /// Create a new session instance.
    pub fn create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        Ok(Self {
            file: File::create(path)?,
            session: types::Session {
                dcs: Vec::new(),
                user: None,
                state: None,
            },
        })
    }

    /// Load a previous session instance.
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut data = Vec::new();
        File::open(path.as_ref())?.read_to_end(&mut data)?;
        let enums::Session::Session(session) = enums::Session::from_bytes(&data).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "malformed session or unsupported version",
            )
        })?;

        Ok(Self {
            file: OpenOptions::new().write(true).open(path.as_ref())?,
            session,
        })
    }

    /// Saves the session file.
    pub fn save(&mut self) -> io::Result<()> {
        self.file.seek(io::SeekFrom::Start(0))?;
        self.file.set_len(0)?;
        self.file
            .write_all(&enums::Session::Session(self.session.clone()).to_bytes())?;
        self.file.sync_data()?;
        Ok(())
    }

    /// User's home datacenter ID, if known.
    pub fn user_dc(&self) -> Option<i32> {
        self.session
            .user
            .as_ref()
            .map(|enums::User::User(user)| user.dc)
    }

    /// Authorization key data for the given datacenter ID, if any.
    pub fn dc_auth_key(&self, dc_id: i32) -> Option<AuthKey> {
        self.session
            .dcs
            .iter()
            .filter_map(|enums::DataCenter::Center(dc)| {
                if dc.id == dc_id {
                    if let Some(auth) = &dc.auth {
                        let mut bytes = [0; 256];
                        bytes.copy_from_slice(auth);
                        Some(AuthKey::from_bytes(bytes))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .next()
    }

    pub fn insert_dc(&mut self, id: i32, (ipv4, port): (Ipv4Addr, u16), auth: &AuthKey) {
        if let Some(pos) = self
            .session
            .dcs
            .iter()
            .position(|enums::DataCenter::Center(dc)| dc.id == id)
        {
            self.session.dcs.remove(pos);
        }
        self.session.dcs.push(
            types::DataCenter {
                id,
                ipv4: Some(i32::from_le_bytes(ipv4.octets())),
                ipv6: None,
                port: port as i32,
                auth: Some(auth.to_bytes().to_vec()),
            }
            .into(),
        );
    }
}
