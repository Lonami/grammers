// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

#[derive(Clone, Debug, PartialEq)]
pub struct Photo {
    photo: tl::types::MessageMediaPhoto,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Uploaded {
    pub(crate) input_file: tl::enums::InputFile,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Media {
    Photo(Photo),
    Uploaded(Uploaded),
}

impl Photo {
    pub(crate) fn from_raw(photo: tl::enums::Photo) -> Self {
        Self {
            photo: tl::types::MessageMediaPhoto {
                photo: Some(photo),
                ttl_seconds: None,
            },
        }
    }

    pub(crate) fn from_media(photo: tl::types::MessageMediaPhoto) -> Self {
        Self { photo }
    }

    fn to_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        use tl::enums::Photo as P;

        self.photo.photo.as_ref().and_then(|p| match p {
            P::Empty(_) => None,
            P::Photo(photo) => Some(
                tl::types::InputPhotoFileLocation {
                    id: photo.id,
                    access_hash: photo.access_hash,
                    file_reference: photo.file_reference.clone(),
                    thumb_size: String::new(),
                }
                .into(),
            ),
        })
    }

    pub fn id(&self) -> i64 {
        use tl::enums::Photo as P;

        match self.photo.photo.as_ref().unwrap() {
            P::Empty(photo) => photo.id,
            P::Photo(photo) => photo.id,
        }
    }
}

impl Uploaded {
    pub(crate) fn from_raw(input_file: tl::enums::InputFile) -> Self {
        Self { input_file }
    }

    pub(crate) fn name(&self) -> &str {
        match &self.input_file {
            tl::enums::InputFile::File(f) => f.name.as_ref(),
            tl::enums::InputFile::Big(f) => f.name.as_ref(),
        }
    }
}

impl Media {
    pub(crate) fn from_raw(media: tl::enums::MessageMedia) -> Option<Self> {
        use tl::enums::MessageMedia as M;

        // TODO implement the rest
        match media {
            M::Empty => None,
            M::Photo(photo) => Some(Self::Photo(Photo::from_media(photo))),
            M::Geo(_) => None,
            M::Contact(_) => None,
            M::Unsupported => None,
            M::Document(_) => None,
            M::WebPage(_) => None,
            M::Venue(_) => None,
            M::Game(_) => None,
            M::Invoice(_) => None,
            M::GeoLive(_) => None,
            M::Poll(_) => None,
            M::Dice(_) => None,
        }
    }

    pub(crate) fn to_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        match self {
            Media::Photo(photo) => photo.to_input_location(),
            Media::Uploaded(_) => None,
        }
    }
}

impl From<Photo> for Media {
    fn from(photo: Photo) -> Self {
        Self::Photo(photo)
    }
}

impl From<Uploaded> for Media {
    fn from(uploaded: Uploaded) -> Self {
        Self::Uploaded(uploaded)
    }
}
