// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::ops::{Deref, DerefMut};

use grammers_tl_types as tl;

use crate::types::Message as Msg;
use grammers_session::State;

#[derive(Debug, Clone)]
pub struct Message {
    pub(crate) msg: Msg,
    pub raw: tl::enums::Update,
    pub state: State,
}

impl Deref for Message {
    type Target = Msg;

    fn deref(&self) -> &Self::Target {
        &self.msg
    }
}

impl DerefMut for Message {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.msg
    }
}
