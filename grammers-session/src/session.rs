// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::net::{SocketAddrV4, SocketAddrV6};

use crate::{PeerId, PeerInfo};

pub trait Session: Send + Sync {
    /// Datacenter that is "home" to the user authorized by this session.
    ///
    /// If not known, the ID of the closest datacenter should be returned instead.
    /// Note that incorrect guesses are allowed, and the user may need to migrate.
    ///
    /// This method should be cheap to call, because it is used on every request.
    fn home_dc_id(&self) -> i32;

    /// Changes the [`Session::home_dc_id`] after finding out the actual datacenter
    /// to which main queries should be executed against.
    fn set_home_dc_id(&self, dc_id: i32);

    /// Query a single datacenter option.
    ///
    /// If no up-to-date option has been [`Session::set_dc_option`] yet,
    /// a statically-known option must be returned.
    ///
    /// `None` may only be returned on invalid DC IDs or DCs that are not yet known.
    ///
    /// This method should be cheap to call, because it is used on every request.
    fn dc_option(&self, dc_id: i32) -> Option<DcOption>;

    /// Update the previously-known [`Session::dc_option`] with new values.
    ///
    /// Should also be used after generating permanent authentication keys to a datacenter.
    fn set_dc_option(&self, dc_option: &DcOption);

    /// Query a peer by its identity.
    ///
    /// Querying for [`PeerId::self_user`] can be used as a way to determine
    /// whether the authentication key has a logged-in user bound (i.e. signed in).
    fn peer(&self, peer: PeerId) -> Option<PeerInfo>;

    /// Cache a peer's basic information for [`Session::peer`] to be able to query them later.
    ///
    /// This method may not necessarily remember the peers forever,
    /// except for users where [`PeerInfo::User::is_self`] is `Some(true)`.
    fn cache_peer(&self, peer: &PeerInfo);

    /// Loads the entire updates state.
    fn updates_state(&self) -> UpdatesState;

    /// Update the state for one or all updates.
    fn set_update_state(&self, update: UpdateState);
}

/// A datacenter option.
///
/// This is very similar to Telegram's own `dcOption` type, except it also
/// contains the permanent authentication key and serves as a stable interface.
pub struct DcOption {
    /// Datacenter identifier.
    ///
    /// The primary datacenters have IDs from 1 to 5 inclusive, and are known statically by the session.
    pub id: i32,
    /// IPv4 address corresponding to this datacenter.
    pub ipv4: SocketAddrV4,
    /// IPv6 address corresponding to this datacenter. May actually be embedding the [`Self::ipv4`] address.
    pub ipv6: SocketAddrV6,
    /// Permanent authentication key generated for encrypted communication with this datacenter.
    ///
    /// A logged-in user may or not be bound to this authentication key.
    pub auth_key: Option<[u8; 256]>,
}

/// Full update state needed to process updates in order without gaps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UpdatesState {
    pub pts: i32,
    pub qts: i32,
    pub date: i32,
    pub seq: i32,
    pub channels: Vec<ChannelState>,
}

/// Update state for a single channel.
#[derive(Clone, Debug, PartialEq)]
pub struct ChannelState {
    pub id: i64,
    pub pts: i32,
}

/// Used in [`Session::set_update_state`] to update parts of the overall [`UpdatesState`].
pub enum UpdateState {
    All(UpdatesState),
    Primary { pts: i32, date: i32, seq: i32 },
    Secondary { qts: i32 },
    Channel { id: i64, pts: i32 },
}
