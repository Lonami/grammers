const TEST: &'static [(i32, &str)] = &[
    (1, "149.154.175.10"),
    (2, "149.154.167.40"),
    (3, "149.154.175.117"),
];

const PROD: &'static [(i32, &str)] = &[
    (1, "149.154.175.53"),
    (2, "149.154.167.51"),
    (3, "149.154.175.100"),
    (4, "149.154.167.91"),
    (5, "91.108.56.130"),
    (203, "91.105.192.100"),
];

const TEST_IPV6: &'static [(i32, &str)] = &[
    (1, "2001:b28:f23d:f001::e"),
    (2, "2001:67c:4e8:f002::e"),
    (3, "2001:b28:f23d:f003::e"),
];

const PROD_IPV6: &'static [(i32, &str)] = &[
    (1, "2001:b28:f23d:f001::a"),
    (2, "2001:67c:4e8:f002::a"),
    (3, "2001:b28:f23d:f003::a"),
    (4, "2001:67c:4e8:f004::a"),
    (5, "2001:b28:f23f:f005::a"),
    (203, "2a0a:f280:0203:000a:5000:0000:0000:0100"),
];

const PROD_IPV6_MEDIA: &'static [(i32, &str)] = &[
    (2, "2001:067c:04e8:f002:0000:0000:0000:000b"),
    (4, "2001:067c:04e8:f004:0000:0000:0000:000b"),
];

pub struct DataCenterExtractor;

impl DataCenterExtractor {
    fn get_ip_address(dc_id: i32, test_mode: bool, ipv6: bool, media: bool) -> &'static str {
        let array = if test_mode {
            if ipv6 { TEST_IPV6 } else { TEST }
        } else {
            if ipv6 {
                if media { PROD_IPV6_MEDIA } else { PROD_IPV6 }
            } else {
                if media { PROD } else { PROD }
            }
        };

        array.iter().find(|&&(id, _)| id == dc_id).map(|&(_, ip)| ip)
            .expect("Invalid data center ID")
    }

    pub fn new(dc_id: i32, test_mode: bool, ipv6: bool, media: bool) -> (String, i32) {
        let ip = Self::get_ip_address(dc_id, test_mode, ipv6, media);
        let port = if test_mode { 80 } else { 443 };
        (ip.to_string(), port)
    }
}
