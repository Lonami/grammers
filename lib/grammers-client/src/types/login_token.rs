// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[derive(Debug, Clone)]
pub struct LoginToken {
    pub(crate) phone: String,
    pub(crate) phone_code_hash: String,
}

impl LoginToken {
    ///Create empty LoginToken for init value
    pub fn empty() -> Self {
        LoginToken {
            phone: String::new(),
            phone_code_hash: String::new(),
        }
    }
    ///If Phone and code hash is empty return true
    pub fn is_empty(&self) -> bool {
        self.phone.is_empty() && self.phone_code_hash.is_empty()
    }
    ///Return phone clone
    pub fn phone(&self)->String{
        self.phone.clone()
    }
}
