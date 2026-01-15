// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use futures_core::future::BoxFuture;

use crate::peer::PeerRef;
use crate::types::{DcOption, PeerId, PeerInfo, UpdateState, UpdatesState};

/// The main interface to interact with the different [`crate::storages`].
///
/// All methods are synchronous and currently infallible because clients
/// are not equipped to deal with the arbitrary errors that a dynamic
/// `Session` could produce. This may change in the future.
///
/// A newly-created storage should return the same values that
/// [crate::SessionData::default] would produce.
pub trait Session: Send + Sync {
    /// Datacenter that is "home" to the user authorized by this session.
    ///
    /// If not known, the ID of the closest datacenter should be returned instead.
    /// Note that incorrect guesses are allowed, and the user may need to migrate.
    ///
    /// This method should be implemented as an infallible memory read,
    /// because it is used on every request and thus should be cheap to call.
    fn home_dc_id(&self) -> i32;

    /// Changes the [`Session::home_dc_id`] after finding out the actual datacenter
    /// to which main queries should be executed against.
    ///
    /// This must update the value in the cache layer used by `home_dc_id`.
    fn set_home_dc_id(&self, dc_id: i32) -> BoxFuture<'_, ()>;

    /// Query a single datacenter option.
    ///
    /// If no up-to-date option has been [`Session::set_dc_option`] yet,
    /// a statically-known option must be returned.
    ///
    /// `None` may only be returned on invalid DC IDs or DCs that are not yet known.
    ///
    /// This method should be implemented as an infallible memory read,
    /// because it is used on every request and thus should be cheap to call.
    fn dc_option(&self, dc_id: i32) -> Option<DcOption>;

    /// Update the previously-known [`Session::dc_option`] with new values.
    ///
    /// Should also be used after generating permanent authentication keys to a datacenter.
    ///
    /// This must update the value in the cache layer used by `dc_option`.
    fn set_dc_option(&self, dc_option: &DcOption) -> BoxFuture<'_, ()>;

    /// Query a peer by its identity.
    ///
    /// Querying for [`PeerId::self_user`] can be used as a way to determine
    /// whether the authentication key has a logged-in user bound (i.e. signed in).
    fn peer(&self, peer: PeerId) -> BoxFuture<'_, Option<PeerInfo>>;

    /// Query the full peer reference from its identity.
    ///
    /// By default, this uses [`Session::peer`] to retrieve the [`PeerAuth`](crate::types::PeerAuth).
    fn peer_ref(&self, peer: PeerId) -> BoxFuture<'_, Option<PeerRef>> {
        Box::pin(async move {
            self.peer(peer)
                .await
                .and_then(|info| info.auth())
                .map(|auth| PeerRef { id: peer, auth })
        })
    }

    /// Cache a peer's basic information for [`Session::peer`] to be able to query them later.
    ///
    /// This method may not necessarily remember the peers forever,
    /// except for users where [`PeerInfo::User::is_self`] is `Some(true)`.
    fn cache_peer(&self, peer: &PeerInfo) -> BoxFuture<'_, ()>;

    /// Loads the entire updates state.
    fn updates_state(&self) -> BoxFuture<'_, UpdatesState>;

    /// Update the state for one or all updates.
    fn set_update_state(&self, update: UpdateState) -> BoxFuture<'_, ()>;
}
