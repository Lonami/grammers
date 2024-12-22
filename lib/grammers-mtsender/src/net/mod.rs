// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

mod tcp;

pub use tcp::NetStream;

#[derive(Debug, Clone)]
pub enum ServerAddr {
    #[cfg(feature = "proxy")]
    Proxied {
        address: std::net::SocketAddr,
        proxy: String,
    },
    Tcp {
        address: std::net::SocketAddr,
    },
}
