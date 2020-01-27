pub const TELEGRAM_TEST_DC_2: &'static str = "149.154.167.40:443";

/// The default datacenter to connect to for testing.
pub const TELEGRAM_DEFAULT_TEST_DC: &'static str = TELEGRAM_TEST_DC_2;

use grammers_mtproto::MTProto;
use grammers_mtsender::MTSender;
use std::io::Result;

#[test]
fn test_auth_key_generation() -> Result<()> {
    let mut sender = MTSender::connect(TELEGRAM_DEFAULT_TEST_DC, MTProto::new()).unwrap();
    let _auth_key = sender.generate_auth_key().unwrap();
    // Great, the authorization key was generated correctly!
    Ok(())
}
