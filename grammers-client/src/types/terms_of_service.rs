// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_tl_types as tl;

/// Telegram's current Terms of Service.
///
/// When signing up a new account, you agree to these comply with these terms for as long as you
/// use the service.
#[derive(Debug)]
pub struct TermsOfService {
    pub raw: tl::types::help::TermsOfService,
}

impl TermsOfService {
    pub(crate) fn from_raw(
        tl::enums::help::TermsOfService::Service(tos): tl::enums::help::TermsOfService,
    ) -> Self {
        Self { raw: tos }
    }

    /// Whether the terms should be shown as a popup dialog to the user.
    pub fn show_popup(&self) -> bool {
        self.raw.popup
    }

    /// The terms and conditions that must be agreed upon in order to use the service.
    pub fn text(&self) -> &str {
        self.raw.text.as_ref()
    }

    // TODO allow access to markdown/html text too, if the features are enabled

    /// The minimum age restriction to use the service, if applicable.
    pub fn minimum_age(&self) -> Option<i32> {
        self.raw.min_age_confirm
    }
}
