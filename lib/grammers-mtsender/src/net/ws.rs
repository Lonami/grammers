// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use futures_util::TryFutureExt;
use log::info;

use super::ServerAddr;

type WsIo = async_io_stream::IoStream<ws_stream_wasm::WsStreamIo, Vec<u8>>;
pub type ReadHalf<'a> = tokio::io::ReadHalf<&'a mut WsIo>;
pub type WriteHalf<'a> = tokio::io::WriteHalf<&'a mut WsIo>;

pub struct NetStream(WsIo);

impl NetStream {
    pub(crate) fn split(&mut self) -> (ReadHalf, WriteHalf) {
        tokio::io::split(&mut self.0)
    }

    pub(crate) async fn connect(addr: &ServerAddr) -> Result<Self, std::io::Error> {
        info!("connecting...");
        match addr {
            ServerAddr::Ws { address } => {
                let (_, wsio) = ws_stream_wasm::WsMeta::connect(address, Some(["binary"].to_vec()))
                    .map_err(|err| {
                        std::io::Error::new(
                            std::io::ErrorKind::ConnectionRefused,
                            format!("failed to connect to websocket: {}", err),
                        )
                    })
                    .await?;

                Ok(Self(wsio.into_io()))
            }
        }
    }
}
