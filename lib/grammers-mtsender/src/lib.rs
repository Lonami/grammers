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
use futures::channel::mpsc;
use futures::future;
use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use grammers_mtproto::transports::{Decoder, Encoder, TransportFull};
use std::io;
use std::net::ToSocketAddrs;
use grammers_mtproto::Mtp;
use grammers_crypto::{auth_key, AuthKey};
pub use errors::{AuthorizationError, InvocationError};

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
    protocol: Mtp,
    requests: mpsc::Sender<Request>,
    responses: mpsc::Receiver<Response>,
}

impl MtpSender {
    /// Performs the handshake necessary to generate a new authorization
    /// key that can be used to safely transmit data to and from the server.
    ///
    /// See also: https://core.telegram.org/mtproto/auth_key.
    pub async fn generate_auth_key(&mut self) -> Result<AuthKey, AuthorizationError> {
        let (request, data) = auth_key::generation::step1()?;
        let response = self.invoke_plain_request(&request).await.unwrap();

        let (request, data) = auth_key::generation::step2(data, response)?;
        let response = self.invoke_plain_request(&request).await.unwrap();

        let (request, data) = auth_key::generation::step3(data, response)?;
        let response = self.invoke_plain_request(&request).await.unwrap();

        let (auth_key, time_offset) = auth_key::generation::create_key(data, response)?;
        self.protocol.set_auth_key(auth_key.clone(), time_offset);

        Ok(auth_key)
    }

    /// Invoke a serialized request in plaintext.
    async fn invoke_plain_request(&mut self, request: &[u8]) -> Result<Vec<u8>, InvocationError> {
        // Send
        let payload = self.protocol.serialize_plain_message(request);
        self.requests.send(payload).await.unwrap();

        // Receive
        let response = self.responses.next().await.unwrap();
        self.protocol
            .deserialize_plain_message(&response)
            .map(|x| x.to_vec())
            .map_err(InvocationError::from)
    }
}

type Request = Vec<u8>;
type Response = Vec<u8>;

pub struct MtpHandler<RW: AsyncRead + AsyncWrite + Clone + Unpin> {
    transport: TransportFull,
    io: RW,
    requests: mpsc::Receiver<Request>,
    responses: mpsc::Sender<Response>,
}
impl<RW: AsyncRead + AsyncWrite + Clone + Unpin> MtpHandler<RW> {
    pub async fn run(self) {
        let Self {
            transport,
            io,
            mut requests,
            mut responses,
        } = self;
        let (mut encoder, mut decoder) = transport.split();
        let mut reader = io.clone();
        let mut writer = io;
        future::join(
            async move {
                let mut recv_buffer = vec![0; MAXIMUM_DATA].into_boxed_slice();
                loop {
                    let mut len = 0;
                    loop {
                        match decoder.read(&recv_buffer[..len]) {
                            Ok(data) => {
                                responses.send(data.to_vec()).await;
                                break;
                            }
                            Err(required_len) => {
                                reader.read_exact(&mut recv_buffer[len..required_len]).await;

                                len = required_len;
                            }
                        }
                    }
                }
            },
            async move {
                let mut send_buffer = vec![0; MAXIMUM_DATA].into_boxed_slice();
                while let Some(request) = requests.next().await {
                    let size = encoder
                        .write_into(&request, send_buffer.as_mut())
                        .expect("tried to send more than MAXIMUM_DATA in a single frame");

                    writer.write_all(&send_buffer[..size]).await;
                }
            },
        )
        .await;
    }
}

fn create_mtp<RW: AsyncRead + AsyncWrite + Clone + Unpin>(io: RW) -> (MtpSender, MtpHandler<RW>) {
    let (request_sender, request_receiver) = mpsc::channel(100);
    let (response_sender, response_receiver) = mpsc::channel(100);
    (
        MtpSender {
            protocol: Mtp::new(),
            requests: request_sender,
            responses: response_receiver,
        },
        MtpHandler {
            transport: TransportFull::default(),
            io,
            requests: request_receiver,
            responses: response_sender,
        },
    )
}

pub async fn connect_mtp<A: ToSocketAddrs>(
    addr: A,
) -> io::Result<(
    MtpSender,
    MtpHandler<impl AsyncRead + AsyncWrite + Clone + Unpin>,
)> {
    let addr = addr.to_socket_addrs()?.next().unwrap();
    let stream = TcpStream::connect(addr).await?;
    Ok(create_mtp(stream))
}
