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

use async_std::task;
use grammers_mtsender::connect_mtp;
use grammers_tl_types::functions;

/*
#[test]
fn test_auth_key_generation() {
    task::block_on(async {
        // Creating a sender without explicitly providing an input auth_key
        // will cause it to generate a new one, because they are otherwise
        // not usable.
        let (mut sender, net_handler) = connect_mtp(TELEGRAM_DEFAULT_TEST_DC).await.unwrap();
    })
}
*/

#[test]
fn test_invoke_encrypted_method() {
    task::block_on(async {
        let (mut sender, net_handler) = connect_mtp(TELEGRAM_DEFAULT_TEST_DC).await.unwrap();
        task::spawn(net_handler.run());
        dbg!(sender.invoke(&functions::help::GetNearestDc {}).await);
        panic!("It works!");
    })
}
