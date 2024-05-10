// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod errors;
mod reconnection;

pub use crate::reconnection::*;
pub use errors::{AuthorizationError, InvocationError, ReadError};
use futures_util::future::{pending, select, Either};
use grammers_crypto::RingBuffer;
use grammers_mtproto::mtp::{self, Deserialization, Mtp};
use grammers_mtproto::transport::{self, Transport};
use grammers_mtproto::{authentication, MsgId};
use grammers_tl_types::{self as tl, Deserializable, RemoteCall};
use log::{debug, info, trace, warn};
use std::io;
use std::io::Error;
use std::ops::ControlFlow;
use std::pin::pin;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::SystemTime;
use tl::Serializable;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::tcp::{ReadHalf, WriteHalf};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::time::{sleep_until, Duration, Instant};

#[cfg(feature = "proxy")]
use {
    std::io::ErrorKind,
    std::net::{IpAddr, SocketAddr},
    tokio_socks::tcp::Socks5Stream,
    trust_dns_resolver::config::{ResolverConfig, ResolverOpts},
    trust_dns_resolver::AsyncResolver,
    url::Host,
};

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

/// How much leading space should be reserved in a buffer to avoid moving memory.
const LEADING_BUFFER_SPACE: usize = mtp::MAX_TRANSPORT_HEADER_LEN
    + mtp::ENCRYPTED_PACKET_HEADER_LEN
    + mtp::PLAIN_PACKET_HEADER_LEN
    + mtp::MESSAGE_CONTAINER_HEADER_LEN;

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

pub enum NetStream {
    Tcp(TcpStream),
    #[cfg(feature = "proxy")]
    ProxySocks5(Socks5Stream<TcpStream>),
}

impl NetStream {
    fn split(&mut self) -> (ReadHalf, WriteHalf) {
        match self {
            Self::Tcp(stream) => stream.split(),
            #[cfg(feature = "proxy")]
            Self::ProxySocks5(stream) => stream.split(),
        }
    }
}

// Manages enqueuing requests, matching them to their response, and IO.

pub struct Sender<T: Transport, M: Mtp> {
    stream: NetStream,
    transport: T,
    mtp: M,
    addr: std::net::SocketAddr,
    #[cfg(feature = "proxy")]
    proxy_url: Option<String>,
    requests: Vec<Request>,
    request_rx: mpsc::UnboundedReceiver<Request>,
    next_ping: Instant,
    reconnection_policy: &'static dyn ReconnectionPolicy,

    // Transport-level buffers and positions
    read_buffer: RingBuffer<u8>,
    read_index: usize,
    write_buffer: RingBuffer<u8>,
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
    async fn connect<'a>(
        transport: T,
        mtp: M,
        addr: std::net::SocketAddr,
        reconnection_policy: &'static dyn ReconnectionPolicy,
    ) -> Result<(Self, Enqueuer), io::Error> {
        let stream = connect_stream(&addr).await?;
        let (tx, rx) = mpsc::unbounded_channel();
        let mut read_buffer = RingBuffer::with_capacity(MAXIMUM_DATA, LEADING_BUFFER_SPACE);
        read_buffer.fill_remaining();
        Ok((
            Self {
                stream,
                transport,
                mtp,
                addr,
                #[cfg(feature = "proxy")]
                proxy_url: None,
                requests: vec![],
                request_rx: rx,
                next_ping: Instant::now() + PING_DELAY,
                reconnection_policy,

                read_buffer,
                read_index: 0,
                write_buffer: RingBuffer::with_capacity(MAXIMUM_DATA, LEADING_BUFFER_SPACE),
                write_index: 0,
            },
            Enqueuer(tx),
        ))
    }

    #[cfg(feature = "proxy")]
    async fn connect_via_proxy<'a>(
        transport: T,
        mtp: M,
        addr: SocketAddr,
        proxy_url: &str,
        reconnection_policy: &'static dyn ReconnectionPolicy,
    ) -> Result<(Self, Enqueuer), io::Error> {
        info!("connecting...");

        let stream = connect_proxy_stream(&addr, proxy_url).await?;
        let (tx, rx) = mpsc::unbounded_channel();
        let mut read_buffer = RingBuffer::with_capacity(MAXIMUM_DATA, LEADING_BUFFER_SPACE);
        read_buffer.fill_remaining();
        Ok((
            Self {
                stream,
                transport,
                mtp,
                addr,
                proxy_url: Some(proxy_url.to_string()),
                requests: vec![],
                request_rx: rx,
                next_ping: Instant::now() + PING_DELAY,
                reconnection_policy,

                read_buffer,
                read_index: 0,
                write_buffer: RingBuffer::with_capacity(MAXIMUM_DATA, LEADING_BUFFER_SPACE),
                write_index: 0,
            },
            Enqueuer(tx),
        ))
    }

    pub async fn invoke<R: RemoteCall>(&mut self, request: &R) -> Result<Vec<u8>, InvocationError> {
        let rx = self.enqueue_body(request.to_bytes());
        self.step_until_receive(rx).await
    }

    /// Like `invoke` but raw data.
    async fn send(&mut self, body: Vec<u8>) -> Result<Vec<u8>, InvocationError> {
        let rx = self.enqueue_body(body);
        self.step_until_receive(rx).await
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
        enum Sel {
            Sleep,
            Request(Option<Request>),
            Read(io::Result<usize>),
            Write(io::Result<usize>),
        }

        let mut attempts = 0u8;
        loop {
            if attempts > 5 {
                log::error!(
                    "attempted more than {} times for reconnection and failed",
                    attempts
                );
                return Err(ReadError::Io(io::Error::new(
                    io::ErrorKind::ConnectionReset,
                    "read 0 bytes",
                )));
            }

            self.try_fill_write();

            // TODO probably want to properly set the request state on disconnect (read fail)

            let write_len = self.write_buffer.len() - self.write_index;

            let (mut reader, mut writer) = self.stream.split();
            // TODO this always has to read the header of the packet and then the rest (2 or more calls)
            // it would be better to always perform calls in a circular buffer to have as much data from
            // the network as possible at all times, not just reading what's needed
            // (perhaps something similar could be done with the write buffer to write packet after packet)
            //
            // The `request_rx.recv()` can't return `None` because we're holding a `tx`.
            trace!(
                "reading bytes and sending up to {} bytes via network",
                write_len
            );

            let sel = {
                let sleep = pin!(async { sleep_until(self.next_ping).await });
                let recv_req = pin!(async { self.request_rx.recv().await });
                let recv_data =
                    pin!(async { reader.read(&mut self.read_buffer[self.read_index..]).await });
                let send_data = pin!(async {
                    if self.write_buffer.is_empty() {
                        pending().await
                    } else {
                        writer.write(&self.write_buffer[self.write_index..]).await
                    }
                });

                match select(select(sleep, recv_req), select(recv_data, send_data)).await {
                    Either::Left((Either::Left(_), _)) => Sel::Sleep,
                    Either::Left((Either::Right((request, _)), _)) => Sel::Request(request),
                    Either::Right((Either::Left((n, _)), _)) => Sel::Read(n),
                    Either::Right((Either::Right((n, _)), _)) => Sel::Write(n),
                }
            };

            let res = match sel {
                Sel::Request(request) => {
                    self.requests.push(request.unwrap());
                    Ok(Vec::new())
                }
                Sel::Read(n) => n.map_err(ReadError::Io).and_then(|n| self.on_net_read(n)),
                Sel::Write(n) => n.map_err(ReadError::Io).map(|n| {
                    self.on_net_write(n);
                    Vec::new()
                }),
                Sel::Sleep => {
                    self.on_ping_timeout();
                    Ok(Vec::new())
                }
            };

            match res {
                Ok(ok) => break Ok(ok),
                Err(err) => {
                    match err {
                        ReadError::Io(_) => {}
                        _ => {
                            log::warn!("unhandled error: {}", &err);
                            break Err(err);
                        }
                    }

                    self.reset_state();

                    self.try_connect().await?;
                }
            }

            log::info!("retrying the call");

            attempts += 1;
        }
    }

    #[allow(unused_variables)]
    async fn try_connect(&mut self) -> Result<(), Error> {
        let mut attempts = 0;
        loop {
            #[cfg(feature = "proxy")]
            let res = if self.proxy_url.is_some() {
                connect_proxy_stream(&self.addr, self.proxy_url.as_ref().unwrap()).await
            } else {
                connect_stream(&self.addr).await
            };

            #[cfg(not(feature = "proxy"))]
            let res = connect_stream(&self.addr).await;

            match res {
                Ok(result) => {
                    self.stream = result;
                    return Ok(());
                }
                Err(e) => {
                    log::warn!("err: {}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;

                    attempts += 1;

                    match self.reconnection_policy.should_retry(attempts) {
                        ControlFlow::Break(_) => {
                            log::error!(
                                "attempted more than {} times for reconnection and failed",
                                attempts
                            );
                            return Err(e);
                        }
                        ControlFlow::Continue(duration) => {
                            tokio::time::sleep(duration).await;
                        }
                    }
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
        for request in self
            .requests
            .iter_mut()
            .filter(|r| matches!(r.state, RequestState::NotSerialized))
        {
            // TODO make mtp itself use BytesMut to avoid copies
            if let Some(msg_id) = self.mtp.push(&mut self.write_buffer, &request.body) {
                assert!(request.body.len() >= 4);
                let req_id = u32::from_le_bytes([
                    request.body[0],
                    request.body[1],
                    request.body[2],
                    request.body[3],
                ]);
                debug!(
                    "serialized request {:x} ({}) with {:?}",
                    req_id,
                    tl::name_for_id(req_id),
                    msg_id
                );
                // Note how only NotSerialized become Serialized.
                // Nasty bugs that take ~2h to find occur otherwise!
                // (e.g. infinite loops leading to transport flood.)
                request.state = RequestState::Serialized(msg_id);
            } else {
                break;
            }
        }

        self.mtp.finalize(&mut self.write_buffer);
        if !self.write_buffer.is_empty() {
            self.transport.pack(&mut self.write_buffer)
        }
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

        self.read_index += n;
        trace!("read {} bytes from the network", n);
        trace!("trying to unpack buffer of {} bytes...", self.read_index);

        // TODO the buffer might have multiple transport packets, what should happen with the
        // updates successfully read if subsequent packets fail to be deserialized properly?
        let mut updates = Vec::new();
        while self.read_index != 0 {
            match self.transport.unpack(&self.read_buffer[..self.read_index]) {
                Ok(offset) => {
                    debug!("deserializing valid transport packet...");
                    let result = self
                        .mtp
                        .deserialize(&self.read_buffer[offset.data_start..offset.data_end])?;

                    self.process_mtp_buffer(result, &mut updates);
                    self.read_buffer.skip(offset.next_offset);
                    self.read_index -= offset.next_offset;
                }
                Err(transport::Error::MissingBytes) => break,
                Err(err) => return Err(err.into()),
            }
        }

        self.read_buffer.reclaim_leading();
        self.read_buffer.fill_remaining();

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

    /// Process the result of deserializing an MTP buffer.
    fn process_mtp_buffer(
        &mut self,
        result: Deserialization,
        updates: &mut Vec<tl::enums::Updates>,
    ) {
        updates.extend(result.updates.iter().filter_map(|update| {
            match tl::enums::Updates::from_bytes(update) {
                Ok(u) => Some(u),
                Err(e) => {
                    // Annoyingly enough, `messages.affectedMessages` also has `pts`.
                    // Mostly received when deleting messages, so pretend that's the
                    // update that actually occured.
                    match tl::enums::messages::AffectedMessages::from_bytes(update) {
                        Ok(tl::enums::messages::AffectedMessages::Messages(
                            tl::types::messages::AffectedMessages { pts, pts_count },
                        )) => Some(
                            tl::types::UpdateShort {
                                update: tl::types::UpdateDeleteMessages {
                                    messages: Vec::new(),
                                    pts,
                                    pts_count,
                                }
                                .into(),
                                date: 0,
                            }
                            .into(),
                        ),
                        Err(_) => match tl::types::messages::InvitedUsers::from_bytes(update) {
                            Ok(u) => Some(u.updates),
                            Err(_) => {
                                warn!(
                                    "telegram sent updates that failed to be deserialized: {}",
                                    e
                                );
                                None
                            }
                        },
                    }
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
    }

    fn reset_state(&mut self) {
        self.transport.reset();
        self.mtp.reset();
        self.read_buffer.clear();
        self.write_index = 0;
        self.write_buffer.clear();
        self.requests
            .iter_mut()
            .for_each(|r| r.state = RequestState::NotSerialized);
    }
}

impl<T: Transport> Sender<T, mtp::Encrypted> {
    pub fn auth_key(&self) -> [u8; 256] {
        self.mtp.auth_key()
    }
}

pub async fn connect<T: Transport>(
    transport: T,
    addr: std::net::SocketAddr,
    rc_policy: &'static dyn ReconnectionPolicy,
) -> Result<(Sender<T, mtp::Encrypted>, Enqueuer), AuthorizationError> {
    let (sender, enqueuer) = Sender::connect(transport, mtp::Plain::new(), addr, rc_policy).await?;
    generate_auth_key(sender, enqueuer).await
}

#[cfg(feature = "proxy")]
pub async fn connect_via_proxy<'a, T: Transport>(
    transport: T,
    addr: std::net::SocketAddr,
    proxy_url: &str,
    rc_policy: &'static dyn ReconnectionPolicy,
) -> Result<(Sender<T, mtp::Encrypted>, Enqueuer), AuthorizationError> {
    let (sender, enqueuer) =
        Sender::connect_via_proxy(transport, mtp::Plain::new(), addr, proxy_url, rc_policy).await?;
    generate_auth_key(sender, enqueuer).await
}

async fn connect_stream(addr: &std::net::SocketAddr) -> Result<NetStream, std::io::Error> {
    info!("connecting...");
    Ok(NetStream::Tcp(TcpStream::connect(addr).await?))
}

#[cfg(feature = "proxy")]
async fn connect_proxy_stream(
    addr: &SocketAddr,
    proxy_url: &str,
) -> Result<NetStream, std::io::Error> {
    let proxy =
        url::Url::parse(proxy_url).map_err(|err| io::Error::new(ErrorKind::InvalidData, err))?;
    let scheme = proxy.scheme();
    let host = proxy.host().ok_or(io::Error::new(
        ErrorKind::NotFound,
        format!("proxy host is missing from url: {}", proxy_url),
    ))?;
    let port = proxy.port().ok_or(io::Error::new(
        ErrorKind::NotFound,
        format!("proxy port is missing from url: {}", proxy_url),
    ))?;
    let username = proxy.username();
    let password = proxy.password().unwrap_or("");
    let socks_addr = match host {
        Host::Domain(domain) => {
            let resolver = AsyncResolver::tokio(ResolverConfig::default(), ResolverOpts::default());
            let response = resolver.lookup_ip(domain).await?;
            let socks_ip_addr = response.into_iter().next().ok_or(io::Error::new(
                ErrorKind::NotFound,
                format!("proxy host did not return any ip address: {}", domain),
            ))?;
            SocketAddr::new(socks_ip_addr, port)
        }
        Host::Ipv4(v4) => SocketAddr::new(IpAddr::from(v4), port),
        Host::Ipv6(v6) => SocketAddr::new(IpAddr::from(v6), port),
    };

    match scheme {
        "socks5" => {
            if username.is_empty() {
                Ok(NetStream::ProxySocks5(
                    tokio_socks::tcp::Socks5Stream::connect(socks_addr, addr)
                        .await
                        .map_err(|err| io::Error::new(ErrorKind::ConnectionAborted, err))?,
                ))
            } else {
                Ok(NetStream::ProxySocks5(
                    tokio_socks::tcp::Socks5Stream::connect_with_password(
                        socks_addr, addr, username, password,
                    )
                    .await
                    .map_err(|err| io::Error::new(ErrorKind::ConnectionAborted, err))?,
                ))
            }
        }
        scheme => Err(io::Error::new(
            ErrorKind::ConnectionAborted,
            format!("proxy scheme not supported: {}", scheme),
        )),
    }
}

pub async fn generate_auth_key<T: Transport>(
    mut sender: Sender<T, mtp::Plain>,
    enqueuer: Enqueuer,
) -> Result<(Sender<T, mtp::Encrypted>, Enqueuer), AuthorizationError> {
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
            requests: sender.requests,
            request_rx: sender.request_rx,
            next_ping: Instant::now() + PING_DELAY,
            read_buffer: sender.read_buffer,
            read_index: sender.read_index,
            write_buffer: sender.write_buffer,
            write_index: sender.write_index,
            addr: sender.addr,
            #[cfg(feature = "proxy")]
            proxy_url: sender.proxy_url,
            reconnection_policy: sender.reconnection_policy,
        },
        enqueuer,
    ))
}

pub async fn connect_with_auth<T: Transport>(
    transport: T,
    addr: std::net::SocketAddr,
    auth_key: [u8; 256],
    rc_policy: &'static dyn ReconnectionPolicy,
) -> Result<(Sender<T, mtp::Encrypted>, Enqueuer), io::Error> {
    Sender::connect(
        transport,
        mtp::Encrypted::build().finish(auth_key),
        addr,
        rc_policy,
    )
    .await
}

#[cfg(feature = "proxy")]
pub async fn connect_via_proxy_with_auth<'a, T: Transport>(
    transport: T,
    addr: std::net::SocketAddr,
    auth_key: [u8; 256],
    proxy_url: &str,
    rc_policy: &'static dyn ReconnectionPolicy,
) -> Result<(Sender<T, mtp::Encrypted>, Enqueuer), io::Error> {
    Sender::connect_via_proxy(
        transport,
        mtp::Encrypted::build().finish(auth_key),
        addr,
        proxy_url,
        rc_policy,
    )
    .await
}
