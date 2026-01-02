// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use grammers_tl_types as tl;

/// Trait implemented by all types that may be downloaded.
pub trait Downloadable {
    /// Converts this downloadable into the input type that may be used by raw requests to download it.
    fn to_raw_input_location(&self) -> Option<tl::enums::InputFileLocation>;

    /// Returns `Some` if the media has a tiny thumbnail embedded within.
    ///
    /// No network request would occur when attempting to download this downloadable.
    fn to_data(&self) -> Option<Vec<u8>> {
        None
    }

    /// File size, in bytes.
    fn size(&self) -> Option<usize> {
        None
    }
}
