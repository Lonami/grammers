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
#[non_exhaustive]
pub enum Media {
    Photo(Photo),
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
}

impl From<Photo> for Media {
    fn from(photo: Photo) -> Self {
        Self::Photo(photo)
    }
}
