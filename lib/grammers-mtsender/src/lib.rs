// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod errors;
//mod tcp_transport;

use async_std::net::TcpStream;
pub use errors::{AuthorizationError, InvocationError};
use futures::channel::{mpsc, oneshot};
use futures::future;
use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use futures::lock::Mutex;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use grammers_crypto::{auth_key, AuthKey};
use grammers_mtproto::transports::{Decoder, Encoder, TransportFull};
use grammers_mtproto::MsgId;
use grammers_mtproto::Mtp;
use std::collections::BTreeMap;
use std::io;
use std::net::ToSocketAddrs;
use std::sync::Arc;
use grammers_tl_types::{RemoteCall, Serializable};

/*
use tcp_transport::TcpTransport;

use grammers_mtproto::errors::RequestError;
use grammers_mtproto::transports::TransportFull;
pub use grammers_mtproto::DEFAULT_COMPRESSION_THRESHOLD;
use grammers_tl_types::{Deserializable, RemoteCall};

use std::io;
use std::net::ToSocketAddrs;

/// A builder to configure `MTSender` instances.
pub struct MtpSenderBuilder {
    compression_threshold: Option<usize>,
    auth_key: Option<AuthKey>,
}

/// A Mobile Transport sender, using the [Mobile Transport Protocol]
/// underneath.
///
/// [Mobile Transport Protocol]: https://core.telegram.org/mtproto
pub struct MtpSender {
    protocol: Mtp,
    // TODO let the user change the type of transport used
    transport: TcpTransport<TransportFull>,
}

impl MtpSenderBuilder {
    fn new() -> Self {
        MtpSenderBuilder {
            compression_threshold: DEFAULT_COMPRESSION_THRESHOLD,
            auth_key: None,
        }
    }

    /// Configures the compression threshold for outgoing messages.
    pub fn compression_threshold(mut self, threshold: Option<usize>) -> Self {
        self.compression_threshold = threshold;
        self
    }

    /// Sets the authorization key to be used. Otherwise, no authorization
    /// key will be present, and a new one will have to be generated before
    /// being able to send encrypted messages.
    pub fn auth_key(mut self, auth_key: AuthKey) -> Self {
        self.auth_key = Some(auth_key);
        self
    }

    /// Finishes the builder and returns the `MTProto` instance with all
    /// the configuration changes applied.
    pub async fn connect<A: ToSocketAddrs>(self, addr: A) -> io::Result<MtpSender> {
        MtpSender::with_builder(self, addr).await
    }
}

impl MtpSender {
    /// Returns a builder to configure certain parameters.
    pub fn build() -> MtpSenderBuilder {
        MtpSenderBuilder::new()
    }

    /// Creates and connects a new instance with default settings.
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        Self::build().connect(addr).await
    }

    /// Constructs an instance using a finished builder.
    async fn with_builder<A: ToSocketAddrs>(builder: MtpSenderBuilder, addr: A) -> io::Result<Self> {
        let addr = addr.to_socket_addrs()?.next().unwrap();
        let transport = TcpTransport::connect(addr).await?;

        let mut protocol = Mtp::build().compression_threshold(builder.compression_threshold);

        if let Some(auth_key) = builder.auth_key {
            protocol = protocol.auth_key(auth_key);
        }

        let protocol = protocol.finish();

        Ok(MtpSender {
            protocol,
            transport,
        })
    }

    /// Changes the authorization key data for a different one.
    pub fn set_auth_key(&mut self, data: [u8; 256]) {
        self.protocol.set_auth_key(AuthKey::from_bytes(data), 0);
    }

    /// Block invoking a single Remote Procedure Call and return its result.
    ///
    /// The invocation might fail due to network problems, in which case the
    /// outermost result represents failure.
    ///
    /// If the request is both sent and received successfully, then the
    /// request itself was understood by the server, but it could not be
    /// executed. This is represented by the innermost result.
    pub async fn invoke<R: RemoteCall>(&mut self, request: &R) -> Result<R::Return, InvocationError> {
        let mut msg_id = self.protocol.enqueue_request(request.to_bytes())?;
        loop {
            // The protocol may generate more outgoing requests, so we need
            // to constantly check for those until we receive a response.
            while let Some(payload) = self.protocol.serialize_encrypted_messages()? {
                self.transport.send(&payload).await?;
            }

            // Process all messages we receive.
            let response = self.receive_message().await?;
            self.protocol.process_encrypted_response(&response)?;

            // TODO dispatch this somehow
            while let Some(data) = self.protocol.poll_update() {
                eprintln!("Received update data: {:?}", data);
            }

            // See if there are responses to our request.
            while let Some((response_id, data)) = self.protocol.poll_response() {
                if response_id == msg_id {
                    match data {
                        Ok(x) => {
                            return Ok(R::Return::from_bytes(&x)?);
                        }
                        Err(RequestError::RPCError(error)) => {
                            return Err(InvocationError::RPC(error));
                        }
                        Err(RequestError::Dropped) => {
                            return Err(InvocationError::Dropped);
                        }
                        Err(RequestError::BadMessage { .. }) => {
                            // Need to retransmit
                            msg_id = self.protocol.enqueue_request(request.to_bytes())?;
                        }
                    }
                }
            }
        }
    }

    /// Receives a single message from the server
    async fn receive_message(&mut self) -> Result<Vec<u8>, io::Error> {
        self.transport.recv().await.map_err(|e| match e.kind() {
            io::ErrorKind::UnexpectedEof => io::Error::new(io::ErrorKind::ConnectionReset, e),
            _ => e,
        })
    }
}
*/

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

pub struct MtpSender {
    request_channel: mpsc::Sender<Request>,
}

impl MtpSender {
    /// Invoking a single Remote Procedure Call and `await` its result.
    async fn invoke<R: RemoteCall>(&mut self, request: &R) -> Result<Vec<u8>, InvocationError> {
        let (sender, receiver) = oneshot::channel();
        self.request_channel.send(Request {
            data: request.to_bytes(),
            response_channel: sender,
        });
        // TODO don't unwrap
        Ok(receiver.await.unwrap())
    }
}

struct Request {
    data: Vec<u8>,
    response_channel: oneshot::Sender<Response>,
}

type Response = Vec<u8>;

pub struct MtpHandler<D: Decoder, E: Encoder, R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    sender: Sender<E, W>,
    receiver: Receiver<D, R>,
}

impl<D: Decoder, E: Encoder, R: AsyncRead + Unpin, W: AsyncWrite + Unpin> MtpHandler<D, E, R, W> {
    pub async fn run(self) {
        let Self { sender, receiver } = self;
        future::join(sender.network_loop(), receiver.network_loop()).await;
    }
}

struct Receiver<D: Decoder, R: AsyncRead + Unpin> {
    buffer: Box<[u8]>,
    protocol: Arc<Mutex<Mtp>>,
    decoder: D,
    in_stream: R,
}

impl<D: Decoder, R: AsyncRead + Unpin> Receiver<D, R> {
    async fn receive(&mut self) -> Vec<u8> {
        let mut len = 0;
        loop {
            match self.decoder.read(&self.buffer[..len]) {
                // TODO try to avoid to_vec
                Ok(response) => break response.to_vec(),
                Err(required_len) => {
                    self.in_stream
                        .read_exact(&mut self.buffer[len..required_len])
                        .await;
                    len = required_len;
                }
            };
        }
    }

    async fn receive_plain(&mut self) -> Vec<u8> {
        todo!()
    }

    async fn network_loop(mut self) {
        let mut plain_channel: Option<oneshot::Sender<Vec<u8>>> = None;
        let mut response_channels: BTreeMap<MsgId, oneshot::Sender<Vec<u8>>> = BTreeMap::new();

        loop {
            let response = self.receive().await;

            // Pass the response on to the MTP to handle
            let mut protocol_guard = self.protocol.lock().await;

            // TODO properly deal with plain-or-encrypted state by only handling
            //      plain responses while we have no `auth_key` (and probably
            //      panic if we're used incorrectly).
            if let Some(plain_channel) = plain_channel.take() {
                let plaintext = protocol_guard.deserialize_plain_message(&response);
                plain_channel.send(plaintext.unwrap().to_vec());
                continue;
            }

            // TODO properly handle error case
            protocol_guard
                .process_encrypted_response(&response)
                .unwrap();

            // TODO dispatch this somehow
            while let Some(update) = protocol_guard.poll_update() {
                eprintln!("Received update data: {:?}", update);
            }

            // See if there are responses to prior requests
            while let Some((response_id, response)) = protocol_guard.poll_response() {
                if let Some(channel) = response_channels.remove(&response_id) {
                    // TODO properly handle error case
                    channel.send(response.unwrap());
                } else {
                    eprintln!(
                        "Got encrypted response for unknown message: {:?}",
                        response_id
                    );
                }
            }
        }
    }
}

struct Sender<E: Encoder, W: AsyncWrite + Unpin> {
    buffer: Box<[u8]>,
    request_channel: mpsc::Receiver<Request>,
    protocol: Arc<Mutex<Mtp>>,
    encoder: E,
    out_stream: W,
}

impl<E: Encoder, W: AsyncWrite + Unpin> Sender<E, W> {
    async fn send(&mut self, payload: &[u8]) {
        let size = self.encoder
            .write_into(payload, self.buffer.as_mut())
            .expect("tried to send more than MAXIMUM_DATA in a single frame");

        self.out_stream.write_all(&self.buffer[..size]).await;
    }

    async fn send_plain(&mut self, payload: &[u8]) {
        todo!()
    }

    async fn network_loop(mut self) {
        while let Some(request) = self.request_channel.next().await {
            let payload = {
                let mut protocol_guard = self.protocol.lock().await;
                // TODO properly handle errors
                protocol_guard.enqueue_request(request.data).unwrap();
    
                // TODO we don't want to serialize as soon as we enqueued.
                //      We want to enqueue many and serialize as soon as we can send more.
                protocol_guard.serialize_encrypted_messages().unwrap().unwrap()
            };
    
            self.send(&payload);
        }
    }
}

async fn create_mtp(
    io_stream: impl AsyncRead + AsyncWrite + Clone + Unpin,
    auth_key: Option<AuthKey>,
) -> (MtpSender, MtpHandler<impl Decoder, impl Encoder, impl AsyncRead + Unpin, impl AsyncWrite + Unpin>) {

    let protocol = Arc::new(Mutex::new(Mtp::new()));

    let transport = TransportFull::default();
    let (mut encoder, mut decoder) = transport.split();
    let mut in_stream = io_stream.clone();
    let mut out_stream = io_stream;
    let (request_sender, request_receiver) = mpsc::channel(100);

    let mut sender = Sender {
        buffer: vec![0; MAXIMUM_DATA].into_boxed_slice(),
        request_channel: request_receiver,
        protocol: Arc::clone(&protocol),
        encoder,
        out_stream,
    };

    let mut receiver = Receiver {
        buffer: vec![0; MAXIMUM_DATA].into_boxed_slice(),
        protocol: Arc::clone(&protocol),
        decoder,
        in_stream,
    };

    if let Some(auth_key) = auth_key {
        protocol.lock().await.set_auth_key(auth_key, 0);
    } else {
        // A sender is not usable without an authorization key; generate one
        // TODO don't unwrap
        let (request, data) = auth_key::generation::step1().unwrap();
        sender.send_plain(&request).await;
        let response = receiver.receive_plain().await;

        let (request, data) = auth_key::generation::step2(data, response).unwrap();
        sender.send_plain(&request).await;
        let response = receiver.receive_plain().await;

        let (request, data) = auth_key::generation::step3(data, response).unwrap();
        sender.send_plain(&request).await;
        let response = receiver.receive_plain().await;

        let (auth_key, time_offset) = auth_key::generation::create_key(data, response).unwrap();
        protocol.lock().await.set_auth_key(auth_key, time_offset);
    }

    (
        MtpSender {
            request_channel: request_sender,
        },
        MtpHandler {
            sender,
            receiver
        },
    )
}

pub async fn connect_mtp<A: ToSocketAddrs>(
    addr: A,
) -> io::Result<(
    MtpSender,
    MtpHandler<impl Decoder, impl Encoder, impl AsyncRead + Unpin, impl AsyncWrite + Unpin>,
)> {
    let addr = addr.to_socket_addrs()?.next().unwrap();
    let stream = TcpStream::connect(addr).await?;
    Ok(create_mtp(stream, None).await)
}
