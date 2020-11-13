// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//! Extension traits to make dealing with Telegram types more pleasant.
mod messages;
mod peers;
mod updates;

pub use messages::MessageExt;
pub use peers::InputPeerExt;
pub use updates::UpdateExt;
