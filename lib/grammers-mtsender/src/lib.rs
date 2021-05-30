// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod errors;

use bytes::{Buf, BytesMut};
pub use errors::{AuthorizationError, InvocationError, ReadError};
use grammers_mtproto::mtp::{self, Mtp};
use grammers_mtproto::transport::{self, Transport};
use grammers_mtproto::{authentication, MsgId};
use grammers_tl_types::{self as tl, Deserializable, RemoteCall};
use log::{debug, info, trace, warn};
use std::io;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::SystemTime;
use tl::Serializable;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::time::{sleep_until, Duration, Instant};

/// The maximum data that we're willing to send or receive at once.
///
/// By having a fixed-size buffer, we can avoid unnecessary allocations
/// and trivially prevent allocating more than this limit if we ever
/// received invalid data.
///
/// Telegram will close the connection with roughly a megabyte of data,
/// so to account for the transports' own overhead, we add a few extra
/// kilobytes to the maximum data size.
const MAXIMUM_DATA: usize = (1024 * 1024) + (8 * 1024);

/// Every how often are pings sent?
const PING_DELAY: Duration = Duration::from_secs(60);

/// After how many seconds should the server close the connection when we send a ping?
///
/// What this value essentially means is that we have `NO_PING_DISCONNECT - PING_DELAY` seconds
/// to keep sending pings, or the server will close the connection.
///
/// Pings ensure the connection is kept active, and the delayed disconnect ensures the messages
/// are getting through consistently enough.
const NO_PING_DISCONNECT: i32 = 75;

/// Generate a "random" ping ID.
pub(crate) fn generate_random_id() -> i64 {
    static LAST_ID: AtomicI64 = AtomicI64::new(0);

    if LAST_ID.load(Ordering::SeqCst) == 0 {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .expect("system time is before epoch")
            .as_nanos() as i64;

        LAST_ID
            .compare_exchange(0, now, Ordering::SeqCst, Ordering::SeqCst)
            .unwrap();
    }

    LAST_ID.fetch_add(1, Ordering::SeqCst)
}

// Manages enqueuing requests, matching them to their response, and IO.
pub struct Sender<T: Transport, M: Mtp> {
    stream: TcpStream,
    transport: T,
    mtp: M,
    mtp_buffer: BytesMut,

    requests: Vec<Request>,
    // Need to keep one sender to ensure there will always be at least one channel alive.
    // Otherwise the receiver would always resolve to `None`.
    request_tx: mpsc::UnboundedSender<Request>,
    request_rx: mpsc::UnboundedReceiver<Request>,
    next_ping: Instant,

    // Transport-level buffers and positions
    read_buffer: BytesMut,
    write_buffer: BytesMut,
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

pub struct Enqueuer(mpsc::UnboundedSender<Request>);

impl Enqueuer {
    /// Enqueue a Remote Procedure Call to be sent in future calls to `step`.
    pub fn enqueue<R: RemoteCall>(
        &self,
        request: &R,
    ) -> oneshot::Receiver<Result<Vec<u8>, InvocationError>> {
        // TODO we probably want a bound here (to not enqueue more than N at once)
        let body = request.to_bytes();
        assert!(body.len() >= 4);
        let req_id = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        debug!(
            "enqueueing request {} to be serialized",
            tl::name_for_id(req_id)
        );

        let (tx, rx) = oneshot::channel();
        if let Err(err) = self.0.send(Request {
            body,
            state: RequestState::NotSerialized,
            result: tx,
        }) {
            err.0.result.send(Err(InvocationError::Dropped)).unwrap();
        }
        rx
    }
}

impl<T: Transport, M: Mtp> Sender<T, M> {
    async fn connect<A: ToSocketAddrs>(
        transport: T,
        mtp: M,
        addr: A,
    ) -> Result<(Self, Enqueuer), io::Error> {
        info!("connecting...");
        let stream = TcpStream::connect(addr).await?;
        let (tx, rx) = mpsc::unbounded_channel();

        Ok((
            Self {
                stream,
                transport,
                mtp,
                mtp_buffer: BytesMut::with_capacity(MAXIMUM_DATA),

                requests: vec![],
                request_tx: tx.clone(),
                request_rx: rx,
                next_ping: Instant::now() + PING_DELAY,

                read_buffer: BytesMut::with_capacity(MAXIMUM_DATA),
                write_buffer: BytesMut::with_capacity(MAXIMUM_DATA),
                write_index: 0,
            },
            Enqueuer(tx),
        ))
    }

    pub async fn invoke<R: RemoteCall>(&mut self, request: &R) -> Result<Vec<u8>, InvocationError> {
        let rx = self.enqueue_body(request.to_bytes());
        Ok(self.step_until_receive(rx).await?)
    }

    /// Like `invoke` but raw data.
    async fn send(&mut self, body: Vec<u8>) -> Result<Vec<u8>, InvocationError> {
        let rx = self.enqueue_body(body);
        Ok(self.step_until_receive(rx).await?)
    }

    fn enqueue_body(
        &mut self,
        body: Vec<u8>,
    ) -> oneshot::Receiver<Result<Vec<u8>, InvocationError>> {
        assert!(body.len() >= 4);
        let req_id = u32::from_le_bytes([body[0], body[1], body[2], body[3]]);
        debug!(
            "enqueueing request {} to be serialized",
            tl::name_for_id(req_id)
        );

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
    ///
    /// Updates received during this step, if any, are returned.
    pub async fn step(&mut self) -> Result<Vec<tl::enums::Updates>, ReadError> {
        self.try_fill_write();

        // TODO probably want to properly set the request state on disconnect (read fail)

        let write_len = self.write_buffer.len() - self.write_index;

        let (mut reader, mut writer) = self.stream.split();
        if self.write_buffer.is_empty() {
            // TODO this always has to read the header of the packet and then the rest (2 or more calls)
            // it would be better to always perform calls in a circular buffer to have as much data from
            // the network as possible at all times, not just reading what's needed
            // (perhaps something similar could be done with the write buffer to write packet after packet)
            //
            // The `request_rx.recv()` can't return `None` because we're holding a `tx`.
            trace!("reading bytes from the network");
            tokio::select!(
                request = self.request_rx.recv() => {
                    self.requests.push(request.unwrap());
                    Ok(Vec::new())
                },
                n = reader.read_buf(&mut self.read_buffer) => {
                    self.on_net_read(n?)
                },
                _ = sleep_until(self.next_ping) => {
                    self.on_ping_timeout();
                    Ok(Vec::new())
                }
            )
        } else {
            trace!(
                "reading bytes and sending up to {} bytes via network",
                write_len
            );
            tokio::select! {
                request = self.request_rx.recv() => {
                    self.requests.push(request.unwrap());
                    Ok(Vec::new())
                },
                n = reader.read_buf(&mut self.read_buffer) => {
                    self.on_net_read(n?)
                }
                n = writer.write(&self.write_buffer[self.write_index..]) => {
                    self.on_net_write(n?);
                    Ok(Vec::new())
                }
                _ = sleep_until(self.next_ping) => {
                    self.on_ping_timeout();
                    Ok(Vec::new())
                }
            }
        }
    }

    /// Setup the write buffer for the transport, unless a write is already pending.
    fn try_fill_write(&mut self) {
        if !self.write_buffer.is_empty() {
            return;
        }

        // TODO add a test to make sure we only ever send the same request once
        let requests = self
            .requests
            .iter_mut()
            .filter(|r| match r.state {
                RequestState::NotSerialized => true,
                RequestState::Serialized(_) | RequestState::Sent(_) => false,
            })
            .collect::<Vec<_>>();

        // TODO add a test to make sure we don't send empty data
        if requests.is_empty() {
            return;
        }

        // TODO make mtp itself use BytesMut to avoid copies
        let mut msg_ids = Vec::new();
        for request in requests.iter() {
            if let Some(msg_id) = self.mtp.push(&request.body) {
                msg_ids.push(msg_id);
            } else {
                break;
            }
        }
        let temp_vec = self.mtp.finalize();
        self.mtp_buffer = temp_vec[..].into();
        self.write_buffer.clear();
        self.transport
            .pack(&self.mtp_buffer, &mut self.write_buffer);

        // NOTE: we have to use the FILTERED requests, not the saved ones.
        // The key to finding this was printing the old and new state (but took ~2h to find).
        // Otherwise we will likely change from Sent to Serialized and enter an infinite loop.
        // This will very easily cause transport flood (using self, trying to upload two files at once).
        // TODO add a test for this
        requests
            .into_iter()
            .zip(msg_ids.into_iter())
            .for_each(|(req, msg_id)| {
                assert!(req.body.len() >= 4);
                let req_id =
                    u32::from_le_bytes([req.body[0], req.body[1], req.body[2], req.body[3]]);
                debug!(
                    "serialized request {:x} ({}) with {:?}",
                    req_id,
                    tl::name_for_id(req_id),
                    msg_id
                );
                req.state = RequestState::Serialized(msg_id);
            });
    }

    /// Handle `n` more read bytes being ready to process by the transport.
    ///
    /// This won't cause `ReadError::Io`, but yet another enum would be overkill.
    fn on_net_read(&mut self, n: usize) -> Result<Vec<tl::enums::Updates>, ReadError> {
        if n == 0 {
            return Err(ReadError::Io(io::Error::new(
                io::ErrorKind::ConnectionReset,
                "read 0 bytes",
            )));
        }

        trace!("read {} bytes from the network", n);

        trace!(
            "trying to unpack buffer of {} bytes...",
            self.read_buffer.len()
        );

        // TODO the buffer might have multiple transport packets, what should happen with the
        // updates successfully read if subsequent packets fail to be deserialized properly?
        let mut updates = Vec::new();
        while !self.read_buffer.is_empty() {
            self.mtp_buffer.clear();
            match self
                .transport
                .unpack(&self.read_buffer, &mut self.mtp_buffer)
            {
                Ok(n) => {
                    self.read_buffer.advance(n);
                    self.process_mtp_buffer(&mut updates)?;
                }
                Err(transport::Error::MissingBytes) => break,
                Err(err) => return Err(err.into()),
            }
        }

        Ok(updates)
    }

    /// Handle `n` more written bytes being ready to process by the transport.
    fn on_net_write(&mut self, n: usize) {
        self.write_index += n;
        trace!(
            "written {} bytes to the network ({}/{})",
            n,
            self.write_index,
            self.write_buffer.len()
        );
        assert!(self.write_index <= self.write_buffer.len());
        if self.write_index != self.write_buffer.len() {
            return;
        }

        self.write_buffer.clear();
        self.write_index = 0;
        for req in self.requests.iter_mut() {
            match req.state {
                RequestState::NotSerialized | RequestState::Sent(_) => {}
                RequestState::Serialized(msg_id) => {
                    debug!("sent request with {:?}", msg_id);
                    req.state = RequestState::Sent(msg_id);
                }
            }
        }
    }

    /// Handle a ping timeout, meaning we need to enqueue a new ping request.
    fn on_ping_timeout(&mut self) {
        let ping_id = generate_random_id();
        debug!("enqueueing keepalive ping {}", ping_id);
        drop(
            self.enqueue_body(
                tl::functions::PingDelayDisconnect {
                    ping_id,
                    disconnect_delay: NO_PING_DISCONNECT,
                }
                .to_bytes(),
            ),
        );
        self.next_ping = Instant::now() + PING_DELAY;
    }

    /// Process the `mtp_buffer` contents and dispatch the results and errors.
    fn process_mtp_buffer(
        &mut self,
        updates: &mut Vec<tl::enums::Updates>,
    ) -> Result<(), mtp::DeserializeError> {
        debug!("deserializing valid transport packet...");
        let result = self.mtp.deserialize(&self.mtp_buffer)?;

        updates.extend(result.updates.iter().filter_map(|update| {
            match tl::enums::Updates::from_bytes(&update) {
                Ok(u) => Some(u),
                Err(e) => {
                    warn!(
                        "telegram sent updates that failed to be deserialized: {}",
                        e
                    );
                    None
                }
            }
        }));

        for (msg_id, ret) in result.rpc_results {
            let mut found = false;
            for i in (0..self.requests.len()).rev() {
                let req = &mut self.requests[i];
                match req.state {
                    RequestState::Serialized(sid) if sid == msg_id => {
                        panic!("got rpc result {:?} for unsent request {:?}", msg_id, sid);
                    }
                    RequestState::Sent(sid) if sid == msg_id => {
                        found = true;
                        let result = match ret {
                            Ok(x) => {
                                assert!(x.len() >= 4);
                                let res_id = u32::from_le_bytes([x[0], x[1], x[2], x[3]]);
                                debug!(
                                    "got result {:x} ({}) for request {:?}",
                                    res_id,
                                    tl::name_for_id(res_id),
                                    msg_id
                                );
                                Ok(x)
                            }
                            Err(mtp::RequestError::RpcError(mut error)) => {
                                debug!("got rpc error {:?} for request {:?}", error, msg_id);
                                let x = req.body.as_slice();
                                error.caused_by =
                                    Some(u32::from_le_bytes([x[0], x[1], x[2], x[3]]));
                                Err(InvocationError::Rpc(error))
                            }
                            Err(mtp::RequestError::Dropped) => {
                                debug!("response for request {:?} dropped", msg_id);
                                Err(InvocationError::Dropped)
                            }
                            Err(mtp::RequestError::Deserialize(error)) => {
                                debug!(
                                    "got deserialize error {:?} for request {:?}",
                                    error, msg_id
                                );
                                Err(InvocationError::Read(error.into()))
                            }
                            Err(err @ mtp::RequestError::BadMessage { .. }) => {
                                // TODO add a test to make sure we resend the request
                                info!("{}; re-sending request {:?}", err, msg_id);
                                req.state = RequestState::NotSerialized;
                                break;
                            }
                        };

                        let req = self.requests.remove(i);
                        drop(req.result.send(result));
                        break;
                    }
                    _ => {}
                }
            }

            if !found {
                info!("got rpc result {:?} but no such request is saved", msg_id);
            }
        }

        Ok(())
    }
}

impl<T: Transport> Sender<T, mtp::Encrypted> {
    pub fn auth_key(&self) -> [u8; 256] {
        self.mtp.auth_key()
    }
}

pub async fn connect<T: Transport, A: ToSocketAddrs>(
    transport: T,
    addr: A,
) -> Result<(Sender<T, mtp::Encrypted>, Enqueuer), AuthorizationError> {
    let (mut sender, enqueuer) = Sender::connect(transport, mtp::Plain::new(), addr).await?;

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
    let authentication::Finished {
        auth_key,
        time_offset,
        first_salt,
    } = authentication::create_key(data, &response)?;
    info!("authorization key generated successfully");

    Ok((
        Sender {
            stream: sender.stream,
            transport: sender.transport,
            mtp: mtp::Encrypted::build()
                .time_offset(time_offset)
                .first_salt(first_salt)
                .finish(auth_key),
            mtp_buffer: sender.mtp_buffer,
            requests: sender.requests,
            request_tx: sender.request_tx,
            request_rx: sender.request_rx,
            next_ping: Instant::now() + PING_DELAY,
            read_buffer: sender.read_buffer,
            write_buffer: sender.write_buffer,
            write_index: sender.write_index,
        },
        enqueuer,
    ))
}

pub async fn connect_with_auth<T: Transport, A: ToSocketAddrs>(
    transport: T,
    addr: A,
    auth_key: [u8; 256],
) -> Result<(Sender<T, mtp::Encrypted>, Enqueuer), io::Error> {
    Ok(Sender::connect(transport, mtp::Encrypted::build().finish(auth_key), addr).await?)
}
