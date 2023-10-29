// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use crate::types::photo_sizes::{PhotoSize, VecExt};
use crate::Client;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};
use grammers_tl_types as tl;
use std::fmt::Debug;

#[derive(Clone, Debug, PartialEq)]
pub struct Photo {
    photo: tl::types::MessageMediaPhoto,
    client: Client,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Document {
    document: tl::types::MessageMediaDocument,
    client: Client,
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
pub struct Contact {
    contact: tl::types::MessageMediaContact,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Poll {
    poll: tl::types::Poll,
    results: tl::types::PollResults,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Geo {
    geo: tl::types::GeoPoint,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Dice {
    dice: tl::types::MessageMediaDice,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Venue {
    pub geo: Option<Geo>,
    venue: tl::types::MessageMediaVenue,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GeoLive {
    pub geo: Option<Geo>,
    geolive: tl::types::MessageMediaGeoLive,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WebPage {
    webpage: tl::types::MessageMediaWebPage,
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
    fn _from_raw(photo: tl::enums::Photo, client: Client) -> Self {
        Self {
            photo: tl::types::MessageMediaPhoto {
                spoiler: false,
                photo: Some(photo),
                ttl_seconds: None,
            },
            client,
        }
    }

    #[cfg(not(feature = "unstable_raw"))]
    pub(crate) fn from_raw(photo: tl::enums::Photo, client: Client) -> Self {
        Self::_from_raw(photo, client)
    }

    #[cfg(feature = "unstable_raw")]
    pub fn from_raw(photo: tl::enums::Photo, client: Client) -> Self {
        Self::_from_raw(photo, client)
    }

    fn _from_media(photo: tl::types::MessageMediaPhoto, client: Client) -> Self {
        Self { photo, client }
    }

    #[cfg(not(feature = "unstable_raw"))]
    pub(crate) fn from_media(photo: tl::types::MessageMediaPhoto, client: Client) -> Self {
        Self::_from_media(photo, client)
    }

    #[cfg(feature = "unstable_raw")]
    pub fn from_media(photo: tl::types::MessageMediaPhoto, client: Client) -> Self {
        Self::_from_media(photo, client)
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

    fn to_input_media(&self) -> tl::types::InputMediaPhoto {
        use tl::{
            enums::{InputPhoto as eInputPhoto, Photo},
            types::InputPhoto,
        };

        tl::types::InputMediaPhoto {
            spoiler: false,
            id: match self.photo.photo {
                Some(Photo::Photo(ref photo)) => InputPhoto {
                    id: photo.id,
                    access_hash: photo.access_hash,
                    file_reference: photo.file_reference.clone(),
                }
                .into(),
                _ => eInputPhoto::Empty,
            },
            ttl_seconds: self.photo.ttl_seconds,
        }
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
    /// <https://core.telegram.org/api/files#image-thumbnail-types>
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
                .map(|x| PhotoSize::make_from(x, photo, self.client.clone()))
                .collect(),
        }
    }

    /// Returns true if the photo is a spoiler.
    pub fn is_spoiler(&self) -> bool {
        self.photo.spoiler
    }
}

impl Document {
    fn _from_media(document: tl::types::MessageMediaDocument, client: Client) -> Self {
        Self { document, client }
    }

    #[cfg(not(feature = "unstable_raw"))]
    pub(crate) fn from_media(document: tl::types::MessageMediaDocument, client: Client) -> Self {
        Self::_from_media(document, client)
    }

    #[cfg(feature = "unstable_raw")]
    pub fn from_media(document: tl::types::MessageMediaDocument, client: Client) -> Self {
        Self::_from_media(document, client)
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

    fn to_input_media(&self) -> tl::types::InputMediaDocument {
        use tl::{
            enums::{Document, InputDocument as eInputDocument},
            types::InputDocument,
        };

        tl::types::InputMediaDocument {
            spoiler: false,
            id: match self.document.document {
                Some(Document::Document(ref document)) => InputDocument {
                    id: document.id,
                    access_hash: document.access_hash,
                    file_reference: document.file_reference.clone(),
                }
                .into(),
                _ => eInputDocument::Empty,
            },
            ttl_seconds: self.document.ttl_seconds,
            query: None,
        }
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
            Some(tl::enums::Document::Document(d)) => Some(Utc.from_utc_datetime(
                &NaiveDateTime::from_timestamp_opt(d.date as i64, 0).expect("date out of range"),
            )),
            _ => None,
        }
    }

    /// The size of the file.
    /// returns 0 if the document is empty.
    pub fn size(&self) -> i64 {
        match self.document.document.as_ref() {
            Some(tl::enums::Document::Document(d)) => d.size,
            _ => 0,
        }
    }

    /// Duration of video/audio, in seconds
    pub fn duration(&self) -> Option<i32> {
        match self.document.document.as_ref() {
            Some(tl::enums::Document::Document(d)) => {
                for attr in &d.attributes {
                    match attr {
                        tl::enums::DocumentAttribute::Video(v) => {
                            return Some(v.duration.max(i32::MAX as _) as i32)
                        }
                        tl::enums::DocumentAttribute::Audio(a) => return Some(a.duration),
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
        match self.document.document.as_ref() {
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
        match self.document.document.as_ref() {
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
        match self.document.document.as_ref() {
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
        match self.document.document.as_ref() {
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
        self.document.spoiler
    }
}

impl Sticker {
    fn _from_document(document: &Document) -> Option<Self> {
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

    #[cfg(not(feature = "unstable_raw"))]
    pub(crate) fn from_document(document: &Document) -> Option<Self> {
        Self::_from_document(document)
    }

    #[cfg(feature = "unstable_raw")]
    pub fn from_document(document: &Document) -> Option<Self> {
        Self::_from_document(document)
    }

    /// Get the emoji associated with the sticker.
    pub fn emoji(&self) -> &str {
        self.attrs.alt.as_str()
    }

    /// Is this sticker an animated sticker?
    pub fn is_animated(&self) -> bool {
        self.animated
    }
}

impl Contact {
    fn _from_media(contact: tl::types::MessageMediaContact) -> Self {
        Self { contact }
    }

    #[cfg(not(feature = "unstable_raw"))]
    pub(crate) fn from_media(contact: tl::types::MessageMediaContact) -> Self {
        Self::_from_media(contact)
    }

    #[cfg(feature = "unstable_raw")]
    pub fn from_media(contact: tl::types::MessageMediaContact) -> Self {
        Self::_from_media(contact)
    }

    pub(crate) fn to_input_media(&self) -> tl::types::InputMediaContact {
        tl::types::InputMediaContact {
            phone_number: self.contact.phone_number.clone(),
            first_name: self.contact.first_name.clone(),
            last_name: self.contact.last_name.clone(),
            vcard: self.contact.vcard.clone(),
        }
    }

    /// The contact's phone number, in international format. This field will always be a non-empty
    /// string of digits, although there's no guarantee that the number actually exists.
    pub fn phone_number(&self) -> &str {
        self.contact.phone_number.as_str()
    }

    /// The contact's first name. Although official clients will always send a non-empty string,
    /// it is possible for this field to be empty when sent via different means.
    pub fn first_name(&self) -> &str {
        self.contact.first_name.as_str()
    }

    /// The contact's last name. May be empty if it's not set by sender.
    pub fn last_name(&self) -> &str {
        self.contact.last_name.as_str()
    }

    /// Contact information in [vCard format][1]. Applications such as Telegram Desktop leave this
    /// field empty. The vCard version used in this field could be any. The field may also contain
    /// arbitrary text when sent by non-official clients.
    ///
    /// [1]: https://en.wikipedia.org/wiki/VCard
    pub fn vcard(&self) -> &str {
        self.contact.vcard.as_str()
    }
}

impl Poll {
    fn _from_media(poll: tl::types::MessageMediaPoll) -> Self {
        Self {
            poll: match poll.poll {
                tl::enums::Poll::Poll(poll) => poll,
            },
            results: match poll.results {
                tl::enums::PollResults::Results(results) => results,
            },
        }
    }

    #[cfg(not(feature = "unstable_raw"))]
    pub(crate) fn from_media(poll: tl::types::MessageMediaPoll) -> Self {
        Self::_from_media(poll)
    }

    #[cfg(feature = "unstable_raw")]
    pub fn from_media(poll: tl::types::MessageMediaPoll) -> Self {
        Self::_from_media(poll)
    }

    fn to_input_media(&self) -> tl::types::InputMediaPoll {
        tl::types::InputMediaPoll {
            poll: grammers_tl_types::enums::Poll::Poll(self.poll.clone()),
            correct_answers: None,
            solution: None,
            solution_entities: None,
        }
    }

    /// Return question of the poll
    pub fn question(&self) -> &str {
        &self.poll.question
    }

    /// Return if current poll is quiz
    pub fn is_quiz(&self) -> bool {
        self.poll.quiz
    }

    /// Indicator that poll is closed
    pub fn closed(&self) -> bool {
        self.poll.closed
    }

    /// Iterator over poll answer options
    pub fn iter_answers(&self) -> impl Iterator<Item = &tl::types::PollAnswer> {
        self.poll.answers.iter().map(|answer| match answer {
            tl::enums::PollAnswer::Answer(answer) => answer,
        })
    }

    /// Total voters that took part in the vote
    ///
    /// May be None if poll isn't started
    pub fn total_voters(&self) -> Option<i32> {
        self.results.total_voters
    }

    /// Return details of the voters choices:
    /// how much voters chose each answer and wether current option
    pub fn iter_voters_summary(
        &self,
    ) -> Option<impl Iterator<Item = &tl::types::PollAnswerVoters>> {
        self.results.results.as_ref().map(|results| {
            results.iter().map(|result| match result {
                tl::enums::PollAnswerVoters::Voters(voters) => voters,
            })
        })
    }
}

impl Geo {
    fn _from_media(geo: tl::types::MessageMediaGeo) -> Option<Self> {
        use tl::enums::GeoPoint as eGeoPoint;

        match &geo.geo {
            eGeoPoint::Empty => None,
            eGeoPoint::Point(point) => Some(Self { geo: point.clone() }),
        }
    }

    #[cfg(not(feature = "unstable_raw"))]
    pub(crate) fn from_media(geo: tl::types::MessageMediaGeo) -> Option<Self> {
        Self::_from_media(geo)
    }

    #[cfg(feature = "unstable_raw")]
    pub fn from_media(geo: tl::types::MessageMediaGeo) -> Option<Self> {
        Self::_from_media(geo)
    }

    pub(crate) fn to_input_media(&self) -> tl::types::InputMediaGeoPoint {
        use tl::types::InputGeoPoint;

        tl::types::InputMediaGeoPoint {
            geo_point: InputGeoPoint {
                lat: self.geo.lat,
                long: self.geo.long,
                accuracy_radius: self.geo.accuracy_radius,
            }
            .into(),
        }
    }

    pub(crate) fn to_input_geo_point(&self) -> tl::enums::InputGeoPoint {
        use tl::{enums::InputGeoPoint as eInputGeoPoint, types::InputGeoPoint};

        eInputGeoPoint::Point(InputGeoPoint {
            lat: self.geo.lat,
            long: self.geo.long,
            accuracy_radius: self.geo.accuracy_radius,
        })
    }

    /// Get the latitude of the location.
    pub fn latitue(&self) -> f64 {
        self.geo.lat
    }

    /// Get the latitude of the location.
    pub fn longitude(&self) -> f64 {
        self.geo.long
    }

    /// Get the accuracy of the geo location in meters.
    pub fn accuracy_radius(&self) -> Option<i32> {
        self.geo.accuracy_radius
    }
}

impl Dice {
    fn _from_media(dice: tl::types::MessageMediaDice) -> Self {
        Self { dice }
    }

    #[cfg(not(feature = "unstable_raw"))]
    pub(crate) fn from_media(dice: tl::types::MessageMediaDice) -> Self {
        Self::_from_media(dice)
    }

    #[cfg(feature = "unstable_raw")]
    pub fn from_media(dice: tl::types::MessageMediaDice) -> Self {
        Self::_from_media(dice)
    }

    fn to_input_media(&self) -> tl::types::InputMediaDice {
        tl::types::InputMediaDice {
            emoticon: self.dice.emoticon.clone(),
        }
    }

    /// Get the emoji of the dice.
    pub fn emoji(&self) -> &str {
        &self.dice.emoticon
    }

    /// Get the value of the dice.
    pub fn value(&self) -> i32 {
        self.dice.value
    }
}

impl Venue {
    fn _from_media(venue: tl::types::MessageMediaVenue) -> Self {
        use tl::types::MessageMediaGeo;
        Self {
            geo: Geo::from_media(MessageMediaGeo {
                geo: venue.geo.clone(),
            }),
            venue,
        }
    }

    #[cfg(not(feature = "unstable_raw"))]
    pub(crate) fn from_media(venue: tl::types::MessageMediaVenue) -> Self {
        Self::_from_media(venue)
    }

    #[cfg(feature = "unstable_raw")]
    pub fn from_media(venue: tl::types::MessageMediaVenue) -> Self {
        Self::_from_media(venue)
    }

    fn to_input_media(&self) -> tl::types::InputMediaVenue {
        tl::types::InputMediaVenue {
            geo_point: match self.geo {
                Some(ref geo) => geo.to_input_geo_point(),
                None => tl::enums::InputGeoPoint::Empty,
            },
            title: self.venue.title.clone(),
            address: self.venue.address.clone(),
            provider: self.venue.provider.clone(),
            venue_id: self.venue.venue_id.clone(),
            venue_type: self.venue.venue_type.clone(),
        }
    }

    /// Get the title of the venue.
    pub fn title(&self) -> &str {
        &self.venue.title
    }

    /// Get the address of the venue.
    pub fn address(&self) -> &str {
        &self.venue.address
    }

    /// Get the provider of the venue location.
    pub fn provider(&self) -> &str {
        &self.venue.provider
    }

    /// Get the id of the venue.
    pub fn venue_id(&self) -> &str {
        &self.venue.venue_id
    }

    /// Get the type of the venue.
    pub fn venue_type(&self) -> &str {
        &self.venue.venue_type
    }
}

impl GeoLive {
    fn _from_media(geolive: tl::types::MessageMediaGeoLive) -> Self {
        use tl::types::MessageMediaGeo;
        Self {
            geo: Geo::from_media(MessageMediaGeo {
                geo: geolive.geo.clone(),
            }),
            geolive,
        }
    }

    #[cfg(not(feature = "unstable_raw"))]
    pub(crate) fn from_media(geolive: tl::types::MessageMediaGeoLive) -> Self {
        Self::_from_media(geolive)
    }

    #[cfg(feature = "unstable_raw")]
    pub fn from_media(geolive: tl::types::MessageMediaGeoLive) -> Self {
        Self::_from_media(geolive)
    }

    fn to_input_media(&self) -> tl::types::InputMediaGeoLive {
        tl::types::InputMediaGeoLive {
            geo_point: match self.geo {
                Some(ref geo) => geo.to_input_geo_point(),
                None => tl::enums::InputGeoPoint::Empty,
            },
            heading: self.geolive.heading,
            period: Some(self.geolive.period),
            proximity_notification_radius: self.geolive.proximity_notification_radius,
            stopped: false,
        }
    }

    /// Get the heading of the live location in degress (1-360).
    pub fn heading(&self) -> Option<i32> {
        self.geolive.heading
    }

    /// Get the validity period of the live location.
    pub fn period(&self) -> i32 {
        self.geolive.period
    }

    /// Get the radius of the proximity alert.
    pub fn proximity_notification_radius(&self) -> Option<i32> {
        self.geolive.proximity_notification_radius
    }
}

impl WebPage {
    fn _from_media(webpage: tl::types::MessageMediaWebPage) -> Self {
        Self { webpage }
    }

    #[cfg(not(feature = "unstable_raw"))]
    pub(crate) fn from_media(webpage: tl::types::MessageMediaWebPage) -> Self {
        Self::_from_media(webpage)
    }

    #[cfg(feature = "unstable_raw")]
    pub fn from_media(webpage: tl::types::MessageMediaWebPage) -> Self {
        Self::_from_media(webpage)
    }
}

impl Uploaded {
    fn _from_raw(input_file: tl::enums::InputFile) -> Self {
        Self { input_file }
    }

    #[cfg(not(feature = "unstable_raw"))]
    pub(crate) fn from_raw(input_file: tl::enums::InputFile) -> Self {
        Self::_from_raw(input_file)
    }

    #[cfg(feature = "unstable_raw")]
    pub fn from_raw(input_file: tl::enums::InputFile) -> Self {
        Self::_from_raw(input_file)
    }

    pub(crate) fn name(&self) -> &str {
        match &self.input_file {
            tl::enums::InputFile::File(f) => f.name.as_ref(),
            tl::enums::InputFile::Big(f) => f.name.as_ref(),
        }
    }
}

impl Media {
    fn _from_raw(media: tl::enums::MessageMedia, client: Client) -> Option<Self> {
        use tl::enums::MessageMedia as M;

        // TODO implement the rest
        match media {
            M::Empty => None,
            M::Photo(photo) => Some(Self::Photo(Photo::from_media(photo, client))),
            M::Geo(geo) => Geo::from_media(geo).map(Self::Geo),
            M::Contact(contact) => Some(Self::Contact(Contact::from_media(contact))),
            M::Unsupported => None,
            M::Document(document) => {
                let document = Document::from_media(document, client);
                Some(if let Some(sticker) = Sticker::from_document(&document) {
                    Self::Sticker(sticker)
                } else {
                    Self::Document(document)
                })
            }
            M::WebPage(webpage) => Some(Self::WebPage(WebPage::from_media(webpage))),
            M::Venue(venue) => Some(Self::Venue(Venue::from_media(venue))),
            M::Game(_) => None,
            M::Invoice(_) => None,
            M::GeoLive(geolive) => Some(Self::GeoLive(GeoLive::from_media(geolive))),
            M::Poll(poll) => Some(Self::Poll(Poll::from_media(poll))),
            M::Dice(dice) => Some(Self::Dice(Dice::from_media(dice))),
            M::Story(_) => None,
            M::Giveaway(_) => None,
        }
    }

    #[cfg(not(feature = "unstable_raw"))]
    pub(crate) fn from_raw(media: tl::enums::MessageMedia, client: Client) -> Option<Self> {
        Self::_from_raw(media, client)
    }

    #[cfg(feature = "unstable_raw")]
    pub fn from_raw(media: tl::enums::MessageMedia, client: Client) -> Option<Self> {
        Self::_from_raw(media, client)
    }

    pub(crate) fn to_input_media(&self) -> Option<tl::enums::InputMedia> {
        match self {
            Media::Photo(photo) => Some(photo.to_input_media().into()),
            Media::Document(document) => Some(document.to_input_media().into()),
            Media::Sticker(sticker) => Some(sticker.document.to_input_media().into()),
            Media::Contact(contact) => Some(contact.to_input_media().into()),
            Media::Poll(poll) => Some(poll.to_input_media().into()),
            Media::Geo(geo) => Some(geo.to_input_media().into()),
            Media::Dice(dice) => Some(dice.to_input_media().into()),
            Media::Venue(venue) => Some(venue.to_input_media().into()),
            Media::GeoLive(geolive) => Some(geolive.to_input_media().into()),
            Media::WebPage(_) => None,
        }
    }

    pub(crate) fn to_input_location(&self) -> Option<tl::enums::InputFileLocation> {
        match self {
            Media::Photo(photo) => photo.to_input_location(),
            Media::Document(document) => document.to_input_location(),
            Media::Sticker(sticker) => sticker.document.to_input_location(),
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

#[cfg(feature = "unstable_raw")]
impl From<Media> for tl::enums::MessageMedia {
    fn from(media: Media) -> Self {
        use tl::enums::GeoPoint as eGeoPoint;
        use tl::types::{MessageMediaGeo, MessageMediaPoll};

        match media {
            Media::Photo(photo) => photo.photo.into(),
            Media::Document(document) => document.document.into(),
            Media::Sticker(sticker) => sticker.document.document.into(),
            Media::Contact(contact) => contact.contact.into(),
            Media::Poll(Poll { poll, results }) => MessageMediaPoll {
                poll: poll.into(),
                results: results.into(),
            }
            .into(),
            Media::Geo(geo) => MessageMediaGeo {
                geo: eGeoPoint::Point(geo.geo),
            }
            .into(),
            Media::Dice(dice) => dice.dice.into(),
            Media::Venue(venue) => venue.venue.into(),
            Media::GeoLive(geolive) => geolive.geolive.into(),
            Media::WebPage(webpage) => webpage.webpage.into(),
        }
    }
}

#[cfg(feature = "unstable_raw")]
impl From<Uploaded> for tl::enums::InputFile {
    fn from(uploaded: Uploaded) -> Self {
        uploaded.input_file
    }
}
