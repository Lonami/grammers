// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
mod tcp;

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
mod ws;

#[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
pub use tcp::NetStream;
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub use ws::NetStream;

#[cfg(all(target_arch = "wasm32", target_os = "unknown", feature = "proxy"))]
compile_error!("TCP proxies are not supported when compiling for WASM");

#[derive(Debug, Clone)]
pub enum ServerAddr {
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    Ws { address: String },
    #[cfg(all(
        not(all(target_arch = "wasm32", target_os = "unknown")),
        feature = "proxy"
    ))]
    Proxied {
        address: std::net::SocketAddr,
        proxy: String,
    },
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    Tcp { address: std::net::SocketAddr },
}
