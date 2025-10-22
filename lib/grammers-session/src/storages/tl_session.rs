// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use crate::generated::{enums, types};
use grammers_tl_types as tl;
use grammers_tl_types::deserialize::Error as DeserializeError;
use grammers_tl_types::{Deserializable, Serializable};
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::net::Ipv4Addr;
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use std::path::Path;
use std::sync::Mutex;

#[cfg_attr(
    feature = "impl-serde",
    derive(serde_derive::Serialize, serde_derive::Deserialize)
)]
pub struct TlSession {
    session: Mutex<types::Session>,
}

/// Hardcoded known `static` options from `functions::help::GetConfig`.
pub const KNOWN_DC_OPTIONS: [types::DataCenter; 5] = [
    types::DataCenter {
        id: 1,
        ipv4: Some(i32::from_le_bytes(
            Ipv4Addr::new(149, 154, 175, 53).octets(),
        )),
        ipv6: None,
        port: 443,
        auth: None,
    },
    types::DataCenter {
        id: 2,
        ipv4: Some(i32::from_le_bytes(
            Ipv4Addr::new(149, 154, 167, 51).octets(),
        )),
        ipv6: None,
        port: 443,
        auth: None,
    },
    types::DataCenter {
        id: 3,
        ipv4: Some(i32::from_le_bytes(
            Ipv4Addr::new(149, 154, 175, 100).octets(),
        )),
        ipv6: None,
        port: 443,
        auth: None,
    },
    types::DataCenter {
        id: 4,
        ipv4: Some(i32::from_le_bytes(
            Ipv4Addr::new(149, 154, 167, 92).octets(),
        )),
        ipv6: None,
        port: 443,
        auth: None,
    },
    types::DataCenter {
        id: 5,
        ipv4: Some(i32::from_le_bytes(Ipv4Addr::new(91, 108, 56, 190).octets())),
        ipv6: None,
        port: 443,
        auth: None,
    },
];

#[allow(clippy::new_without_default)]
impl TlSession {
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
            let session = TlSession::new();
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
            .filter_map(|dc| match dc {
                enums::DataCenter::Center(types::DataCenter {
                    id,
                    auth: Some(auth),
                    ..
                }) if *id == dc_id => auth.clone().try_into().ok(),
                enums::DataCenter::Ws(types::DataCenterWs {
                    id,
                    auth: Some(auth),
                    ..
                }) if *id == dc_id => auth.clone().try_into().ok(),
                _ => None,
            })
            .next()
    }

    fn insert_dc(&self, new_dc: enums::DataCenter) {
        let mut session = self.session.lock().unwrap();

        if let Some(pos) = session.dcs.iter().position(|dc| dc.id() == new_dc.id()) {
            session.dcs.remove(pos);
        }
        session.dcs.push(new_dc);
    }

    pub fn set_dc_auth_key(&self, dc_id: i32, auth: [u8; 256]) {
        let mut session = self.session.lock().unwrap();

        for dc in session.dcs.iter_mut() {
            if dc.id() == dc_id {
                match dc {
                    enums::DataCenter::Center(data_center) => data_center.auth = Some(auth.into()),
                    enums::DataCenter::Ws(data_center_ws) => {
                        data_center_ws.auth = Some(auth.into())
                    }
                }
                break;
            }
        }
    }

    pub fn insert_dc_tcp(&self, id: i32, addr: &SocketAddr, auth: [u8; 256]) {
        let (ip_v4, ip_v6): (Option<&SocketAddrV4>, Option<&SocketAddrV6>) = match addr {
            SocketAddr::V4(ip_v4) => (Some(ip_v4), None),
            SocketAddr::V6(ip_v6) => (None, Some(ip_v6)),
        };

        self.insert_dc(
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

    pub fn insert_dc_ws(&self, id: i32, url: &str, auth: [u8; 256]) {
        self.insert_dc(
            types::DataCenterWs {
                id,
                url: url.to_string(),
                auth: Some(auth.into()),
            }
            .into(),
        );
    }

    pub fn set_user(&self, id: i64, dc: i32, bot: bool) {
        self.session.lock().unwrap().user = Some(types::User { id, dc, bot }.into())
    }

    /// Returns the stored user
    pub fn get_user(&self) -> Option<types::User> {
        self.session
            .lock()
            .unwrap()
            .user
            .as_ref()
            .map(|enums::User::User(user)| user.clone())
    }

    pub fn get_state(&self) -> Option<types::UpdateState> {
        let session = self.session.lock().unwrap();
        let enums::UpdateState::State(state) = session.state.clone()?;
        Some(state)
    }

    pub fn set_state(&self, state: types::UpdateState) {
        self.session.lock().unwrap().state = Some(state.into())
    }

    pub fn get_dcs(&self) -> Vec<enums::DataCenter> {
        self.session.lock().unwrap().dcs.to_vec()
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

pub fn state_to_update_state(
    tl::enums::updates::State::State(state): tl::enums::updates::State,
) -> types::UpdateState {
    types::UpdateState {
        pts: state.pts,
        qts: state.qts,
        date: state.date,
        seq: state.seq,
        channels: Vec::new(),
    }
}

pub fn try_push_channel_state(
    update_state: &mut types::UpdateState,
    channel_id: i64,
    pts: i32,
) -> bool {
    if update_state
        .channels
        .iter()
        .any(|enums::ChannelState::State(channel_state)| channel_state.channel_id == channel_id)
    {
        return false;
    }

    update_state
        .channels
        .push(enums::ChannelState::State(types::ChannelState {
            channel_id,
            pts,
        }));
    true
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
