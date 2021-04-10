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
use grammers_tl_types::{enums, functions, Deserializable, RemoteCall};
use log;
use simple_logger::SimpleLogger;
use tokio::runtime;

#[test]
fn test_invoke_encrypted_method() {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    let rt = runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        let (mut sender, enqueuer) = connect(transport::Full::new(), TELEGRAM_TEST_DC_2)
            .await
            .unwrap();

        let mut rx = enqueuer.enqueue(&functions::help::GetNearestDc {});
        loop {
            sender.step().await.unwrap();
            if let Ok(response) = rx.try_recv() {
                match response {
                    Ok(body) => {
                        let response =
                            <functions::help::GetNearestDc as RemoteCall>::Return::from_bytes(
                                &body,
                            );
                        assert!(matches!(response, Ok(enums::NearestDc::Dc(_))));
                        break;
                    }
                    x => panic!("did not get nearest dc, got: {:?}", x),
                }
            }
        }
    });
}
