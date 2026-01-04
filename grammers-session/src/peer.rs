// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::{fmt, ops::Deref as _};

use grammers_tl_types as tl;

/// A compact peer identifier.
/// ```
/// use std::mem::size_of;
/// assert_eq!(size_of::<grammers_session::types::PeerId>(), size_of::<i64>());
/// ```
/// The [`PeerInfo`] cached by the session for this `PeerId` may be retrieved via [`crate::Session::peer`].
///
/// The internal representation uses the Bot API Dialog ID format to
/// bit-pack both the peer's true identifier and type in a single integer.
///
/// Internally, arbitrary values outside the valid range of Bot API Dialog ID
/// may be used to represent special peer identifiers.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PeerId(i64);

/// Witness to the session's authority from Telegram to interact with a peer.
///
/// If Telegram deems the session to already have such authority, the session may
/// be allowed to present [`PeerAuth::default`] instead of this witness. This can
/// happen when the logged-in user is a bot account, or, for user accounts, when
/// the peer being interacted with is one of its contacts.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PeerAuth(i64);

/// Ocap-style reference to a peer object, a peer object capability, bundling the identity
/// of a peer (its [`PeerId`]) with authority over it (as [`PeerAuth`]), to allow fluent use.
///
/// This type implements conversion to [`tl::enums::InputPeer`] and derivatives.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PeerRef {
    /// The peer identity.
    pub id: PeerId,
    /// The authority bound to both the sibling identity and the session of the logged-in user.
    pub auth: PeerAuth,
}

/// [`PeerId`]'s kind.
///
/// The `PeerId` bitpacks this information for size reasons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PeerKind {
    /// The peer identity belongs to a [`tl::enums::User`]. May also represent [`PeerKind::UserSelf`].
    User,
    /// The peer identity belongs to a user with its [`tl::types::User::is_self`] flag set to `true`.
    UserSelf,
    /// The peer identity belongs to a [`tl::types::Chat`] or one of its derivatives.
    Chat,
    /// The peer identity belongs to a [`tl::types::Channel`] or one of its derivatives.
    Channel,
}

/// An exploded peer reference along with any known useful information about the peer.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PeerInfo {
    User {
        /// Bare user identifier.
        ///
        /// Despite being `i64`, Telegram only uses strictly positive values.
        id: i64,
        /// Non-ambient authority bound to both the user itself and the session.
        auth: Option<PeerAuth>,
        /// Whether this user represents a bot or not.
        bot: Option<bool>,
        /// Whether this user represents the logged-in user authorized by this session or not.
        is_self: Option<bool>,
    },
    Chat {
        /// Bare chat identifier.
        ///
        /// Note that the HTTP Bot API negates this identifier to signal that it is a chat,
        /// but the true value used by Telegram's API is always strictly-positive.
        id: i64,
    },
    Channel {
        /// Bare channel identifier.
        ///
        /// Note that the HTTP Bot API prefixes this identifier with `-100` to signal that it is a channel,
        /// but the true value used by Telegram's API is always strictly-positive.
        id: i64,
        /// Non-ambient authority bound to both the user itself and the session.
        auth: Option<PeerAuth>,
        /// Channel kind, useful to determine what the possible permissions on it are.
        kind: Option<ChannelKind>,
    },
}

/// Additional information about a [`PeerInfo::Channel`].
///
/// A non-zero enum,
/// to make working with `Option<ChannelKind>`
/// and `Result<ChannelKind, ()>`
/// slightly more performant.
/// (See [`mod@core::option`]'s documentation about the "null pointer optimization".)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChannelKind {
    /// Value used for a channel with its [`tl::types::Channel::broadcast`] flag set to `true`.
    Broadcast = 1,
    /// Value used for a channel with its [`tl::types::Channel::megagroup`] flag set to `true`.
    Megagroup,
    /// Value used for a channel with its [`tl::types::Channel::gigagroup`] flag set to `true`.
    Gigagroup,
}

/// Sentinel value used to represent the self-user
/// when its true `PeerId` is unknown.
///
/// Per <https://core.telegram.org/api/bots/ids>:
/// > a bot API dialog ID ranges from -4000000000000 to 1099511627775
///
/// This value is not intended to be visible or persisted,
/// so it can be changed as needed in the future.
const SELF_USER_ID: PeerId = PeerId(1 << 40);

/// Sentinel value used to represent empty chats.
///
/// Per <https://core.telegram.org/api/bots/ids>:
/// > \[…] transformed range for bot API chat dialog IDs is -999999999999 to -1 inclusively
/// >
/// > \[…] transformed range for bot API channel dialog IDs is -1997852516352 to -1000000000001 inclusively
///
/// `chat_id` parameters are in Telegram's API use the bare identifier,
/// so there's no empty constructor,
/// but it can be mimicked by picking the value in the correct range hole.
/// This value is closer to "channel with ID 0" than "chat with ID 0",
/// but there's no distinct `-0` integer,
/// and channels have a proper constructor for empty already.
const EMPTY_CHAT_ID: i64 = -1000000000000;

impl PeerId {
    /// Creates a peer identity for the currently-logged-in user or bot account.
    ///
    /// Internally, this will use a special sentinel value outside of any valid Bot API Dialog ID range.
    pub fn self_user() -> Self {
        SELF_USER_ID
    }

    /// Creates a peer identity for a user or bot account.
    pub fn user(id: i64) -> Self {
        // https://core.telegram.org/api/bots/ids#user-ids
        if !(1 <= id && id <= 0xffffffffff) {
            panic!("user ID out of range");
        }

        Self(id)
    }

    /// Creates a peer identity for a small group chat.
    pub fn chat(id: i64) -> Self {
        // https://core.telegram.org/api/bots/ids#chat-ids
        if !(1 <= id && id <= 999999999999) {
            panic!("chat ID out of range");
        }

        Self(-id)
    }

    /// Creates a peer identity for a broadcast channel, megagroup, gigagroup or monoforum.
    pub fn channel(id: i64) -> Self {
        // https://core.telegram.org/api/bots/ids#supergroup-channel-ids and #monoforum-ids
        if !((1 <= id && id <= 997852516352) || (1002147483649 <= id && id <= 3000000000000)) {
            panic!("channel ID out of range");
        }

        Self(-(1000000000000 + id))
    }

    /// Peer kind.
    pub fn kind(self) -> PeerKind {
        if 1 <= self.0 && self.0 <= 0xffffffffff {
            PeerKind::User
        } else if self.0 == SELF_USER_ID.0 {
            PeerKind::UserSelf
        } else if -999999999999 <= self.0 && self.0 <= -1 {
            PeerKind::Chat
        } else if -1997852516352 <= self.0 && self.0 <= -1000000000001
            || (-4000000000000 <= self.0 && self.0 <= -2002147483649)
        {
            PeerKind::Channel
        } else {
            unreachable!()
        }
    }

    /// Returns the identity using the Bot API Dialog ID format.
    ///
    /// Will return an arbitrary value if [`Self::kind`] is [`PeerKind::UserSelf`].
    /// This value should not be relied on and may change between releases.
    pub fn bot_api_dialog_id(&self) -> i64 {
        self.0
    }

    /// Unpacked peer identifier. Panics if [`Self::kind`] is [`PeerKind::UserSelf`].
    pub fn bare_id(&self) -> i64 {
        match self.kind() {
            PeerKind::User => self.0,
            PeerKind::UserSelf => panic!("self-user ID not known"),
            PeerKind::Chat => -self.0,
            PeerKind::Channel => -self.0 - 1000000000000,
        }
    }
}

impl PeerAuth {
    /// Construct a new peer authentication using Telegram's `access_hash` value.
    pub fn from_hash(access_hash: i64) -> Self {
        PeerAuth(access_hash)
    }

    /// Grants access to the internal access hash.
    pub fn hash(&self) -> i64 {
        self.0
    }
}

impl Default for PeerAuth {
    /// Returns the ambient authority to authorize peers only when Telegram considers it valid.
    ///
    /// The internal representation uses `0` to signal the ambient authority,
    /// although this might happen to be the actual witness used by some peers.
    fn default() -> Self {
        Self(0)
    }
}

impl PeerInfo {
    /// Returns the `PeerId` represented by this info.
    ///
    /// The returned [`PeerId::kind()`] will never be [`PeerKind::UserSelf`].
    pub fn id(&self) -> PeerId {
        match self {
            PeerInfo::User { id, .. } => PeerId::user(*id),
            PeerInfo::Chat { id } => PeerId::chat(*id),
            PeerInfo::Channel { id, .. } => PeerId::channel(*id),
        }
    }

    /// Returns the `PeerAuth` stored in this info.
    pub fn auth(&self) -> Option<PeerAuth> {
        match self {
            PeerInfo::User { auth, .. } => *auth,
            PeerInfo::Chat { .. } => Some(PeerAuth::default()),
            PeerInfo::Channel { auth, .. } => *auth,
        }
    }
}

impl fmt::Display for PeerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.bot_api_dialog_id().fmt(f)
    }
}

impl From<tl::enums::Peer> for PeerId {
    #[inline]
    fn from(peer: tl::enums::Peer) -> Self {
        <Self as From<&tl::enums::Peer>>::from(&peer)
    }
}
impl<'a> From<&'a tl::enums::Peer> for PeerId {
    fn from(peer: &'a tl::enums::Peer) -> Self {
        use tl::enums::Peer;
        match peer {
            Peer::User(user) => Self::from(user),
            Peer::Chat(chat) => Self::from(chat),
            Peer::Channel(channel) => Self::from(channel),
        }
    }
}

impl From<tl::types::PeerUser> for PeerId {
    #[inline]
    fn from(user: tl::types::PeerUser) -> Self {
        <Self as From<&tl::types::PeerUser>>::from(&user)
    }
}
impl<'a> From<&'a tl::types::PeerUser> for PeerId {
    fn from(user: &'a tl::types::PeerUser) -> Self {
        Self::user(user.user_id)
    }
}

impl From<tl::types::PeerChat> for PeerId {
    #[inline]
    fn from(user: tl::types::PeerChat) -> Self {
        <Self as From<&tl::types::PeerChat>>::from(&user)
    }
}
impl<'a> From<&'a tl::types::PeerChat> for PeerId {
    fn from(chat: &'a tl::types::PeerChat) -> Self {
        Self::chat(chat.chat_id)
    }
}

impl From<tl::types::PeerChannel> for PeerId {
    #[inline]
    fn from(channel: tl::types::PeerChannel) -> Self {
        <Self as From<&tl::types::PeerChannel>>::from(&channel)
    }
}
impl<'a> From<&'a tl::types::PeerChannel> for PeerId {
    fn from(channel: &'a tl::types::PeerChannel) -> Self {
        Self::channel(channel.channel_id)
    }
}

impl From<tl::enums::InputPeer> for PeerRef {
    #[inline]
    fn from(peer: tl::enums::InputPeer) -> Self {
        <Self as From<&tl::enums::InputPeer>>::from(&peer)
    }
}
impl<'a> From<&'a tl::enums::InputPeer> for PeerRef {
    fn from(peer: &'a tl::enums::InputPeer) -> Self {
        use tl::{enums::InputPeer, types::InputPeerSelf};
        match peer {
            InputPeer::Empty => panic!("InputPeer::Empty cannot be converted to any Peer"),
            InputPeer::PeerSelf => <Self as From<&'a _>>::from(&InputPeerSelf {}),
            InputPeer::User(user) => <Self as From<&'a _>>::from(user),
            InputPeer::Chat(chat) => <Self as From<&'a _>>::from(chat),
            InputPeer::Channel(channel) => <Self as From<&'a _>>::from(channel),
            InputPeer::UserFromMessage(user) => <Self as From<&'a _>>::from(user.deref()),
            InputPeer::ChannelFromMessage(channel) => <Self as From<&'a _>>::from(channel.deref()),
        }
    }
}

impl From<tl::types::InputPeerSelf> for PeerRef {
    #[inline]
    fn from(self_user: tl::types::InputPeerSelf) -> Self {
        <Self as From<&tl::types::InputPeerSelf>>::from(&self_user)
    }
}
impl<'a> From<&'a tl::types::InputPeerSelf> for PeerRef {
    fn from(self_user: &'a tl::types::InputPeerSelf) -> Self {
        _ = self_user;
        Self {
            id: SELF_USER_ID,
            auth: PeerAuth::default(),
        }
    }
}

impl From<tl::types::InputPeerUser> for PeerRef {
    #[inline]
    fn from(user: tl::types::InputPeerUser) -> Self {
        <Self as From<&tl::types::InputPeerUser>>::from(&user)
    }
}
impl<'a> From<&'a tl::types::InputPeerUser> for PeerRef {
    fn from(user: &'a tl::types::InputPeerUser) -> Self {
        Self {
            id: PeerId::user(user.user_id),
            auth: PeerAuth::from_hash(user.access_hash),
        }
    }
}

impl From<tl::types::InputPeerChat> for PeerRef {
    #[inline]
    fn from(chat: tl::types::InputPeerChat) -> Self {
        <Self as From<&tl::types::InputPeerChat>>::from(&chat)
    }
}
impl<'a> From<&'a tl::types::InputPeerChat> for PeerRef {
    fn from(chat: &'a tl::types::InputPeerChat) -> Self {
        Self {
            id: PeerId::chat(chat.chat_id),
            auth: PeerAuth::default(),
        }
    }
}

impl From<tl::types::InputPeerChannel> for PeerRef {
    #[inline]
    fn from(channel: tl::types::InputPeerChannel) -> Self {
        <Self as From<&tl::types::InputPeerChannel>>::from(&channel)
    }
}
impl<'a> From<&'a tl::types::InputPeerChannel> for PeerRef {
    fn from(channel: &'a tl::types::InputPeerChannel) -> Self {
        Self {
            id: PeerId::channel(channel.channel_id),
            auth: PeerAuth::from_hash(channel.access_hash),
        }
    }
}

impl From<tl::types::InputPeerUserFromMessage> for PeerRef {
    #[inline]
    fn from(user: tl::types::InputPeerUserFromMessage) -> Self {
        <Self as From<&tl::types::InputPeerUserFromMessage>>::from(&user)
    }
}
impl<'a> From<&'a tl::types::InputPeerUserFromMessage> for PeerRef {
    fn from(user: &'a tl::types::InputPeerUserFromMessage) -> Self {
        // Not currently willing to make PeerRef significantly larger to accomodate for this uncommon type.
        Self {
            id: PeerId::user(user.user_id),
            auth: PeerAuth::default(),
        }
    }
}

impl From<tl::types::InputPeerChannelFromMessage> for PeerRef {
    #[inline]
    fn from(channel: tl::types::InputPeerChannelFromMessage) -> Self {
        <Self as From<&tl::types::InputPeerChannelFromMessage>>::from(&channel)
    }
}
impl<'a> From<&'a tl::types::InputPeerChannelFromMessage> for PeerRef {
    fn from(channel: &'a tl::types::InputPeerChannelFromMessage) -> Self {
        // Not currently willing to make PeerRef significantly larger to accomodate for this uncommon type.
        Self {
            id: PeerId::channel(channel.channel_id),
            auth: PeerAuth::default(),
        }
    }
}

impl From<tl::enums::User> for PeerRef {
    #[inline]
    fn from(user: tl::enums::User) -> Self {
        <Self as From<&tl::enums::User>>::from(&user)
    }
}
impl<'a> From<&'a tl::enums::User> for PeerRef {
    fn from(user: &'a tl::enums::User) -> Self {
        use tl::enums::User;
        match user {
            User::Empty(user) => <Self as From<&_>>::from(user),
            User::User(user) => <Self as From<&_>>::from(user),
        }
    }
}

impl From<tl::types::UserEmpty> for PeerRef {
    #[inline]
    fn from(user: tl::types::UserEmpty) -> Self {
        <Self as From<&tl::types::UserEmpty>>::from(&user)
    }
}
impl<'a> From<&'a tl::types::UserEmpty> for PeerRef {
    fn from(user: &'a tl::types::UserEmpty) -> Self {
        Self {
            id: PeerId::user(user.id),
            auth: PeerAuth::default(),
        }
    }
}

impl From<tl::types::User> for PeerRef {
    #[inline]
    fn from(user: tl::types::User) -> Self {
        <Self as From<&tl::types::User>>::from(&user)
    }
}
impl<'a> From<&'a tl::types::User> for PeerRef {
    fn from(user: &'a tl::types::User) -> Self {
        Self {
            id: if user.is_self {
                PeerId::self_user()
            } else {
                PeerId::user(user.id)
            },
            auth: user
                .access_hash
                .map(PeerAuth::from_hash)
                .unwrap_or(PeerAuth::default()),
        }
    }
}

impl From<tl::enums::Chat> for PeerRef {
    #[inline]
    fn from(chat: tl::enums::Chat) -> Self {
        <Self as From<&tl::enums::Chat>>::from(&chat)
    }
}
impl<'a> From<&'a tl::enums::Chat> for PeerRef {
    fn from(chat: &'a tl::enums::Chat) -> Self {
        use tl::enums::Chat;
        match chat {
            Chat::Empty(chat) => <Self as From<&_>>::from(chat),
            Chat::Chat(chat) => <Self as From<&_>>::from(chat),
            Chat::Forbidden(chat) => <Self as From<&_>>::from(chat),
            Chat::Channel(channel) => <Self as From<&_>>::from(channel),
            Chat::ChannelForbidden(channel) => <Self as From<&_>>::from(channel),
        }
    }
}

impl From<tl::types::ChatEmpty> for PeerRef {
    #[inline]
    fn from(chat: tl::types::ChatEmpty) -> Self {
        <Self as From<&tl::types::ChatEmpty>>::from(&chat)
    }
}
impl<'a> From<&'a tl::types::ChatEmpty> for PeerRef {
    fn from(chat: &'a tl::types::ChatEmpty) -> Self {
        Self {
            id: PeerId::chat(chat.id),
            auth: PeerAuth::default(),
        }
    }
}

impl From<tl::types::Chat> for PeerRef {
    #[inline]
    fn from(chat: tl::types::Chat) -> Self {
        <Self as From<&tl::types::Chat>>::from(&chat)
    }
}
impl<'a> From<&'a tl::types::Chat> for PeerRef {
    fn from(chat: &'a tl::types::Chat) -> Self {
        Self {
            id: PeerId::chat(chat.id),
            auth: PeerAuth::default(),
        }
    }
}

impl From<tl::types::ChatForbidden> for PeerRef {
    #[inline]
    fn from(chat: tl::types::ChatForbidden) -> Self {
        <Self as From<&tl::types::ChatForbidden>>::from(&chat)
    }
}
impl<'a> From<&'a tl::types::ChatForbidden> for PeerRef {
    fn from(chat: &'a tl::types::ChatForbidden) -> Self {
        Self {
            id: PeerId::chat(chat.id),
            auth: PeerAuth::default(),
        }
    }
}

impl From<tl::types::Channel> for PeerRef {
    #[inline]
    fn from(channel: tl::types::Channel) -> Self {
        <Self as From<&tl::types::Channel>>::from(&channel)
    }
}
impl<'a> From<&'a tl::types::Channel> for PeerRef {
    fn from(channel: &'a tl::types::Channel) -> Self {
        Self {
            id: PeerId::channel(channel.id),
            auth: channel
                .access_hash
                .map(PeerAuth::from_hash)
                .unwrap_or(PeerAuth::default()),
        }
    }
}

impl From<tl::types::ChannelForbidden> for PeerRef {
    #[inline]
    fn from(channel: tl::types::ChannelForbidden) -> Self {
        <Self as From<&tl::types::ChannelForbidden>>::from(&channel)
    }
}
impl<'a> From<&'a tl::types::ChannelForbidden> for PeerRef {
    fn from(channel: &'a tl::types::ChannelForbidden) -> Self {
        Self {
            id: PeerId::channel(channel.id),
            auth: PeerAuth::from_hash(channel.access_hash),
        }
    }
}

impl From<PeerId> for tl::enums::Peer {
    #[inline]
    fn from(peer: PeerId) -> Self {
        <Self as From<&PeerId>>::from(&peer)
    }
}
impl<'a> From<&'a PeerId> for tl::enums::Peer {
    fn from(peer: &'a PeerId) -> Self {
        match peer.kind() {
            PeerKind::User => Self::User(tl::types::PeerUser {
                user_id: peer.bare_id(),
            }),
            PeerKind::UserSelf => panic!("self-user ID not known"),
            PeerKind::Chat => Self::Chat(tl::types::PeerChat {
                chat_id: peer.bare_id(),
            }),
            PeerKind::Channel => Self::Channel(tl::types::PeerChannel {
                channel_id: peer.bare_id(),
            }),
        }
    }
}

impl From<PeerRef> for tl::enums::InputPeer {
    #[inline]
    fn from(peer: PeerRef) -> Self {
        <Self as From<&PeerRef>>::from(&peer)
    }
}
impl<'a> From<&'a PeerRef> for tl::enums::InputPeer {
    fn from(peer: &'a PeerRef) -> Self {
        match peer.id.kind() {
            PeerKind::User => Self::User(tl::types::InputPeerUser {
                user_id: peer.id.bare_id(),
                access_hash: peer.auth.hash(),
            }),
            PeerKind::UserSelf => Self::PeerSelf,
            PeerKind::Chat => Self::Chat(tl::types::InputPeerChat {
                chat_id: peer.id.bare_id(),
            }),
            PeerKind::Channel => Self::Channel(tl::types::InputPeerChannel {
                channel_id: peer.id.bare_id(),
                access_hash: peer.auth.hash(),
            }),
        }
    }
}

impl From<PeerRef> for tl::enums::InputUser {
    #[inline]
    fn from(peer: PeerRef) -> Self {
        <Self as From<&PeerRef>>::from(&peer)
    }
}
impl<'a> From<&'a PeerRef> for tl::enums::InputUser {
    fn from(peer: &'a PeerRef) -> Self {
        match peer.id.kind() {
            PeerKind::User => Self::User(tl::types::InputUser {
                user_id: peer.id.bare_id(),
                access_hash: peer.auth.hash(),
            }),
            PeerKind::UserSelf => Self::UserSelf,
            PeerKind::Chat => Self::Empty,
            PeerKind::Channel => Self::Empty,
        }
    }
}

impl From<PeerRef> for i64 {
    #[inline]
    fn from(peer: PeerRef) -> Self {
        <Self as From<&PeerRef>>::from(&peer)
    }
}
impl<'a> From<&'a PeerRef> for i64 {
    fn from(peer: &'a PeerRef) -> Self {
        match peer.id.kind() {
            PeerKind::User => EMPTY_CHAT_ID,
            PeerKind::UserSelf => EMPTY_CHAT_ID,
            PeerKind::Chat => peer.id.bare_id(),
            PeerKind::Channel => EMPTY_CHAT_ID,
        }
    }
}

impl From<PeerRef> for tl::enums::InputChannel {
    #[inline]
    fn from(peer: PeerRef) -> Self {
        <Self as From<&PeerRef>>::from(&peer)
    }
}
impl<'a> From<&'a PeerRef> for tl::enums::InputChannel {
    fn from(peer: &'a PeerRef) -> Self {
        match peer.id.kind() {
            PeerKind::User => Self::Empty,
            PeerKind::UserSelf => Self::Empty,
            PeerKind::Chat => Self::Empty,
            PeerKind::Channel => Self::Channel(tl::types::InputChannel {
                channel_id: peer.id.bare_id(),
                access_hash: peer.auth.hash(),
            }),
        }
    }
}

impl From<tl::enums::Chat> for PeerInfo {
    #[inline]
    fn from(chat: tl::enums::Chat) -> Self {
        <Self as From<&tl::enums::Chat>>::from(&chat)
    }
}
impl<'a> From<&'a tl::enums::Chat> for PeerInfo {
    fn from(chat: &'a tl::enums::Chat) -> Self {
        match chat {
            tl::enums::Chat::Chat(chat) => <Self as From<&tl::types::Chat>>::from(&chat),
            tl::enums::Chat::Empty(chat) => <Self as From<&tl::types::ChatEmpty>>::from(&chat),
            tl::enums::Chat::Forbidden(chat) => {
                <Self as From<&tl::types::ChatForbidden>>::from(&chat)
            }
            tl::enums::Chat::Channel(channel) => {
                <Self as From<&tl::types::Channel>>::from(&channel)
            }
            tl::enums::Chat::ChannelForbidden(channel) => {
                <Self as From<&tl::types::ChannelForbidden>>::from(&channel)
            }
        }
    }
}

impl From<tl::enums::User> for PeerInfo {
    #[inline]
    fn from(user: tl::enums::User) -> Self {
        <Self as From<&tl::enums::User>>::from(&user)
    }
}
impl<'a> From<&'a tl::enums::User> for PeerInfo {
    fn from(user: &'a tl::enums::User) -> Self {
        match user {
            tl::enums::User::User(user) => <Self as From<&tl::types::User>>::from(&user),
            tl::enums::User::Empty(user) => <Self as From<&tl::types::UserEmpty>>::from(&user),
        }
    }
}

impl From<tl::types::User> for PeerInfo {
    #[inline]
    fn from(user: tl::types::User) -> Self {
        <Self as From<&tl::types::User>>::from(&user)
    }
}
impl<'a> From<&'a tl::types::User> for PeerInfo {
    fn from(user: &'a tl::types::User) -> Self {
        Self::User {
            id: user.id,
            auth: user.access_hash.map(PeerAuth),
            bot: Some(user.bot),
            is_self: Some(user.is_self),
        }
    }
}

impl From<tl::types::UserEmpty> for PeerInfo {
    #[inline]
    fn from(user: tl::types::UserEmpty) -> Self {
        <Self as From<&tl::types::UserEmpty>>::from(&user)
    }
}
impl<'a> From<&'a tl::types::UserEmpty> for PeerInfo {
    fn from(user: &'a tl::types::UserEmpty) -> Self {
        Self::User {
            id: user.id,
            auth: None,
            bot: None,
            is_self: None,
        }
    }
}

impl From<tl::types::Chat> for PeerInfo {
    #[inline]
    fn from(chat: tl::types::Chat) -> Self {
        <Self as From<&tl::types::Chat>>::from(&chat)
    }
}
impl<'a> From<&'a tl::types::Chat> for PeerInfo {
    fn from(chat: &'a tl::types::Chat) -> Self {
        Self::Chat { id: chat.id }
    }
}

impl From<tl::types::ChatEmpty> for PeerInfo {
    #[inline]
    fn from(chat: tl::types::ChatEmpty) -> Self {
        <Self as From<&tl::types::ChatEmpty>>::from(&chat)
    }
}
impl<'a> From<&'a tl::types::ChatEmpty> for PeerInfo {
    fn from(chat: &'a tl::types::ChatEmpty) -> Self {
        Self::Chat { id: chat.id }
    }
}

impl From<tl::types::ChatForbidden> for PeerInfo {
    #[inline]
    fn from(chat: tl::types::ChatForbidden) -> Self {
        <Self as From<&tl::types::ChatForbidden>>::from(&chat)
    }
}
impl<'a> From<&'a tl::types::ChatForbidden> for PeerInfo {
    fn from(chat: &'a tl::types::ChatForbidden) -> Self {
        Self::Chat { id: chat.id }
    }
}

impl From<tl::types::Channel> for PeerInfo {
    #[inline]
    fn from(channel: tl::types::Channel) -> Self {
        <Self as From<&tl::types::Channel>>::from(&channel)
    }
}
impl<'a> From<&'a tl::types::Channel> for PeerInfo {
    fn from(channel: &'a tl::types::Channel) -> Self {
        Self::Channel {
            id: channel.id,
            auth: channel.access_hash.map(PeerAuth),
            kind: <ChannelKind as TryFrom<&'a tl::types::Channel>>::try_from(channel).ok(),
        }
    }
}

impl From<tl::types::ChannelForbidden> for PeerInfo {
    #[inline]
    fn from(channel: tl::types::ChannelForbidden) -> Self {
        <Self as From<&tl::types::ChannelForbidden>>::from(&channel)
    }
}
impl<'a> From<&'a tl::types::ChannelForbidden> for PeerInfo {
    fn from(channel: &'a tl::types::ChannelForbidden) -> Self {
        Self::Channel {
            id: channel.id,
            auth: Some(PeerAuth(channel.access_hash)),
            kind: <ChannelKind as TryFrom<&'a tl::types::ChannelForbidden>>::try_from(channel).ok(),
        }
    }
}

impl TryFrom<tl::types::Channel> for ChannelKind {
    type Error = <ChannelKind as TryFrom<&'static tl::types::Channel>>::Error;

    #[inline]
    fn try_from(channel: tl::types::Channel) -> Result<Self, Self::Error> {
        <ChannelKind as TryFrom<&tl::types::Channel>>::try_from(&channel)
    }
}
impl<'a> TryFrom<&'a tl::types::Channel> for ChannelKind {
    type Error = ();

    fn try_from(channel: &'a tl::types::Channel) -> Result<Self, Self::Error> {
        match channel {
            channel if channel.gigagroup => Ok(Self::Gigagroup),
            channel if channel.broadcast => Ok(Self::Broadcast),
            channel if channel.megagroup => Ok(Self::Megagroup),
            _channel => Err(()),
        }
    }
}

impl TryFrom<tl::types::ChannelForbidden> for ChannelKind {
    type Error = <ChannelKind as TryFrom<&'static tl::types::ChannelForbidden>>::Error;

    #[inline]
    fn try_from(channel: tl::types::ChannelForbidden) -> Result<Self, Self::Error> {
        <ChannelKind as TryFrom<&tl::types::ChannelForbidden>>::try_from(&channel)
    }
}
impl<'a> TryFrom<&'a tl::types::ChannelForbidden> for ChannelKind {
    type Error = ();

    fn try_from(channel: &'a tl::types::ChannelForbidden) -> Result<Self, Self::Error> {
        match channel {
            // channel if channel.gigagroup => Ok(Self::Gigagroup),
            channel if channel.broadcast => Ok(Self::Broadcast),
            channel if channel.megagroup => Ok(Self::Megagroup),
            _channel => Err(()),
        }
    }
}
