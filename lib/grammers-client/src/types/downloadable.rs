// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

#[derive(Clone, Debug, PartialEq)]
pub struct UserProfilePhoto {
    pub big: bool,
    pub peer: tl::enums::InputPeer,
    pub photo: tl::types::UserProfilePhoto,
}

impl UserProfilePhoto {
    fn to_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        Some(tl::enums::InputFileLocation::InputPeerPhotoFileLocation(
            tl::types::InputPeerPhotoFileLocation {
                big: self.big,
                peer: self.peer.clone(),
                photo_id: self.photo.photo_id,
            },
        ))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChatPhoto {
    pub big: bool,
    pub peer: tl::enums::InputPeer,
    pub photo: tl::types::ChatPhoto,
}

impl ChatPhoto {
    fn to_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        Some(tl::enums::InputFileLocation::InputPeerPhotoFileLocation(
            tl::types::InputPeerPhotoFileLocation {
                big: self.big,
                peer: self.peer.clone(),
                photo_id: self.photo.photo_id,
            },
        ))
    }
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Downloadable {
    Media(crate::types::Media),
    UserProfilePhoto(UserProfilePhoto),
    ChatPhoto(ChatPhoto),
}

impl Downloadable {
    pub(crate) fn to_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        match self {
            Self::Media(media) => media.to_input_location(),
            Self::UserProfilePhoto(user_profile_photo) => user_profile_photo.to_input_location(),
            Self::ChatPhoto(chat_photo) => chat_photo.to_input_location(),
        }
    }
}
