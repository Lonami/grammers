// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6};

use crate::types::DcOption;

pub(crate) const DEFAULT_DC: i32 = 2;

const fn ipv4(a: u8, b: u8, c: u8, d: u8) -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(a, b, c, d), 443)
}

const fn ipv6(a: u16, b: u16, c: u16, d: u16, e: u16, f: u16, g: u16, h: u16) -> SocketAddrV6 {
    SocketAddrV6::new(Ipv6Addr::new(a, b, c, d, e, f, g, h), 443, 0, 0)
}

/// Hardcoded known `static` options from `functions::help::GetConfig`.
pub(crate) const KNOWN_DC_OPTIONS: [DcOption; 5] = [
    DcOption {
        id: 1,
        ipv4: ipv4(149, 154, 175, 53),
        ipv6: ipv6(0x2001, 0xb28, 0xf23d, 0xf001, 0, 0, 0, 0xa),
        auth_key: None,
    },
    DcOption {
        id: 2,
        ipv4: ipv4(149, 154, 167, 41),
        ipv6: ipv6(0x2001, 0x67c, 0x4e8, 0xf002, 0, 0, 0, 0xa),
        auth_key: None,
    },
    DcOption {
        id: 3,
        ipv4: ipv4(149, 154, 175, 100),
        ipv6: ipv6(0x2001, 0xb28, 0xf23d, 0xf003, 0, 0, 0, 0xa),
        auth_key: None,
    },
    DcOption {
        id: 4,
        ipv4: ipv4(149, 154, 167, 92),
        ipv6: ipv6(0x2001, 0x67c, 0x4e8, 0xf004, 0, 0, 0, 0xa),
        auth_key: None,
    },
    DcOption {
        id: 5,
        ipv4: ipv4(91, 108, 56, 104),
        ipv6: ipv6(0x2001, 0xb28, 0xf23f, 0xf005, 0, 0, 0, 0xa),
        auth_key: None,
    },
];
