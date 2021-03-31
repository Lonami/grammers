// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::types::photo_sizes::PhotoSize;
use crate::ClientHandle;
use chrono::{DateTime, NaiveDateTime, Utc};
use grammers_tl_types as tl;
use std::fmt::Debug;

#[derive(Clone, Debug, PartialEq)]
pub struct Photo {
    photo: tl::types::MessageMediaPhoto,
    client: ClientHandle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    document: tl::types::MessageMediaDocument,
    client: ClientHandle,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sticker {
    pub document: Document,
    attrs: tl::types::DocumentAttributeSticker,
    animated: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Uploaded {
    pub(crate) input_file: tl::enums::InputFile,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Media {
    Photo(Photo),
    Document(Document),
    Sticker(Sticker),
    Uploaded(Uploaded),
}

impl Photo {
    pub(crate) fn from_raw(photo: tl::enums::Photo, client: ClientHandle) -> Self {
        Self {
            photo: tl::types::MessageMediaPhoto {
                photo: Some(photo),
                ttl_seconds: None,
            },
            client,
        }
    }

    pub(crate) fn from_media(photo: tl::types::MessageMediaPhoto, client: ClientHandle) -> Self {
        Self { photo, client }
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

    /// Get photo thumbs.
    ///
    /// Since Telegram doesn't store the original photo, it can be presented in different sizes
    /// and quality, a.k.a. thumbnails. Each photo preview has a specific type, indicating
    /// the resolution and image transform that was applied server-side. Some low-resolution
    /// thumbnails already contain all necessary information that can be shown to the user, but
    /// for other types an additional request to the Telegram should be performed.
    /// Check the description of [PhotoSize] to get an information about each particular thumbnail.
    ///
    /// https://core.telegram.org/api/files#image-thumbnail-types
    pub fn thumbs(&self) -> Vec<PhotoSize> {
        use tl::enums::Photo as P;

        let photo = match self.photo.photo.as_ref() {
            Some(photo) => photo,
            None => return vec![],
        };

        match photo {
            P::Empty(_) => vec![],
            P::Photo(photo) => photo
                .sizes
                .iter()
                .map(|x| PhotoSize::make_from(&x, &photo, self.client.clone()))
                .collect(),
        }
    }
}

impl Document {
    pub(crate) fn from_media(
        document: tl::types::MessageMediaDocument,
        client: ClientHandle,
    ) -> Self {
        Self { document, client }
    }

    fn to_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        use tl::enums::Document as D;

        self.document.document.as_ref().and_then(|p| match p {
            D::Empty(_) => None,
            D::Document(document) => Some(
                tl::types::InputDocumentFileLocation {
                    id: document.id,
                    access_hash: document.access_hash,
                    file_reference: document.file_reference.clone(),
                    thumb_size: String::new(),
                }
                .into(),
            ),
        })
    }

    pub fn id(&self) -> i64 {
        use tl::enums::Document as D;

        match self.document.document.as_ref().unwrap() {
            D::Empty(document) => document.id,
            D::Document(document) => document.id,
        }
    }

    /// Return the file's name.
    ///
    /// If the file was uploaded with no file name, the returned string will be empty.
    pub fn name(&self) -> &str {
        use tl::enums::Document as D;

        match self.document.document.as_ref().unwrap() {
            D::Empty(_) => "",
            D::Document(document) => document
                .attributes
                .iter()
                .find_map(|attr| match attr {
                    tl::enums::DocumentAttribute::Filename(attr) => Some(attr.file_name.as_ref()),
                    _ => None,
                })
                .unwrap_or(""),
        }
    }

    /// Get the file's MIME type, if any.
    pub fn mime_type(&self) -> Option<&str> {
        match self.document.document.as_ref() {
            Some(tl::enums::Document::Document(d)) => Some(d.mime_type.as_str()),
            _ => None,
        }
    }

    /// The date on which the file was created, if any.
    pub fn creation_date(&self) -> Option<DateTime<Utc>> {
        match self.document.document.as_ref() {
            Some(tl::enums::Document::Document(d)) => Some(DateTime::from_utc(
                NaiveDateTime::from_timestamp(d.date as i64, 0),
                Utc,
            )),
            _ => None,
        }
    }

    /// The size of the file.
    /// returns 0 if the document is empty.
    pub fn size(&self) -> i32 {
        match self.document.document.as_ref() {
            Some(tl::enums::Document::Document(d)) => d.size,
            _ => 0,
        }
    }
}

impl Sticker {
    pub(crate) fn from_document(document: &Document) -> Option<Self> {
        match document.document.document {
            Some(tl::enums::Document::Document(ref doc)) => {
                let mut animated = false;
                let mut sticker_attrs: Option<tl::types::DocumentAttributeSticker> = None;
                for attr in &doc.attributes {
                    match attr {
                        tl::enums::DocumentAttribute::Sticker(s) => sticker_attrs = Some(s.clone()),
                        tl::enums::DocumentAttribute::Animated => animated = true,
                        _ => (),
                    }
                }
                Some(Self {
                    document: document.clone(),
                    attrs: sticker_attrs?,
                    animated,
                })
            }
            _ => None,
        }
    }

    /// Get the emoji associated with the sticker.
    pub fn emoji(&self) -> &str {
        return self.attrs.alt.as_str();
    }

    /// Is this sticker an animated sticker?
    pub fn is_animated(&self) -> bool {
        return self.animated;
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
    pub(crate) fn from_raw(media: tl::enums::MessageMedia, client: ClientHandle) -> Option<Self> {
        use tl::enums::MessageMedia as M;

        // TODO implement the rest
        match media {
            M::Empty => None,
            M::Photo(photo) => Some(Self::Photo(Photo::from_media(photo, client))),
            M::Geo(_) => None,
            M::Contact(_) => None,
            M::Unsupported => None,
            M::Document(document) => {
                let document = Document::from_media(document, client);
                Some(if let Some(sticker) = Sticker::from_document(&document) {
                    Self::Sticker(sticker)
                } else {
                    Self::Document(document)
                })
            }
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
            Media::Document(document) => document.to_input_location(),
            Media::Sticker(sticker) => sticker.document.to_input_location(),
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
