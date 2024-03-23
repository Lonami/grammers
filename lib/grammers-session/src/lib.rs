// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod chat;
mod generated;
mod message_box;

pub use chat::{ChatHashCache, PackedChat, PackedType};
pub use generated::types::UpdateState;
pub use generated::types::User;
pub use generated::LAYER as VERSION;
use generated::{enums, types};
use grammers_tl_types::deserialize::Error as DeserializeError;
pub use message_box::{channel_id, PrematureEndReason};
pub use message_box::{Gap, MessageBox};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use std::path::Path;
use std::sync::Mutex;

// Needed for auto-generated definitions.
use grammers_tl_types::{deserialize, Deserializable, Identifiable, Serializable};

pub struct Session {
    session: Mutex<types::Session>,
}

#[allow(clippy::new_without_default)]
impl Session {
    pub fn new() -> Self {
        Self {
            session: Mutex::new(types::Session {
                dcs: Vec::new(),
                user: None,
                state: None,
            }),
        }
    }

    /// Load a previous session instance from a file,
    /// creating one if it doesn't exist
    pub fn load_file_or_create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            File::create(path)?;
            let session = Session::new();
            session.save_to_file(path)?;
            Ok(session)
        } else {
            Self::load_file(path)
        }
    }

    /// Load a previous session instance from a file.
    pub fn load_file<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut data = Vec::new();
        File::open(path.as_ref())?.read_to_end(&mut data)?;

        Self::load(&data).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn load(data: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            session: Mutex::new(
                enums::Session::from_bytes(data)
                    .map_err(|e| match e {
                        DeserializeError::UnexpectedEof => Error::MalformedData,
                        DeserializeError::UnexpectedConstructor { .. } => Error::UnsupportedVersion,
                    })?
                    .into(),
            ),
        })
    }

    pub fn signed_in(&self) -> bool {
        self.session.lock().unwrap().user.is_some()
    }

    pub fn dc_auth_key(&self, dc_id: i32) -> Option<[u8; 256]> {
        self.session
            .lock()
            .unwrap()
            .dcs
            .iter()
            .filter_map(|enums::DataCenter::Center(dc)| {
                if dc.id == dc_id {
                    if let Some(auth) = &dc.auth {
                        let mut bytes = [0; 256];
                        bytes.copy_from_slice(auth);
                        Some(bytes)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .next()
    }

    pub fn insert_dc(&self, id: i32, addr: SocketAddr, auth: [u8; 256]) {
        let mut session = self.session.lock().unwrap();
        if let Some(pos) = session
            .dcs
            .iter()
            .position(|enums::DataCenter::Center(dc)| dc.id == id)
        {
            session.dcs.remove(pos);
        }

        let (ip_v4, ip_v6): (Option<&SocketAddrV4>, Option<&SocketAddrV6>) = match &addr {
            SocketAddr::V4(ip_v4) => (Some(ip_v4), None),
            SocketAddr::V6(ip_v6) => (None, Some(ip_v6)),
        };

        session.dcs.push(
            types::DataCenter {
                id,
                ipv4: ip_v4.map(|addr| i32::from_le_bytes(addr.ip().octets())),
                ipv6: ip_v6.map(|addr| addr.ip().octets()),
                port: addr.port() as i32,
                auth: Some(auth.into()),
            }
            .into(),
        );
    }

    pub fn set_user(&self, id: i64, dc: i32, bot: bool) {
        self.session.lock().unwrap().user = Some(User { id, dc, bot }.into())
    }

    /// Returns the stored user
    pub fn get_user(&self) -> Option<User> {
        self.session
            .lock()
            .unwrap()
            .user
            .as_ref()
            .map(|enums::User::User(user)| user.clone())
    }

    pub fn get_state(&self) -> Option<UpdateState> {
        let session = self.session.lock().unwrap();
        let enums::UpdateState::State(state) = session.state.clone()?;
        Some(state)
    }

    pub fn set_state(&self, state: UpdateState) {
        self.session.lock().unwrap().state = Some(state.into())
    }

    pub fn get_dcs(&self) -> Vec<types::DataCenter> {
        self.session
            .lock()
            .unwrap()
            .dcs
            .iter()
            .map(|enums::DataCenter::Center(dc)| dc.clone())
            .collect()
    }

    #[must_use]
    pub fn save(&self) -> Vec<u8> {
        enums::Session::Session(self.session.lock().unwrap().clone()).to_bytes()
    }

    /// Saves the session to a file.
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let mut file = OpenOptions::new().write(true).open(path.as_ref())?;
        file.seek(io::SeekFrom::Start(0))?;
        file.set_len(0)?;
        file.write_all(&self.save())?;
        file.sync_data()
    }
}

#[derive(Debug)]
pub enum Error {
    MalformedData,
    UnsupportedVersion,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::MalformedData => write!(f, "malformed data"),
            Error::UnsupportedVersion => write!(f, "unsupported version"),
        }
    }
}

impl std::error::Error for Error {}
