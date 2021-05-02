// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;
use std::{convert::TryInto, time::Duration};

pub enum Attribute {
    Audio {
        duration: Duration,
        title: Option<String>,
        performer: Option<String>,
    },
    Voice {
        duration: Duration,
        waveform: Option<Vec<u8>>,
    },
    Video {
        round_message: bool,
        supports_streaming: bool,
        duration: Duration,
        w: i32,
        h: i32,
    },
    FileName(String),
}

impl From<Attribute> for tl::enums::DocumentAttribute {
    fn from(attr: Attribute) -> Self {
        use Attribute::*;
        match attr {
            Audio {
                duration,
                title,
                performer,
            } => Self::Audio(tl::types::DocumentAttributeAudio {
                voice: false,
                duration: duration.as_secs().try_into().unwrap(),
                title,
                performer,
                waveform: None,
            }),
            Voice { duration, waveform } => Self::Audio(tl::types::DocumentAttributeAudio {
                voice: false,
                duration: duration.as_secs().try_into().unwrap(),
                title: None,
                performer: None,
                waveform,
            }),
            Video {
                round_message,
                supports_streaming,
                duration,
                w,
                h,
            } => Self::Video(tl::types::DocumentAttributeVideo {
                round_message,
                supports_streaming,
                duration: duration.as_secs().try_into().unwrap(),
                w,
                h,
            }),
            FileName(file_name) => {
                Self::Filename(tl::types::DocumentAttributeFilename { file_name })
            }
        }
    }
}
