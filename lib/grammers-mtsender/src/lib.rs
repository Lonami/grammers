// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod errors;
mod tcp_transport;

pub use errors::{AuthorizationError, InvocationError};
use tcp_transport::TcpTransport;

use grammers_crypto::{auth_key, AuthKey};
use grammers_mtproto::errors::RequestError;
use grammers_mtproto::transports::TransportFull;
use grammers_mtproto::MTProto;
pub use grammers_mtproto::DEFAULT_COMPRESSION_THRESHOLD;
use grammers_tl_types::{Deserializable, RPC};

use std::io;
use std::net::ToSocketAddrs;

/// A builder to configure `MTSender` instances.
pub struct MTSenderBuilder {
    compression_threshold: Option<usize>,
    auth_key: Option<AuthKey>,
}

/// A Mobile Transport sender, using the [Mobile Transport Protocol]
/// underneath.
///
/// [Mobile Transport Protocol]: https://core.telegram.org/mtproto
pub struct MTSender {
    protocol: MTProto,
    // TODO let the user change the type of transport used
    transport: TcpTransport<TransportFull>,
}

impl MTSenderBuilder {
    fn new() -> Self {
        Self {
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
    pub async fn connect<A: ToSocketAddrs>(self, addr: A) -> io::Result<MTSender> {
        MTSender::with_builder(self, addr).await
    }
}

impl MTSender {
    /// Returns a builder to configure certain parameters.
    pub fn build() -> MTSenderBuilder {
        MTSenderBuilder::new()
    }

    /// Creates and connects a new instance with default settings.
    pub async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        Self::build().connect(addr).await
    }

    /// Constructs an instance using a finished builder.
    async fn with_builder<A: ToSocketAddrs>(builder: MTSenderBuilder, addr: A) -> io::Result<Self> {
        let addr = addr.to_socket_addrs()?.next().unwrap();
        let transport = TcpTransport::connect(addr).await?;

        let mut protocol = MTProto::build().compression_threshold(builder.compression_threshold);

        if let Some(auth_key) = builder.auth_key {
            protocol = protocol.auth_key(auth_key);
        }

        let protocol = protocol.finish();

        Ok(Self {
            protocol,
            transport,
        })
    }

    /// Performs the handshake necessary to generate a new authorization
    /// key that can be used to safely transmit data to and from the server.
    ///
    /// See also: https://core.telegram.org/mtproto/auth_key.
    pub async fn generate_auth_key(&mut self) -> Result<AuthKey, AuthorizationError> {
        let (request, data) = auth_key::generation::step1()?;
        let response = self.invoke_plain_request(&request).await?;

        let (request, data) = auth_key::generation::step2(data, response)?;
        let response = self.invoke_plain_request(&request).await?;

        let (request, data) = auth_key::generation::step3(data, response)?;
        let response = self.invoke_plain_request(&request).await?;

        let (auth_key, time_offset) = auth_key::generation::create_key(data, response)?;
        self.protocol.set_auth_key(auth_key.clone(), time_offset);

        Ok(auth_key)
    }

    /// Changes the authorization key data for a different one.
    pub fn set_auth_key(&mut self, data: [u8; 256]) {
        self.protocol.set_auth_key(AuthKey::from_bytes(data), 0);
    }

    /// Invoke a serialized request in plaintext.
    async fn invoke_plain_request(&mut self, request: &[u8]) -> Result<Vec<u8>, InvocationError> {
        // Send
        let payload = self.protocol.serialize_plain_message(request);
        self.transport.send(&payload).await?;

        // Receive
        let response = self.receive_message().await?;
        self.protocol
            .deserialize_plain_message(&response)
            .map(|x| x.to_vec())
            .map_err(InvocationError::from)
    }

    /// Block invoking a single Remote Procedure Call and return its result.
    ///
    /// The invocation might fail due to network problems, in which case the
    /// outermost result represents failure.
    ///
    /// If the request is both sent and received successfully, then the
    /// request itself was understood by the server, but it could not be
    /// executed. This is represented by the innermost result.
    pub async fn invoke<R: RPC>(&mut self, request: &R) -> Result<R::Return, InvocationError> {
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
