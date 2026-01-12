// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::collections::HashMap;

use crate::types::{DcOption, PeerId, PeerInfo, UpdateState, UpdatesState};
use crate::{DEFAULT_DC, KNOWN_DC_OPTIONS, Session};

/// In-memory representation of the entire [`Session`] state.
///
/// This type can be used for conversions `From` any [`Session`],
/// and be [`SessionData::import_to`] any other [`Session`].
pub struct SessionData {
    /// The identifier of the datacenter option determined
    /// to be the primary one for the logged-in user, or
    /// the identifier of an arbitrary datacenter otherwise.
    pub home_dc: i32,
    /// List of all known datacenter options, along with their
    /// Authorization Key if an encrypted connection has been
    /// made to them previously. Indexed by their identifier.
    pub dc_options: HashMap<i32, DcOption>,
    /// List of all peer informations cached in the session.
    /// Indexed by their identifier.
    pub peer_infos: HashMap<PeerId, PeerInfo>,
    /// Entirety of the update state for the logged-in user.
    pub updates_state: UpdatesState,
}

impl Default for SessionData {
    /// Constructs a default instance of the session data, with an arbitrary
    /// [`Self::home_dc`] and the list of statically-known [`Self::dc_options`].
    fn default() -> Self {
        Self {
            home_dc: DEFAULT_DC,
            dc_options: KNOWN_DC_OPTIONS
                .iter()
                .cloned()
                .map(|dc_option| (dc_option.id, dc_option))
                .collect(),
            peer_infos: HashMap::new(),
            updates_state: UpdatesState::default(),
        }
    }
}

impl SessionData {
    /// Imports all information from this session data to a type implementing `Session`.
    pub async fn import_to<S: Session>(&self, session: &S) {
        session.set_home_dc_id(self.home_dc).await;
        for dc_option in self.dc_options.values() {
            session.set_dc_option(dc_option).await;
        }
        for peer in self.peer_infos.values() {
            session.cache_peer(peer).await;
        }
        session
            .set_update_state(UpdateState::All(self.updates_state.clone()))
            .await;
    }
}
