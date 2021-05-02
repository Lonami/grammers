// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

pub enum Attribute {
    Audio {
        duration: i32,
        title: Option<String>,
        performer: Option<String>,
    },
    Voice {
        duration: i32,
        waveform: Option<Vec<u8>>,
    },
    Video {
        round_message: bool,
        supports_streaming: bool,
        duration: i32,
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
                duration,
                title,
                performer,
                waveform: None,
            }),
            Voice { duration, waveform } => Self::Audio(tl::types::DocumentAttributeAudio {
                voice: false,
                duration,
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
                duration,
                w,
                h,
            }),
            FileName(file_name) => {
                Self::Filename(tl::types::DocumentAttributeFilename { file_name })
            }
        }
    }
}
