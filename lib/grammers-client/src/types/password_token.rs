// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

#[derive(Debug, Clone)]
pub struct PasswordToken {
    pub(crate) password: tl::types::account::Password,
}

impl PasswordToken {
    pub fn new(password: tl::types::account::Password) -> Self {
        PasswordToken { password }
    }

    pub fn hint(&self) -> Option<&str> {
        self.password.hint.as_deref()
    }
}
