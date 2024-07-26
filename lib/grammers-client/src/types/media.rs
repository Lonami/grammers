// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::types::photo_sizes::{PhotoSize, VecExt};
use chrono::{DateTime, Utc};
use grammers_tl_types as tl;
use std::fmt::Debug;

#[derive(Clone, Debug, PartialEq)]
pub struct Photo {
    pub raw: tl::types::MessageMediaPhoto,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    pub raw: tl::types::MessageMediaDocument,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Sticker {
    pub document: Document,
    pub raw_attrs: tl::types::DocumentAttributeSticker,
    animated: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Uploaded {
    pub raw: tl::enums::InputFile,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Contact {
    pub raw: tl::types::MessageMediaContact,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Poll {
    pub raw: tl::types::Poll,
    pub raw_results: tl::types::PollResults,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Geo {
    pub raw: tl::types::GeoPoint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dice {
    pub raw: tl::types::MessageMediaDice,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Venue {
    pub geo: Option<Geo>,
    pub raw_venue: tl::types::MessageMediaVenue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoLive {
    pub geo: Option<Geo>,
    pub raw_geolive: tl::types::MessageMediaGeoLive,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WebPage {
    pub raw: tl::types::MessageMediaWebPage,
}

#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Media {
    Photo(Photo),
    Document(Document),
    Sticker(Sticker),
    Contact(Contact),
    Poll(Poll),
    Geo(Geo),
    Dice(Dice),
    Venue(Venue),
    GeoLive(GeoLive),
    WebPage(WebPage),
}

impl Photo {
    pub fn from_raw(photo: tl::enums::Photo) -> Self {
        Self {
            raw: tl::types::MessageMediaPhoto {
                spoiler: false,
                photo: Some(photo),
                ttl_seconds: None,
            },
        }
    }

    pub fn from_raw_media(photo: tl::types::MessageMediaPhoto) -> Self {
        Self { raw: photo }
    }

    pub fn to_raw_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        use tl::enums::Photo as P;

        self.raw.photo.as_ref().and_then(|p| match p {
            P::Empty(_) => None,
            P::Photo(photo) => Some(
                tl::types::InputPhotoFileLocation {
                    id: photo.id,
                    access_hash: photo.access_hash,
                    file_reference: photo.file_reference.clone(),
                    thumb_size: self
                        .thumbs()
                        .largest()
                        .map(|ps| ps.photo_type())
                        .unwrap_or(String::from("w")),
                }
                .into(),
            ),
        })
    }

    pub fn to_raw_input_media(&self) -> tl::types::InputMediaPhoto {
        use tl::{
            enums::{InputPhoto as eInputPhoto, Photo},
            types::InputPhoto,
        };

        tl::types::InputMediaPhoto {
            spoiler: false,
            id: match self.raw.photo {
                Some(Photo::Photo(ref photo)) => InputPhoto {
                    id: photo.id,
                    access_hash: photo.access_hash,
                    file_reference: photo.file_reference.clone(),
                }
                .into(),
                _ => eInputPhoto::Empty,
            },
            ttl_seconds: self.raw.ttl_seconds,
        }
    }

    pub fn id(&self) -> i64 {
        use tl::enums::Photo as P;

        match self.raw.photo.as_ref().unwrap() {
            P::Empty(photo) => photo.id,
            P::Photo(photo) => photo.id,
        }
    }

    /// The size of the photo.
    /// returns 0 if unable to get the size.
    pub fn size(&self) -> i64 {
        match self.thumbs().largest() {
            Some(thumb) => thumb.size() as i64,
            None => 0,
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
    /// <https://core.telegram.org/api/files#image-thumbnail-types>
    pub fn thumbs(&self) -> Vec<PhotoSize> {
        use tl::enums::Photo as P;

        let photo = match self.raw.photo.as_ref() {
            Some(photo) => photo,
            None => return vec![],
        };

        match photo {
            P::Empty(_) => vec![],
            P::Photo(photo) => photo
                .sizes
                .iter()
                .map(|x| PhotoSize::make_from(x, photo))
                .collect(),
        }
    }

    /// Returns true if the photo is a spoiler.
    pub fn is_spoiler(&self) -> bool {
        self.raw.spoiler
    }

    /// Returns TTL seconds if the photo is self-destructive, None otherwise
    pub fn ttl_seconds(&self) -> Option<i32> {
        self.raw.ttl_seconds
    }
}

impl Document {
    pub fn from_raw_media(document: tl::types::MessageMediaDocument) -> Self {
        Self { raw: document }
    }

    pub fn to_raw_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        use tl::enums::Document as D;

        self.raw.document.as_ref().and_then(|p| match p {
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

    pub fn to_raw_input_media(&self) -> tl::types::InputMediaDocument {
        use tl::{
            enums::{Document, InputDocument as eInputDocument},
            types::InputDocument,
        };

        tl::types::InputMediaDocument {
            spoiler: false,
            id: match self.raw.document {
                Some(Document::Document(ref document)) => InputDocument {
                    id: document.id,
                    access_hash: document.access_hash,
                    file_reference: document.file_reference.clone(),
                }
                .into(),
                _ => eInputDocument::Empty,
            },
            ttl_seconds: self.raw.ttl_seconds,
            query: None,
        }
    }

    pub fn id(&self) -> i64 {
        use tl::enums::Document as D;

        match self.raw.document.as_ref().unwrap() {
            D::Empty(document) => document.id,
            D::Document(document) => document.id,
        }
    }

    /// Return the file's name.
    ///
    /// If the file was uploaded with no file name, the returned string will be empty.
    pub fn name(&self) -> &str {
        use tl::enums::Document as D;

        match self.raw.document.as_ref().unwrap() {
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
        match self.raw.document.as_ref() {
            Some(tl::enums::Document::Document(d)) => Some(d.mime_type.as_str()),
            _ => None,
        }
    }

    /// The date on which the file was created, if any.
    pub fn creation_date(&self) -> Option<DateTime<Utc>> {
        match self.raw.document.as_ref() {
            Some(tl::enums::Document::Document(d)) => {
                Some(DateTime::<Utc>::from_timestamp(d.date as i64, 0).expect("date out of range"))
            }
            _ => None,
        }
    }

    /// The size of the file.
    /// returns 0 if the document is empty.
    pub fn size(&self) -> i64 {
        match self.raw.document.as_ref() {
            Some(tl::enums::Document::Document(d)) => d.size,
            _ => 0,
        }
    }

    /// Get document thumbs.
    /// <https://core.telegram.org/api/files#image-thumbnail-types>
    pub fn thumbs(&self) -> Vec<PhotoSize> {
        use tl::enums::Document as D;

        let document = match self.raw.document.as_ref() {
            Some(document) => document,
            None => return vec![],
        };

        match document {
            D::Empty(_) => vec![],
            D::Document(document) => match &document.thumbs {
                Some(thumbs) => thumbs
                    .iter()
                    .map(|x| PhotoSize::make_from_document(x, document))
                    .collect(),
                None => vec![],
            },
        }
    }

    /// Duration of video/audio, in seconds
    pub fn duration(&self) -> Option<f64> {
        match self.raw.document.as_ref() {
            Some(tl::enums::Document::Document(d)) => {
                for attr in &d.attributes {
                    match attr {
                        tl::enums::DocumentAttribute::Video(v) => return Some(v.duration),
                        tl::enums::DocumentAttribute::Audio(a) => return Some(a.duration as _),
                        _ => {}
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Width & height of video/image
    pub fn resolution(&self) -> Option<(i32, i32)> {
        match self.raw.document.as_ref() {
            Some(tl::enums::Document::Document(d)) => {
                for attr in &d.attributes {
                    match attr {
                        tl::enums::DocumentAttribute::Video(v) => return Some((v.w, v.h)),
                        tl::enums::DocumentAttribute::ImageSize(i) => return Some((i.w, i.h)),
                        _ => {}
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Title of audio
    pub fn audio_title(&self) -> Option<String> {
        match self.raw.document.as_ref() {
            Some(tl::enums::Document::Document(d)) => {
                for attr in &d.attributes {
                    #[allow(clippy::single_match)]
                    match attr {
                        tl::enums::DocumentAttribute::Audio(a) => return a.title.clone(),
                        _ => {}
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Performer (artist) of audio
    pub fn performer(&self) -> Option<String> {
        match self.raw.document.as_ref() {
            Some(tl::enums::Document::Document(d)) => {
                for attr in &d.attributes {
                    #[allow(clippy::single_match)]
                    match attr {
                        tl::enums::DocumentAttribute::Audio(a) => return a.performer.clone(),
                        _ => {}
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Returns true if the document is an animated sticker
    pub fn is_animated(&self) -> bool {
        match self.raw.document.as_ref() {
            Some(tl::enums::Document::Document(d)) => {
                for attr in &d.attributes {
                    #[allow(clippy::single_match)]
                    match attr {
                        tl::enums::DocumentAttribute::Animated => return true,
                        _ => {}
                    }
                }
                false
            }
            _ => false,
        }
    }

    /// Returns true if the document is a spoiler
    pub fn is_spoiler(&self) -> bool {
        self.raw.spoiler
    }
}

impl Sticker {
    pub fn from_document(document: &Document) -> Option<Self> {
        match document.raw.document {
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
                    raw_attrs: sticker_attrs?,
                    animated,
                })
            }
            _ => None,
        }
    }

    /// Get the emoji associated with the sticker.
    pub fn emoji(&self) -> &str {
        self.raw_attrs.alt.as_str()
    }

    /// Is this sticker an animated sticker?
    pub fn is_animated(&self) -> bool {
        self.animated
    }
}

impl Contact {
    pub fn from_raw_media(contact: tl::types::MessageMediaContact) -> Self {
        Self { raw: contact }
    }

    pub fn to_raw_input_media(&self) -> tl::types::InputMediaContact {
        tl::types::InputMediaContact {
            phone_number: self.raw.phone_number.clone(),
            first_name: self.raw.first_name.clone(),
            last_name: self.raw.last_name.clone(),
            vcard: self.raw.vcard.clone(),
        }
    }

    /// The contact's phone number, in international format. This field will always be a non-empty
    /// string of digits, although there's no guarantee that the number actually exists.
    pub fn phone_number(&self) -> &str {
        self.raw.phone_number.as_str()
    }

    /// The contact's first name. Although official clients will always send a non-empty string,
    /// it is possible for this field to be empty when sent via different means.
    pub fn first_name(&self) -> &str {
        self.raw.first_name.as_str()
    }

    /// The contact's last name. May be empty if it's not set by sender.
    pub fn last_name(&self) -> &str {
        self.raw.last_name.as_str()
    }

    /// Contact information in [vCard format][1]. Applications such as Telegram Desktop leave this
    /// field empty. The vCard version used in this field could be any. The field may also contain
    /// arbitrary text when sent by non-official clients.
    ///
    /// [1]: https://en.wikipedia.org/wiki/VCard
    pub fn vcard(&self) -> &str {
        self.raw.vcard.as_str()
    }
}

impl Poll {
    pub fn from_raw_media(poll: tl::types::MessageMediaPoll) -> Self {
        Self {
            raw: match poll.poll {
                tl::enums::Poll::Poll(poll) => poll,
            },
            raw_results: match poll.results {
                tl::enums::PollResults::Results(results) => results,
            },
        }
    }

    pub fn to_raw_input_media(&self) -> tl::types::InputMediaPoll {
        tl::types::InputMediaPoll {
            poll: grammers_tl_types::enums::Poll::Poll(self.raw.clone()),
            correct_answers: None,
            solution: None,
            solution_entities: None,
        }
    }

    /// Return question of the poll
    pub fn question(&self) -> &grammers_tl_types::enums::TextWithEntities {
        &self.raw.question
    }

    /// Return if current poll is quiz
    pub fn is_quiz(&self) -> bool {
        self.raw.quiz
    }

    /// Indicator that poll is closed
    pub fn closed(&self) -> bool {
        self.raw.closed
    }

    /// Iterator over poll answer options
    pub fn iter_answers(&self) -> impl Iterator<Item = &tl::types::PollAnswer> {
        self.raw.answers.iter().map(|answer| match answer {
            tl::enums::PollAnswer::Answer(answer) => answer,
        })
    }

    /// Total voters that took part in the vote
    ///
    /// May be None if poll isn't started
    pub fn total_voters(&self) -> Option<i32> {
        self.raw_results.total_voters
    }

    /// Return details of the voters choices:
    /// how much voters chose each answer and wether current option
    pub fn iter_voters_summary(
        &self,
    ) -> Option<impl Iterator<Item = &tl::types::PollAnswerVoters>> {
        self.raw_results.results.as_ref().map(|results| {
            results.iter().map(|result| match result {
                tl::enums::PollAnswerVoters::Voters(voters) => voters,
            })
        })
    }
}

impl Geo {
    pub fn from_raw_media(geo: tl::types::MessageMediaGeo) -> Option<Self> {
        use tl::enums::GeoPoint as eGeoPoint;

        match &geo.geo {
            eGeoPoint::Empty => None,
            eGeoPoint::Point(point) => Some(Self { raw: point.clone() }),
        }
    }

    pub fn to_raw_input_media(&self) -> tl::types::InputMediaGeoPoint {
        use tl::types::InputGeoPoint;

        tl::types::InputMediaGeoPoint {
            geo_point: InputGeoPoint {
                lat: self.raw.lat,
                long: self.raw.long,
                accuracy_radius: self.raw.accuracy_radius,
            }
            .into(),
        }
    }

    pub fn to_raw_input_geo_point(&self) -> tl::enums::InputGeoPoint {
        use tl::{enums::InputGeoPoint as eInputGeoPoint, types::InputGeoPoint};

        eInputGeoPoint::Point(InputGeoPoint {
            lat: self.raw.lat,
            long: self.raw.long,
            accuracy_radius: self.raw.accuracy_radius,
        })
    }

    /// Get the latitude of the location.
    pub fn latitue(&self) -> f64 {
        self.raw.lat
    }

    /// Get the latitude of the location.
    pub fn longitude(&self) -> f64 {
        self.raw.long
    }

    /// Get the accuracy of the geo location in meters.
    pub fn accuracy_radius(&self) -> Option<i32> {
        self.raw.accuracy_radius
    }
}

impl Dice {
    pub fn from_raw_media(dice: tl::types::MessageMediaDice) -> Self {
        Self { raw: dice }
    }

    pub fn to_raw_input_media(&self) -> tl::types::InputMediaDice {
        tl::types::InputMediaDice {
            emoticon: self.raw.emoticon.clone(),
        }
    }

    /// Get the emoji of the dice.
    pub fn emoji(&self) -> &str {
        &self.raw.emoticon
    }

    /// Get the value of the dice.
    pub fn value(&self) -> i32 {
        self.raw.value
    }
}

impl Venue {
    pub fn from_raw_media(venue: tl::types::MessageMediaVenue) -> Self {
        use tl::types::MessageMediaGeo;
        Self {
            geo: Geo::from_raw_media(MessageMediaGeo {
                geo: venue.geo.clone(),
            }),
            raw_venue: venue,
        }
    }

    pub fn to_raw_input_media(&self) -> tl::types::InputMediaVenue {
        tl::types::InputMediaVenue {
            geo_point: match self.geo {
                Some(ref geo) => geo.to_raw_input_geo_point(),
                None => tl::enums::InputGeoPoint::Empty,
            },
            title: self.raw_venue.title.clone(),
            address: self.raw_venue.address.clone(),
            provider: self.raw_venue.provider.clone(),
            venue_id: self.raw_venue.venue_id.clone(),
            venue_type: self.raw_venue.venue_type.clone(),
        }
    }

    /// Get the title of the venue.
    pub fn title(&self) -> &str {
        &self.raw_venue.title
    }

    /// Get the address of the venue.
    pub fn address(&self) -> &str {
        &self.raw_venue.address
    }

    /// Get the provider of the venue location.
    pub fn provider(&self) -> &str {
        &self.raw_venue.provider
    }

    /// Get the id of the venue.
    pub fn venue_id(&self) -> &str {
        &self.raw_venue.venue_id
    }

    /// Get the type of the venue.
    pub fn venue_type(&self) -> &str {
        &self.raw_venue.venue_type
    }
}

impl GeoLive {
    pub fn from_raw_media(geolive: tl::types::MessageMediaGeoLive) -> Self {
        use tl::types::MessageMediaGeo;
        Self {
            geo: Geo::from_raw_media(MessageMediaGeo {
                geo: geolive.geo.clone(),
            }),
            raw_geolive: geolive,
        }
    }

    pub fn to_raw_input_media(&self) -> tl::types::InputMediaGeoLive {
        tl::types::InputMediaGeoLive {
            geo_point: match self.geo {
                Some(ref geo) => geo.to_raw_input_geo_point(),
                None => tl::enums::InputGeoPoint::Empty,
            },
            heading: self.raw_geolive.heading,
            period: Some(self.raw_geolive.period),
            proximity_notification_radius: self.raw_geolive.proximity_notification_radius,
            stopped: false,
        }
    }

    /// Get the heading of the live location in degress (1-360).
    pub fn heading(&self) -> Option<i32> {
        self.raw_geolive.heading
    }

    /// Get the validity period of the live location.
    pub fn period(&self) -> i32 {
        self.raw_geolive.period
    }

    /// Get the radius of the proximity alert.
    pub fn proximity_notification_radius(&self) -> Option<i32> {
        self.raw_geolive.proximity_notification_radius
    }
}

impl WebPage {
    pub fn from_raw_media(webpage: tl::types::MessageMediaWebPage) -> Self {
        Self { raw: webpage }
    }
}

impl Uploaded {
    pub fn from_raw(input_file: tl::enums::InputFile) -> Self {
        Self { raw: input_file }
    }

    pub(crate) fn name(&self) -> &str {
        match &self.raw {
            tl::enums::InputFile::File(f) => f.name.as_ref(),
            tl::enums::InputFile::Big(f) => f.name.as_ref(),
        }
    }
}

impl Media {
    pub fn from_raw(media: tl::enums::MessageMedia) -> Option<Self> {
        use tl::enums::MessageMedia as M;

        // TODO implement the rest
        match media {
            M::Empty => None,
            M::Photo(photo) => Some(Self::Photo(Photo::from_raw_media(photo))),
            M::Geo(geo) => Geo::from_raw_media(geo).map(Self::Geo),
            M::Contact(contact) => Some(Self::Contact(Contact::from_raw_media(contact))),
            M::Unsupported => None,
            M::Document(document) => {
                let document = Document::from_raw_media(document);
                Some(if let Some(sticker) = Sticker::from_document(&document) {
                    Self::Sticker(sticker)
                } else {
                    Self::Document(document)
                })
            }
            M::WebPage(webpage) => Some(Self::WebPage(WebPage::from_raw_media(webpage))),
            M::Venue(venue) => Some(Self::Venue(Venue::from_raw_media(venue))),
            M::Game(_) => None,
            M::Invoice(_) => None,
            M::GeoLive(geolive) => Some(Self::GeoLive(GeoLive::from_raw_media(geolive))),
            M::Poll(poll) => Some(Self::Poll(Poll::from_raw_media(poll))),
            M::Dice(dice) => Some(Self::Dice(Dice::from_raw_media(dice))),
            M::Story(_) => None,
            M::Giveaway(_) => None,
            M::GiveawayResults(_) => None,
            M::PaidMedia(_) => None,
        }
    }

    pub fn to_raw_input_media(&self) -> Option<tl::enums::InputMedia> {
        match self {
            Media::Photo(photo) => Some(photo.to_raw_input_media().into()),
            Media::Document(document) => Some(document.to_raw_input_media().into()),
            Media::Sticker(sticker) => Some(sticker.document.to_raw_input_media().into()),
            Media::Contact(contact) => Some(contact.to_raw_input_media().into()),
            Media::Poll(poll) => Some(poll.to_raw_input_media().into()),
            Media::Geo(geo) => Some(geo.to_raw_input_media().into()),
            Media::Dice(dice) => Some(dice.to_raw_input_media().into()),
            Media::Venue(venue) => Some(venue.to_raw_input_media().into()),
            Media::GeoLive(geolive) => Some(geolive.to_raw_input_media().into()),
            Media::WebPage(_) => None,
        }
    }

    pub fn to_raw_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        match self {
            Media::Photo(photo) => photo.to_raw_input_location(),
            Media::Document(document) => document.to_raw_input_location(),
            Media::Sticker(sticker) => sticker.document.to_raw_input_location(),
            Media::Contact(_) => None,
            Media::Poll(_) => None,
            Media::Geo(_) => None,
            Media::Dice(_) => None,
            Media::Venue(_) => None,
            Media::GeoLive(_) => None,
            Media::WebPage(_) => None,
        }
    }
}

impl From<Photo> for Media {
    fn from(photo: Photo) -> Self {
        Self::Photo(photo)
    }
}
