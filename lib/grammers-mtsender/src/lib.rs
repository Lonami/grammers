// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod errors;

pub use errors::{AuthorizationError, InvocationError, ReadError};
use futures::future::FutureExt as _;
use futures::{future, pin_mut};
use grammers_crypto::AuthKey;
use grammers_mtproto::mtp::{self, Mtp};
use grammers_mtproto::transport::{self, Transport};
use grammers_mtproto::{authentication, MsgId};
use grammers_tl_types::{Deserializable, RemoteCall};
use log::{debug, info};
use std::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;

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

// Manages enqueuing requests, matching them to their response, and IO.
pub struct Sender<T: Transport, M: Mtp> {
    stream: TcpStream,
    transport: T,
    mtp: M,
    mtp_buffer: Vec<u8>,

    requests: Vec<Request>,

    // Transport-level buffers and positions
    read_buffer: Vec<u8>,
    read_index: usize,
    write_buffer: Vec<u8>,
    write_index: usize,
}

struct Request {
    body: Vec<u8>,
    state: RequestState,
    result: oneshot::Sender<Result<Vec<u8>, InvocationError>>,
}

enum RequestState {
    NotSerialized,
    Serialized(MsgId),
    Sent(MsgId),
}

impl<T: Transport, M: Mtp> Sender<T, M> {
    async fn connect<A: ToSocketAddrs>(transport: T, mtp: M, addr: A) -> Result<Self, io::Error> {
        info!("connecting...");
        let stream = TcpStream::connect(addr).await?;

        Ok(Self {
            stream,
            transport,
            mtp,
            mtp_buffer: Vec::with_capacity(MAXIMUM_DATA),

            requests: vec![],

            read_buffer: Vec::with_capacity(MAXIMUM_DATA),
            read_index: 0,
            write_buffer: Vec::with_capacity(MAXIMUM_DATA),
            write_index: 0,
        })
    }

    /// `enqueue` a Remote Procedure Call and `step` until it is answered.
    pub async fn invoke<R: RemoteCall>(
        &mut self,
        request: &R,
    ) -> Result<R::Return, InvocationError> {
        let rx = self.enqueue(request);
        let body = self.step_until_receive(rx).await?;
        Ok(R::Return::from_bytes(&body)?)
    }

    /// Like `invoke` but raw data.
    async fn send(&mut self, body: Vec<u8>) -> Result<Vec<u8>, InvocationError> {
        let rx = self.enqueue_body(body);
        Ok(self.step_until_receive(rx).await?)
    }

    /// Enqueue a Remote Procedure Call to be sent in future calls to `step`.
    pub fn enqueue<R: RemoteCall>(
        &mut self,
        request: &R,
    ) -> oneshot::Receiver<Result<Vec<u8>, InvocationError>> {
        self.enqueue_body(request.to_bytes())
    }

    fn enqueue_body(
        &mut self,
        body: Vec<u8>,
    ) -> oneshot::Receiver<Result<Vec<u8>, InvocationError>> {
        let (tx, rx) = oneshot::channel();
        self.requests.push(Request {
            body,
            state: RequestState::NotSerialized,
            result: tx,
        });
        rx
    }

    async fn step_until_receive(
        &mut self,
        mut rx: oneshot::Receiver<Result<Vec<u8>, InvocationError>>,
    ) -> Result<Vec<u8>, InvocationError> {
        loop {
            self.step().await?;
            match rx.try_recv() {
                Ok(x) => break x,
                Err(TryRecvError::Empty) => continue,
                Err(TryRecvError::Closed) => {
                    panic!("request channel dropped before receiving a result")
                }
            }
        }
    }

    /// Step network events, writing and reading at the same time.
    pub async fn step(&mut self) -> Result<(), ReadError> {
        self.try_fill_read();
        self.try_fill_write();

        // TODO probably want to properly set the request state on disconnect (read fail)

        let read_len = self.read_buffer.len() - self.read_index;
        let write_len = self.write_buffer.len() - self.write_index;

        let (mut reader, mut writer) = self.stream.split();
        if self.write_buffer.is_empty() {
            // TODO this always has to read the header of the packet and then the rest (2 or more calls)
            // it would be better to always perform calls in a circular buffer to have as much data from
            // the network as possible at all times, not just reading what's needed
            // (perhaps something similar could be done with the write buffer to write packet after packet)
            debug!("reading up to {} bytes from the network", read_len);
            let n = reader
                .read(&mut self.read_buffer[self.read_index..])
                .await?;

            self.on_net_read(n)
        } else {
            debug!(
                "reading up to {} bytes and sending up to {} bytes via network",
                read_len, write_len
            );
            let read = reader.read(&mut self.read_buffer[self.read_index..]);
            let write = writer.write(&self.write_buffer[self.write_index..]);
            pin_mut!(read);
            pin_mut!(write);
            match future::select(read, write).await {
                future::Either::Left((n, write)) => {
                    let n_write = write.now_or_never();

                    if let Some(n) = n_write {
                        self.on_net_write(n?);
                    }
                    self.on_net_read(n?)
                }
                future::Either::Right((n, read)) => {
                    let n_read = read.now_or_never();

                    self.on_net_write(n?);
                    if let Some(n) = n_read {
                        self.on_net_read(n?)
                    } else {
                        Ok(())
                    }
                }
            }
        }
    }

    /// Setup the read buffer for the transport, unless a read is already pending.
    fn try_fill_read(&mut self) {
        if !self.read_buffer.is_empty() {
            return;
        }

        let mut empty = Vec::new();
        match self.transport.unpack(&self.read_buffer, &mut empty) {
            Ok(_) => panic!("transports should not handle data 0 bytes long"),
            Err(transport::Error::MissingBytes(n)) => {
                (0..n).for_each(|_| self.read_buffer.push(0));
            }
            Err(_) => panic!("transports should not fail with data 0 bytes long"),
        }
    }

    /// Setup the write buffer for the transport, unless a write is already pending.
    fn try_fill_write(&mut self) {
        if !self.write_buffer.is_empty() {
            return;
        }

        // TODO avoid clone
        // TODO add a test to make sure we only ever send the same request once
        let requests = self
            .requests
            .iter()
            .filter_map(|r| match r.state {
                RequestState::NotSerialized => Some(r.body.clone()),
                RequestState::Serialized(_) | RequestState::Sent(_) => None,
            })
            .collect::<Vec<_>>();

        // TODO add a test to make sure we don't send empty data
        if requests.is_empty() {
            return;
        }

        let msg_ids = self.mtp.serialize(&requests, &mut self.mtp_buffer);
        self.write_buffer.clear();
        self.transport
            .pack(&self.mtp_buffer, &mut self.write_buffer);

        self.requests
            .iter_mut()
            .zip(msg_ids.into_iter())
            .for_each(|(req, msg_id)| {
                req.state = RequestState::Serialized(msg_id);
            });
    }

    /// Handle `n` more read bytes being ready to process by the transport.
    ///
    /// This won't cause `ReadError::Io`, but yet another enum would be overkill.
    fn on_net_read(&mut self, n: usize) -> Result<(), ReadError> {
        debug!("read {} bytes from the network", n);
        self.read_index += n;
        if self.read_index != self.read_buffer.len() {
            return Ok(());
        }

        debug!(
            "trying to unpack buffer of {} bytes...",
            self.read_buffer.len()
        );

        self.mtp_buffer.clear();
        match self
            .transport
            .unpack(&self.read_buffer, &mut self.mtp_buffer)
        {
            Ok(_) => {
                self.read_buffer.clear();
                self.read_index = 0;
                self.process_mtp_buffer().map_err(|e| e.into())
            }
            Err(transport::Error::MissingBytes(n)) => {
                let start = self.read_buffer.len();
                let missing = n - start;
                (0..missing).for_each(|_| self.read_buffer.push(0));
                Ok(())
            }
            Err(err) => return Err(err.into()),
        }
    }

    /// Handle `n` more written bytes being ready to process by the transport.
    fn on_net_write(&mut self, n: usize) {
        debug!("written {} bytes to the network", n);
        self.write_index += n;
        if self.write_index != self.write_buffer.len() {
            return;
        }

        self.write_buffer.clear();
        self.write_index = 0;
        for req in self.requests.iter_mut() {
            match req.state {
                RequestState::NotSerialized | RequestState::Sent(_) => {}
                RequestState::Serialized(msg_id) => {
                    req.state = RequestState::Sent(msg_id);
                }
            }
        }
    }

    /// Process the `mtp_buffer` contents and dispatch the results and errors.
    fn process_mtp_buffer(&mut self) -> Result<(), mtp::DeserializeError> {
        debug!("deserializing valid transport packet...");
        let result = self.mtp.deserialize(&self.mtp_buffer)?;
        for (msg_id, ret) in result.rpc_results {
            debug!("got result for request {:?}", msg_id);

            for i in (0..self.requests.len()).rev() {
                let req = &mut self.requests[i];
                match req.state {
                    RequestState::Serialized(sid) if sid == msg_id => {
                        panic!(format!(
                            "got rpc result {:?} for unsent request {:?}",
                            msg_id, sid
                        ));
                    }
                    RequestState::Sent(sid) if sid == msg_id => {
                        let result = match ret {
                            Ok(x) => Ok(x),
                            Err(mtp::RequestError::RpcError(error)) => {
                                Err(InvocationError::Rpc(error))
                            }
                            Err(mtp::RequestError::Dropped) => Err(InvocationError::Dropped),
                            Err(mtp::RequestError::Deserialize(error)) => {
                                Err(InvocationError::Read(error.into()))
                            }
                            Err(mtp::RequestError::BadMessage { .. }) => {
                                // TODO add a test to make sure we resend the request
                                info!("bad msg mtp error, re-sending request");
                                req.state = RequestState::NotSerialized;
                                break;
                            }
                        };

                        drop(req);
                        let req = self.requests.remove(i);
                        drop(req.result.send(result));
                        break;
                    }
                    _ => {}
                }
            }
        }
        Ok(())
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
    let response = sender.send(request).await?;
    debug!("gen auth key: starting step 2");
    let (request, data) = authentication::step2(data, &response)?;
    debug!("gen auth key: sending step 2");
    let response = sender.send(request).await?;
    debug!("gen auth key: starting step 3");
    let (request, data) = authentication::step3(data, &response)?;
    debug!("gen auth key: sending step 3");
    let response = sender.send(request).await?;
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
        requests: sender.requests,
        read_buffer: sender.read_buffer,
        read_index: sender.read_index,
        write_buffer: sender.write_buffer,
        write_index: sender.write_index,
    })
}

pub async fn connect_with_auth<T: Transport, A: ToSocketAddrs>(
    transport: T,
    addr: A,
    auth_key: AuthKey,
) -> Result<Sender<T, mtp::Encrypted>, io::Error> {
    Ok(Sender::connect(transport, mtp::Encrypted::build().finish(auth_key), addr).await?)
}
