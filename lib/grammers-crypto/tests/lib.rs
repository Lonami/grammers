use grammers_crypto::{self, Padding, Side};

fn get_test_auth_key() -> [u8; 256] {
    let mut buffer = [0u8; 256];
    buffer
        .iter_mut()
        .enumerate()
        .for_each(|(i, x)| *x = i as u8);

    buffer
}

#[test]
fn auth_key_aux_hash() {
    let auth_key = get_test_auth_key();
    let expected = [73, 22, 214, 189, 183, 247, 142, 104];

    assert_eq!(grammers_crypto::auth_key_aux_hash(&auth_key), expected);
}

#[test]
fn auth_key_id() {
    let auth_key = get_test_auth_key();
    let expected = [50, 209, 88, 110, 164, 87, 223, 200];

    assert_eq!(grammers_crypto::auth_key_id(&auth_key), expected);
}

#[test]
fn encrypt_client_data_v2() {
    let plaintext = b"Hello, world! This data should remain secure!"
        .iter()
        .cloned()
        .collect::<Vec<u8>>();

    let auth_key = get_test_auth_key();
    let side = Side::Client;
    let padding = Padding::Zero;
    let expected = vec![
        50, 209, 88, 110, 164, 87, 223, 200, 168, 23, 41, 212, 109, 181, 64, 25, 162, 191, 215,
        247, 68, 249, 185, 108, 79, 113, 108, 253, 196, 71, 125, 178, 162, 193, 95, 109, 219, 133,
        35, 95, 185, 85, 47, 29, 132, 7, 198, 170, 234, 0, 204, 132, 76, 90, 27, 246, 172, 68, 183,
        155, 94, 220, 42, 35, 134, 139, 61, 96, 115, 165, 144, 153, 44, 15, 41, 117, 36, 61, 86,
        62, 161, 128, 210, 24, 238, 117, 124, 154,
    ];

    assert_eq!(
        grammers_crypto::encrypt_data_v2(&plaintext, &auth_key, side, padding),
        expected
    );
}
