// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Types relating to message media and downloadables.
//!
//! Properties containing raw types are public and will either be called "raw" or prefixed with "raw_".\
//! Keep in mind that **these fields are not part of the semantic versioning guarantees**.

mod attributes;
mod downloadable;
mod input_media;
mod media;
mod photo_sizes;

pub use attributes::Attribute;
pub use downloadable::Downloadable;
pub use input_media::InputMedia;
pub use media::{
    ChatPhoto, Contact, Dice, Document, Geo, GeoLive, Media, Photo, Poll, Sticker, Uploaded, Venue,
    WebPage,
};
pub use photo_sizes::PhotoSize;
