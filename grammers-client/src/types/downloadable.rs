// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

pub trait Downloadable {
    fn to_raw_input_location(&self) -> Option<tl::enums::InputFileLocation>;

    // Data for tiny thumbnails comes inline, so there is no need to download anything.
    fn to_data(&self) -> Option<Vec<u8>> {
        None
    }

    // Size, if known, to parallelize large downloads.
    fn size(&self) -> Option<usize> {
        None
    }
}
