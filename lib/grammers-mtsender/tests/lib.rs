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

use grammers_mtproto::transport;
use grammers_mtsender::connect;
use grammers_tl_types::{enums, functions};
use log;
use simple_logger;
use tokio::runtime;

#[test]
fn test_invoke_encrypted_method() {
    simple_logger::init_with_level(log::Level::Debug).unwrap();

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let mut sender = connect(transport::Full::new(), TELEGRAM_TEST_DC_2)
            .await
            .unwrap();

        match sender.invoke(&functions::help::GetNearestDc {}).await {
            Ok(enums::NearestDc::Dc(_)) => {}
            x => panic!(format!("did not get nearest dc, got: {:?}", x)),
        }
    });
}
