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
        voice: bool,
        duration: i32,
        title: Option<String>,
        performer: Option<String>,
        waveform: Option<Vec<u8>>,
    },
    Video {
        round_message: bool,
        supports_streaming: bool,
        duration: i32,
        w: i32,
        h: i32,
    },
}

impl From<&Attribute> for tl::enums::DocumentAttribute {
    fn from(attr: &Attribute) -> Self {
        match attr {
            Attribute::Audio {
                voice,
                duration,
                title,
                performer,
                waveform,
            } => Self::Audio(tl::types::DocumentAttributeAudio {
                voice: voice.clone(),
                duration: duration.clone(),
                title: title.clone(),
                performer: performer.clone(),
                waveform: waveform.clone(),
            }),
            Attribute::Video {
                round_message,
                supports_streaming,
                duration,
                w,
                h,
            } => Self::Video(tl::types::DocumentAttributeVideo {
                round_message: round_message.clone(),
                supports_streaming: supports_streaming.clone(),
                duration: duration.clone(),
                w: w.clone(),
                h: h.clone(),
            }),
        }
    }
}
