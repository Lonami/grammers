// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

pub trait InputFileExt {
    /// Get the file's name.
    fn name(&self) -> &str;
}

impl InputFileExt for tl::enums::InputFile {
    fn name(&self) -> &str {
        match self {
            tl::enums::InputFile::File(f) => &f.name,
            tl::enums::InputFile::Big(f) => &f.name,
        }
    }
}
