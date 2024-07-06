// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This module is currently public even though it directly uses `grammers-tl-types`.
//! This will probably change before the 1.0 release.

#[cfg(any(feature = "markdown", feature = "html"))]
mod common;

#[cfg(feature = "html")]
mod html;
#[cfg(feature = "html")]
pub use html::{generate_html_message, parse_html_message};

#[cfg(feature = "markdown")]
mod markdown;
#[cfg(feature = "markdown")]
pub use markdown::{generate_markdown_message, parse_markdown_message};
