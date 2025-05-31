// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

slint::include_modules!();

#[cfg(test)]
use arbtest::arbtest;

use grammers_session::types::{
    // PeerAuth,
    PeerId,
    PeerKind,
    // PeerRef,
};

impl From<PeerKind> for ChatKindEnum {
    fn from(value: PeerKind) -> Self {
        match value {
            PeerKind::User => Self::User,
            PeerKind::UserSelf => Self::UserSelf,
            PeerKind::Chat => Self::Chat,
            PeerKind::Channel => Self::Channel,
        }
    }
}

impl From<ChatKindEnum> for PeerKind {
    fn from(value: ChatKindEnum) -> Self {
        match value {
            ChatKindEnum::User => Self::User,
            ChatKindEnum::UserSelf => Self::UserSelf,
            ChatKindEnum::Chat => Self::Chat,
            ChatKindEnum::Channel => Self::Channel,
        }
    }
}

impl ChatId {
    pub fn new(kind: ChatKindEnum, id: i64) -> Self {
        Self {
            kind,
            id_hi: ((id.cast_unsigned() >> 32) as u32).cast_signed(),
            id_lo: (id.cast_unsigned() as u32).cast_signed(),
        }
    }

    pub fn id(&self) -> i64 {
        (((self.id_hi.cast_unsigned() as u64) << 32) | (self.id_lo.cast_unsigned() as u64))
            .cast_signed()
    }
}

#[test]
fn chat_id_preserves_id() {
    let property = |u: &mut arbitrary::Unstructured<'_>| -> arbitrary::Result<()> {
        // let kind: ChatKindEnum = u.arbitrary()?;
        let kind: ChatKindEnum = ChatKindEnum::User;
        let id: i64 = u.arbitrary()?;
        let chat_id = ChatId::new(kind, id);
        assert_eq!(chat_id.id(), id, "ChatId::new(_kind, id).id() != id");
        Ok(())
    };
    arbtest(property);
}

impl From<PeerId> for ChatId {
    fn from(value: PeerId) -> Self {
        Self::new(value.kind().into(), value.bare_id())
    }
}

impl From<ChatId> for PeerId {
    fn from(value: ChatId) -> Self {
        let id: i64 = value.id();
        match value.kind {
            ChatKindEnum::User => Self::user(id),
            ChatKindEnum::UserSelf => Self::self_user(),
            ChatKindEnum::Chat => Self::chat(id),
            ChatKindEnum::Channel => Self::channel(id),
        }
    }
}

// impl ChatAuth {
//     pub fn new(access_hash: i64) -> Self {
//         Self {
//             access_hash_hi: (access_hash >> 32) as i32,
//             access_hash_lo: access_hash as i32,
//             access_hash_hi: ((access_hash.cast_unsigned() >> 32) as u32).cast_signed(),
//             access_hash_lo: (access_hash.cast_unsigned() as u32).cast_signed(),
//         }
//     }
//
//     pub fn access_hash(&self) -> i64 {
//         ((self.access_hash_hi as i64) << 32)
//             | (self.access_hash_lo as i64)(
//                 ((self.access_hash_hi.cast_unsigned() as u64) << 32)
//                     | (self.access_hash_lo.cast_unsigned() as u64),
//             )
//             .cast_signed()
//     }
// }
//
// impl From<PeerAuth> for ChatAuth {
//     fn from(value: PeerAuth) -> Self {
//         Self::new(value.hash())
//     }
// }
//
// impl From<ChatAuth> for PeerAuth {
//     fn from(value: ChatAuth) -> Self {
//         Self::from_hash(value.access_hash())
//     }
// }

// impl From<PeerRef> for ChatRef {
//     fn from(value: PeerRef) -> Self {
//         Self {
//             id: value.id.into(),
//             auth: value.auth.into(),
//         }
//     }
// }
//
// impl From<ChatRef> for PeerRef {
//     fn from(value: ChatRef) -> Self {
//         Self {
//             id: value.id.into(),
//             auth: value.auth.into(),
//         }
//     }
// }
