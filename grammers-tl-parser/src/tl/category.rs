// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

/// The category to which a definition belongs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Category {
    /// The default category, a definition represents a type.
    Types,

    /// A definition represents a callable function.
    Functions,
}
