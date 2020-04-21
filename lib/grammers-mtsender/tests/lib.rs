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

use async_std::net::TcpStream;
use async_std::task;
use grammers_mtproto::transports::TransportFull;
use grammers_mtsender::create_mtp;
use grammers_tl_types::{enums, functions};

#[test]
fn test_invoke_encrypted_method() {
    task::block_on(async {
        let stream = TcpStream::connect(TELEGRAM_DEFAULT_TEST_DC).await.unwrap();
        let in_stream = stream.clone();
        let out_stream = stream;

        // Creating a sender without explicitly providing an input auth_key
        // will cause it to generate a new one, because they are otherwise
        // not usable. We're also making sure that works here.
        let (mut sender, _updates, handler) =
            create_mtp::<TransportFull, _, _>((in_stream, out_stream), None)
                .await
                .unwrap();

        task::spawn(handler.run());
        match sender.invoke(&functions::help::GetNearestDc {}).await {
            Ok(enums::NearestDc::Dc(_)) => {}
            x => panic!(format!("did not get nearest dc, got: {:?}", x)),
        }
    })
}
