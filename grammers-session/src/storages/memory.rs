// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::sync::Mutex;

use crate::types::{ChannelState, DcOption, PeerId, PeerInfo, UpdateState, UpdatesState};
use crate::{Session, SessionData};

/// In-memory session interface.
///
/// Does not actually offer direct ways to persist the state anywhere,
/// so it should only be used in very few select cases.
///
/// Logging in has a very high cost in terms of flood wait errors,
/// so the state really should be persisted by other means.
#[derive(Default)]
pub struct MemorySession(Mutex<SessionData>);

impl From<SessionData> for MemorySession {
    /// Constructs a memory session from the entirety of the session data,
    /// unlike the blanket `From` implementation which cannot import all values
    fn from(session_data: SessionData) -> Self {
        Self(Mutex::new(session_data))
    }
}

#[async_trait::async_trait]
impl Session for MemorySession {
    async fn home_dc_id(&self) -> i32 {
        self.0.lock().unwrap().home_dc
    }

    async fn set_home_dc_id(&self, dc_id: i32) {
        self.0.lock().unwrap().home_dc = dc_id;
    }

    async fn dc_option(&self, dc_id: i32) -> Option<DcOption> {
        self.0.lock().unwrap().dc_options.get(&dc_id).cloned()
    }

    async fn set_dc_option(&self, dc_option: &DcOption) {
        self.0
            .lock()
            .unwrap()
            .dc_options
            .insert(dc_option.id, dc_option.clone());
    }

    async fn peer(&self, peer: PeerId) -> Option<PeerInfo> {
        self.0.lock().unwrap().peer_infos.get(&peer).cloned()
    }

    async fn cache_peer(&self, peer: &PeerInfo) {
        self.0
            .lock()
            .unwrap()
            .peer_infos
            .insert(peer.id(), peer.clone());
    }

    async fn updates_state(&self) -> UpdatesState {
        self.0.lock().unwrap().updates_state.clone()
    }

    async fn set_update_state(&self, update: UpdateState) {
        let mut data = self.0.lock().unwrap();

        match update {
            UpdateState::All(updates_state) => {
                data.updates_state = updates_state;
            }
            UpdateState::Primary { pts, date, seq } => {
                data.updates_state.pts = pts;
                data.updates_state.date = date;
                data.updates_state.seq = seq;
            }
            UpdateState::Secondary { qts } => {
                data.updates_state.qts = qts;
            }
            UpdateState::Channel { id, pts } => {
                data.updates_state.channels.retain(|c| c.id != id);
                data.updates_state.channels.push(ChannelState { id, pts });
            }
        }
    }
}
