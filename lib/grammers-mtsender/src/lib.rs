// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod errors;

pub use errors::{AuthorizationError, InvocationError};
use grammers_crypto::AuthKey;
use grammers_mtproto::errors::{RequestError, TransportError};
use grammers_mtproto::mtp::{self, Deserialization, Mtp};
use grammers_mtproto::transports::Transport;
use grammers_mtproto::{authentication, MsgId};
use grammers_tl_types::{Deserializable, RemoteCall};
use log::{debug, info};
use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};

/// The maximum data that we're willing to send or receive at once.
///
/// By having a fixed-size buffer, we can avoid unnecessary allocations
/// and trivially prevent allocating more than this limit if we ever
/// received invalid data.
///
/// Telegram will close the connection with roughly a megabyte of data,
/// so to account for the transports' own overhead, we add a few extra
/// kilobytes to the maximum data size.
const MAXIMUM_DATA: usize = (1 * 1024 * 1024) + (8 * 1024);

pub struct Sender<T: Transport, M: Mtp> {
    stream: TcpStream,
    transport: T,
    mtp: M,
    mtp_buffer: Vec<u8>,
    transport_buffer: Vec<u8>,
}

// TODO currently only one thing can be invoked at a time, and updates are
//      ignored; the sender needs a way to support enqueueing multiple
//      requests and properly giving access to updates
impl<T: Transport, M: Mtp> Sender<T, M> {
    async fn connect<A: ToSocketAddrs>(transport: T, mtp: M, addr: A) -> Result<Self, io::Error> {
        info!("connecting...");
        let stream = TcpStream::connect(addr).await?;

        Ok(Self {
            stream,
            transport,
            mtp,
            mtp_buffer: Vec::with_capacity(MAXIMUM_DATA),
            transport_buffer: Vec::with_capacity(MAXIMUM_DATA),
        })
    }

    /// Invoke a Remote Procedure Call and receive packets from the network until it is answered.
    pub async fn invoke<R: RemoteCall>(
        &mut self,
        request: &R,
    ) -> Result<R::Return, InvocationError> {
        let data = request.to_bytes();
        loop {
            let sent_id = self.write(&data).await?;
            debug!("waiting for a response for sent request {:?}", sent_id);
            'resend: loop {
                let result = self.read().await?;
                for (msg_id, ret) in result.rpc_results {
                    debug!("got result for request {:?}", msg_id);
                    if msg_id != sent_id {
                        continue;
                    }

                    match ret {
                        Ok(x) => return Ok(R::Return::from_bytes(&x)?),
                        Err(RequestError::RPCError(error)) => {
                            return Err(InvocationError::RPC(error));
                        }
                        Err(RequestError::Dropped) => {
                            return Err(InvocationError::Dropped);
                        }
                        Err(RequestError::Deserialize(error)) => {
                            return Err(InvocationError::Deserialize(error));
                        }
                        Err(RequestError::BadMessage { .. }) => {
                            info!("bad msg mtp error, re-sending request");
                            break 'resend;
                        }
                    }
                }
            }
        }
    }

    /// Send the body of a single request, and receive the next message.
    async fn send(&mut self, data: &[u8]) -> Result<Vec<u8>, InvocationError> {
        self.write(data).await?;
        let mut result = self.read().await?;
        Ok(result.rpc_results.remove(0).1.unwrap())
    }

    /// Write out the body of a single request.
    async fn write(&mut self, data: &[u8]) -> Result<MsgId, io::Error> {
        debug!("serializing {} bytes of data...", data.len());
        let msg_id = self
            .mtp
            .serialize(&vec![data.into()], &mut self.mtp_buffer)
            .remove(0);
        self.transport_buffer.clear();
        self.transport
            .pack(&self.mtp_buffer, &mut self.transport_buffer);

        debug!("sending buffer of {} bytes...", self.transport_buffer.len());
        self.stream.write_all(&self.transport_buffer).await?;
        Ok(msg_id)
    }

    /// Read a single transport-level packet and deserialize it.
    async fn read(&mut self) -> Result<Deserialization, InvocationError> {
        self.mtp_buffer.clear();
        self.transport_buffer.clear();
        loop {
            debug!(
                "trying to unpack buffer of {} bytes...",
                self.transport_buffer.len()
            );
            match self
                .transport
                .unpack(&self.transport_buffer, &mut self.mtp_buffer)
            {
                Ok(_) => {
                    debug!("deserializing valid transport packet...");
                    return Ok(self.mtp.deserialize(&self.mtp_buffer)?);
                }
                Err(TransportError::MissingBytes(n)) => {
                    let start = self.transport_buffer.len();
                    let missing = n - start;
                    debug!("receiving {} more bytes...", missing);
                    (0..missing).for_each(|_| self.transport_buffer.push(0));
                    self.stream
                        .read_exact(&mut self.transport_buffer[start..])
                        .await?;
                }
                Err(_err) => todo!(),
            }
        }
    }
}

pub async fn connect<T: Transport, A: ToSocketAddrs>(
    transport: T,
    addr: A,
) -> Result<Sender<T, mtp::Encrypted>, AuthorizationError> {
    let mut sender = Sender::connect(transport, mtp::Plain::new(), addr).await?;

    info!("generating new authorization key...");
    let (request, data) = authentication::step1()?;
    debug!("gen auth key: sending step 1");
    let response = sender.send(&request).await?;
    debug!("gen auth key: starting step 2");
    let (request, data) = authentication::step2(data, &response)?;
    debug!("gen auth key: sending step 2");
    let response = sender.send(&request).await?;
    debug!("gen auth key: starting step 3");
    let (request, data) = authentication::step3(data, &response)?;
    debug!("gen auth key: sending step 3");
    let response = sender.send(&request).await?;
    debug!("gen auth key: completing generation");
    let (auth_key, time_offset) = authentication::create_key(data, &response)?;
    info!("authorization key generated successfully");

    Ok(Sender {
        stream: sender.stream,
        transport: sender.transport,
        mtp: mtp::Encrypted::build()
            .time_offset(time_offset)
            .finish(auth_key),
        mtp_buffer: sender.mtp_buffer,
        transport_buffer: sender.transport_buffer,
    })
}

pub async fn connect_with_auth<T: Transport, A: ToSocketAddrs>(
    transport: T,
    addr: A,
    auth_key: AuthKey,
) -> Result<Sender<T, mtp::Encrypted>, AuthorizationError> {
    Ok(Sender::connect(transport, mtp::Encrypted::build().finish(auth_key), addr).await?)
}
