// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
pub const TELEGRAM_TEST_DC_2: &str = "149.154.167.40:443";

/// The default datacenter to connect to for testing.
pub const TELEGRAM_DEFAULT_TEST_DC: &str = TELEGRAM_TEST_DC_2;

use grammers_mtsender::MTSender;

#[test]
fn test_auth_key_generation() {
    let mut sender = MTSender::connect(TELEGRAM_DEFAULT_TEST_DC).unwrap();
    assert!(sender.generate_auth_key().is_ok());
}
