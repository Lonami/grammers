use grammers_tl_types as tl;

/// A compact opaque peer reference that can be converted as necessary to invoke remote procedure calls.
///
/// You can think of it as the object capability that lets you act upon this object with the authorization contained in it.
///
/// The [`PeerInfo`] cached by the session for this `Peer` may be retrieved via [`crate::Session::peer`].
#[derive(Clone, Copy, Debug)]
pub struct Peer {
    /// Stored in Bot API Dialog ID format for compactness.
    packed_id: i64,
    /// A value of `0` means either "hash not known" or "hash value is zero".
    /// Telegram sometimes ignores the hash, so any arbitrary number works to signal that it is not known.
    hash: i64,
}

/// [`Peer`]'s kind.
///
/// The `Peer` bitpacks this information for size reasons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerKind {
    User,
    UserSelf,
    Chat,
    Channel,
}

/// A peer reference along with any known useful information about the peer.
#[derive(Clone, Debug)]
pub enum PeerInfo {
    User {
        /// User identifier.
        ///
        /// Despite being `i64`, Telegram only uses strictly positive values.
        id: i64,
        /// Access hash bound to both the user itself and the session.
        ///
        /// It cannot be used by other sessions.
        hash: Option<i64>,
        /// Whether this user represents a bot or not.
        bot: Option<bool>,
        /// Whether this user represents the logged-in user authorized by this session or not.
        is_self: Option<bool>,
    },
    Chat {
        /// Chat identifier.
        ///
        /// Note that the HTTP Bot API negates this identifier to signal that it is a chat,
        /// but the true value used by Telegram's API is always strictly-positive.
        id: i64,
    },
    Channel {
        /// Channel identifier.
        ///
        /// Note that the HTTP Bot API prefixes this identifier with `-100` to signal that it is a channel,
        /// but the true value used by Telegram's API is always strictly-positive.
        id: i64,
        /// Access hash bound to both the channel itself and the session.
        ///
        /// It cannot be used by other sessions.
        hash: Option<i64>,
        /// Channel kind, useful to determine what the possible permissions on it are.
        kind: Option<ChannelKind>,
    },
}

/// Additional information about a [`PeerInfo::Channel`].
#[derive(Clone, Debug)]
pub enum ChannelKind {
    Megagroup,
    Broadcast,
    Gigagroup,
}

// Per https://core.telegram.org/api/bots/ids:
// > a bot API dialog ID ranges from -4000000000000 to 1099511627775
//
// So it is enough to pick an arbitrary value outside of that range.
// This value is not intended to be visible or persisted, so it can be changed as needed in the future.
const SELF_USER_ID: i64 = 1 << 40;

// In the API, `chat_id` parameters are the bare ID, so there's no empty constructor.
// Mimic that behaviour by producing a non-existent chat ID instead.
const EMPTY_CHAT_ID: i64 = (1 << 40) + 1;

/// Ambient authentication to authorize peers only when Telegram considers it valid (i.e. user in contacts or bot accounts).
const AMBIENT_AUTH: i64 = 0;

impl Peer {
    /// Creates a peer referencing the currently-logged-in user or bot account with ambient authentication.
    pub fn self_user() -> Self {
        Self {
            packed_id: SELF_USER_ID,
            hash: AMBIENT_AUTH,
        }
    }

    /// Creates a peer reference to a user or bot account with ambient authentication.
    pub fn user(id: i64) -> Self {
        // https://core.telegram.org/api/bots/ids#user-ids
        if !(1 <= id && id <= 0xffffffffff) {
            panic!("user ID out of range");
        }

        Self {
            packed_id: id,
            hash: 0,
        }
    }

    /// Creates a peer reference to a small group chat with ambient authentication.
    pub fn chat(id: i64) -> Self {
        // https://core.telegram.org/api/bots/ids#chat-ids
        if !(1 <= id && id <= 999999999999) {
            panic!("chat ID out of range");
        }

        Self {
            packed_id: -id,
            hash: 0,
        }
    }

    /// Creates a peer reference to a broadcast channel, megagroup or gigagroup with ambient authentication.
    pub fn channel(id: i64) -> Self {
        // https://core.telegram.org/api/bots/ids#supergroup-channel-ids
        if !(1 <= id && id <= 997852516352) {
            panic!("channel ID out of range");
        }

        Self {
            packed_id: -(1000000000000 + id),
            hash: 0,
        }
    }

    /// Replaces the ambient authentication with the given access hash witness.
    /// Telegram will use this value in its authentication process to determine
    /// whether this peer can be interacted with or not.
    pub fn with_auth(mut self, hash: i64) -> Self {
        self.hash = hash;
        self
    }

    /// Peer kind.
    pub fn kind(self) -> PeerKind {
        if 1 <= self.packed_id && self.packed_id <= 0xffffffffff {
            PeerKind::User
        } else if self.packed_id == SELF_USER_ID {
            PeerKind::UserSelf
        } else if -999999999999 <= self.packed_id && self.packed_id <= -1 {
            PeerKind::Chat
        } else if -1997852516352 <= self.packed_id && self.packed_id <= -1000000000001 {
            PeerKind::Channel
        } else {
            unreachable!()
        }
    }

    /// Unpacked peer identifier. Panics if [`Self::kind`] is [`PeerKind::UserSelf`].
    pub fn id(&self) -> i64 {
        match self.kind() {
            PeerKind::User => self.packed_id,
            PeerKind::UserSelf => panic!("self-user ID not known"),
            PeerKind::Chat => -self.packed_id,
            PeerKind::Channel => -self.packed_id - 1000000000000,
        }
    }
}

impl PeerInfo {
    /// Convenience getter around the `id` without the need for matching.
    pub fn id(&self) -> i64 {
        match self {
            PeerInfo::User { id, .. } => *id,
            PeerInfo::Chat { id } => *id,
            PeerInfo::Channel { id, .. } => *id,
        }
    }

    /// Convenience getter around the `hash` without the need for matching.
    pub fn hash(&self) -> Option<i64> {
        match self {
            PeerInfo::User { hash, .. } => *hash,
            PeerInfo::Chat { .. } => None,
            PeerInfo::Channel { hash, .. } => *hash,
        }
    }
}

impl From<PeerInfo> for Peer {
    fn from(peer: PeerInfo) -> Self {
        match peer {
            PeerInfo::User { id, is_self, .. } => {
                if is_self == Some(true) {
                    Peer::self_user()
                } else {
                    Peer::user(id)
                }
            }
            PeerInfo::Chat { id } => Peer::chat(id),
            PeerInfo::Channel { id, .. } => Peer::channel(id),
        }
    }
}

impl From<tl::enums::Peer> for Peer {
    fn from(peer: tl::enums::Peer) -> Self {
        match peer {
            tl::enums::Peer::User(user) => Peer::from(user),
            tl::enums::Peer::Chat(chat) => Peer::from(chat),
            tl::enums::Peer::Channel(channel) => Peer::from(channel),
        }
    }
}

impl From<tl::types::PeerUser> for Peer {
    fn from(user: tl::types::PeerUser) -> Self {
        Peer::user(user.user_id)
    }
}

impl From<tl::types::PeerChat> for Peer {
    fn from(chat: tl::types::PeerChat) -> Self {
        Peer::chat(chat.chat_id)
    }
}

impl From<tl::types::PeerChannel> for Peer {
    fn from(channel: tl::types::PeerChannel) -> Self {
        Peer::channel(channel.channel_id)
    }
}

impl From<tl::enums::InputPeer> for Peer {
    fn from(peer: tl::enums::InputPeer) -> Self {
        match peer {
            tl::enums::InputPeer::Empty => {
                panic!("InputPeer::Empty cannot be converted to any Peer");
            }
            tl::enums::InputPeer::PeerSelf => Peer::self_user(),
            tl::enums::InputPeer::User(user) => Peer::from(user),
            tl::enums::InputPeer::Chat(chat) => Peer::from(chat),
            tl::enums::InputPeer::Channel(channel) => Peer::from(channel),
            tl::enums::InputPeer::UserFromMessage(user) => Peer::from(*user),
            tl::enums::InputPeer::ChannelFromMessage(channel) => Peer::from(*channel),
        }
    }
}

impl From<tl::types::InputPeerSelf> for Peer {
    fn from(_: tl::types::InputPeerSelf) -> Self {
        Peer::self_user()
    }
}

impl From<tl::types::InputPeerUser> for Peer {
    fn from(user: tl::types::InputPeerUser) -> Self {
        Peer::user(user.user_id).with_auth(user.access_hash)
    }
}

impl From<tl::types::InputPeerChat> for Peer {
    fn from(chat: tl::types::InputPeerChat) -> Self {
        Peer::chat(chat.chat_id)
    }
}

impl From<tl::types::InputPeerChannel> for Peer {
    fn from(channel: tl::types::InputPeerChannel) -> Self {
        Peer::channel(channel.channel_id).with_auth(channel.access_hash)
    }
}

impl From<tl::types::InputPeerUserFromMessage> for Peer {
    fn from(user: tl::types::InputPeerUserFromMessage) -> Self {
        // Not currently willing to make Peer significantly larger to accomodate for this uncommon type.
        Peer::user(user.user_id)
    }
}

impl From<tl::types::InputPeerChannelFromMessage> for Peer {
    fn from(channel: tl::types::InputPeerChannelFromMessage) -> Self {
        // Not currently willing to make Peer significantly larger to accomodate for this uncommon type.
        Peer::channel(channel.channel_id)
    }
}

impl From<tl::enums::User> for Peer {
    fn from(user: tl::enums::User) -> Self {
        match user {
            grammers_tl_types::enums::User::Empty(user) => Peer::from(user),
            grammers_tl_types::enums::User::User(user) => Peer::from(user),
        }
    }
}

impl From<tl::types::UserEmpty> for Peer {
    fn from(user: tl::types::UserEmpty) -> Self {
        Peer::user(user.id)
    }
}

impl From<tl::types::User> for Peer {
    fn from(user: tl::types::User) -> Self {
        if user.is_self {
            Peer::self_user()
        } else {
            Peer::user(user.id).with_auth(user.access_hash.unwrap_or(AMBIENT_AUTH))
        }
    }
}

impl From<tl::enums::Chat> for Peer {
    fn from(chat: tl::enums::Chat) -> Self {
        match chat {
            grammers_tl_types::enums::Chat::Empty(chat) => Peer::from(chat),
            grammers_tl_types::enums::Chat::Chat(chat) => Peer::from(chat),
            grammers_tl_types::enums::Chat::Forbidden(chat) => Peer::from(chat),
            grammers_tl_types::enums::Chat::Channel(channel) => Peer::from(channel),
            grammers_tl_types::enums::Chat::ChannelForbidden(channel) => Peer::from(channel),
        }
    }
}

impl From<tl::types::ChatEmpty> for Peer {
    fn from(chat: tl::types::ChatEmpty) -> Self {
        Peer::chat(chat.id)
    }
}

impl From<tl::types::Chat> for Peer {
    fn from(chat: tl::types::Chat) -> Self {
        Peer::chat(chat.id)
    }
}

impl From<tl::types::ChatForbidden> for Peer {
    fn from(chat: tl::types::ChatForbidden) -> Self {
        Peer::chat(chat.id)
    }
}

impl From<tl::types::Channel> for Peer {
    fn from(channel: tl::types::Channel) -> Self {
        Peer::channel(channel.id).with_auth(channel.access_hash.unwrap_or(AMBIENT_AUTH))
    }
}

impl From<tl::types::ChannelForbidden> for Peer {
    fn from(channel: tl::types::ChannelForbidden) -> Self {
        Peer::channel(channel.id).with_auth(channel.access_hash)
    }
}

impl From<Peer> for tl::enums::InputPeer {
    fn from(peer: Peer) -> Self {
        match peer.kind() {
            PeerKind::User => tl::enums::InputPeer::User(tl::types::InputPeerUser {
                user_id: peer.id(),
                access_hash: peer.hash,
            }),
            PeerKind::UserSelf => tl::enums::InputPeer::PeerSelf,
            PeerKind::Chat => {
                tl::enums::InputPeer::Chat(tl::types::InputPeerChat { chat_id: peer.id() })
            }
            PeerKind::Channel => tl::enums::InputPeer::Channel(tl::types::InputPeerChannel {
                channel_id: peer.id(),
                access_hash: peer.hash,
            }),
        }
    }
}

impl From<Peer> for tl::enums::InputUser {
    fn from(peer: Peer) -> Self {
        match peer.kind() {
            PeerKind::User => tl::enums::InputUser::User(tl::types::InputUser {
                user_id: peer.id(),
                access_hash: peer.hash,
            }),
            PeerKind::UserSelf => tl::enums::InputUser::UserSelf,
            PeerKind::Chat => tl::enums::InputUser::Empty,
            PeerKind::Channel => tl::enums::InputUser::Empty,
        }
    }
}

impl From<Peer> for i64 {
    fn from(peer: Peer) -> Self {
        match peer.kind() {
            PeerKind::User => EMPTY_CHAT_ID,
            PeerKind::UserSelf => EMPTY_CHAT_ID,
            PeerKind::Chat => peer.id(),
            PeerKind::Channel => EMPTY_CHAT_ID,
        }
    }
}

impl From<Peer> for tl::enums::InputChannel {
    fn from(peer: Peer) -> Self {
        match peer.kind() {
            PeerKind::User => tl::enums::InputChannel::Empty,
            PeerKind::UserSelf => tl::enums::InputChannel::Empty,
            PeerKind::Chat => tl::enums::InputChannel::Empty,
            PeerKind::Channel => tl::enums::InputChannel::Channel(tl::types::InputChannel {
                channel_id: -peer.packed_id - 1000000000000,
                access_hash: peer.hash,
            }),
        }
    }
}
