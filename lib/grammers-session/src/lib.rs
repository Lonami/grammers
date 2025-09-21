// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![deny(unsafe_code)]

mod chat;
mod generated;
mod message_box;
pub mod session_storage;

pub use chat::{ChatHashCache, PackedChat, PackedType};
pub use generated::LAYER as VERSION;
pub use generated::types::UpdateState;
pub use generated::types::User;
use generated::{enums, types};
pub use message_box::PrematureEndReason;
pub use message_box::{Gap, MessageBox, MessageBoxes, State, UpdatesLike, peer_from_input_peer};
use std::fmt::{Debug, Display};
use std::io::{Read, Seek, Write};
use std::net::{SocketAddr, SocketAddrV4, SocketAddrV6};
use std::path::Path;
use std::sync::{Arc, Mutex};
use snafu::ResultExt;

use crate::session_storage::{file_storage::{FileSessionStorage, FileSessionStorageOptions}, string_storage, SessionProviderError};

// Needed for auto-generated definitions.
use crate::session_storage::SessionProvider;
use grammers_tl_types::{Deserializable, Identifiable, Serializable, deserialize};
use crate::session_storage::file_storage::OpenMode;
use crate::session_storage::string_storage::{StringSessionStorage, StringSessionStorageOptions};

#[cfg_attr(
    feature = "impl-serde",
    derive(serde_derive::Serialize, serde_derive::Deserialize)
)]
pub struct Session {
    session: Arc<Mutex<types::Session>>,
    provider: Box<dyn SessionProvider + Send + Sync + 'static>,
}

#[allow(clippy::new_without_default)]
impl Session {
    pub fn save(&self) -> Result<(), SessionProviderError> {
        self.provider.save()
    }

    pub fn load(&self) -> Result<(), SessionProviderError> {
        self.provider.load()
    }

    pub fn load_from_file_or_create<T: AsRef<Path> + 'static>(path: T) -> Result<Self, SessionProviderError> {
        let fs_storage = FileSessionStorage::default_with_path(path)?;

        match fs_storage.load() {
            Ok(_) => (),
            Err(SessionProviderError::InvalidFormat {source}) => {
                let file_ref = fs_storage.get_file();
                let mut file = file_ref.lock().unwrap();
                let size = file.read_to_end(&mut vec![]).unwrap();
                if size > 0 {
                    return Err(SessionProviderError::InvalidFormat {source})
                }
            },
            Err(e) => return Err(e)
        };
        let session = fs_storage.get_session();
        let provider = Box::new(fs_storage);

        Ok(
            Self {
                session,
                provider,
            }
        )
    }

    pub fn load_from_file<T: AsRef<Path> + 'static>(path: T) -> Result<Self, SessionProviderError> {
        let fs_storage_options = FileSessionStorageOptions{
            path: Box::new(path),
            mode: OpenMode::Open,
            session: Arc::new(Mutex::new(types::Session{
                dcs: vec![],
                user: None,
                state: None,
            }))
        };

        let fs_storage = FileSessionStorage::new(fs_storage_options)?;
        fs_storage.load()?;
        let session = fs_storage.get_session();
        let provider = Box::new(fs_storage);

        Ok(
            Self {
                session,
                provider,
            }
        )
    }

    pub fn load_from_string(string: &str) -> Result<Self, SessionProviderError> {
        let storage = StringSessionStorage::new(StringSessionStorageOptions{ string: Some(Arc::new(Mutex::new(string.to_string()))), session: None });

        storage.load()?;

        Ok(
            Self{
                session: storage.get_session(),
                provider: Box::new(storage)
            }
        )
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

    pub fn get_dcs(&self) -> Vec<enums::DataCenter> {
        self.session.lock().unwrap().dcs.to_vec()
    }
}