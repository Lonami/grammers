// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::ops::{Deref, DerefMut};

use grammers_session::updates::State;
use grammers_tl_types as tl;

/// Update that all receive whenever a message is received or edited.
///
/// For bots to receive this update, they must either be an administrator
/// or have disabled the privacy mode via [@BotFather](https://t.me/BotFather).
#[derive(Debug, Clone)]
pub struct Message {
    pub(crate) msg: crate::message::Message,
    pub raw: tl::enums::Update,
    pub state: State,
}

impl Deref for Message {
    type Target = crate::message::Message;

    fn deref(&self) -> &Self::Target {
        &self.msg
    }
}

impl DerefMut for Message {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.msg
    }
}
