pub const TELEGRAM_TEST_DC_2: &'static str = "149.154.167.40:443";

/// The default datacenter to connect to for testing.
pub const TELEGRAM_DEFAULT_TEST_DC: &'static str = TELEGRAM_TEST_DC_2;

use grammers_mtsender::MTSender;

#[test]
fn test_auth_key_generation() {
    let mut sender = MTSender::connect(TELEGRAM_DEFAULT_TEST_DC).unwrap();
    assert!(sender.generate_auth_key().is_ok());
}
