use grammers_mtproto::transports::{Transport, TransportFull};
use grammers_mtproto::{auth_key, MTProto};
use grammers_tl_types::{Deserializable, RPC};

use std::io::{self, Result};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

/// A Mobile Transport sender, using the [Mobile Transport Protocol]
/// underneath.
///
/// [Mobile Transport Protocol]: https://core.telegram.org/mtproto
pub struct MTSender {
    protocol: MTProto,
    stream: TcpStream,
    // TODO let the user change the type of transport used
    transport: TransportFull,
}

impl MTSender {
    pub fn connect<A: ToSocketAddrs>(addr: A) -> Result<Self> {
        let stream = TcpStream::connect(addr)?;
        // TODO let the user configure this (and MTProto too)
        stream.set_read_timeout(Some(Duration::from_secs(2)))?;
        Ok(Self {
            protocol: MTProto::new(),
            stream,
            transport: TransportFull::new(),
        })
    }

    /// Performs the handshake necessary to generate a new authorization
    /// key that can be used to safely transmit data to and from the server.
    ///
    /// See also: https://core.telegram.org/mtproto/auth_key.
    pub fn generate_auth_key(&mut self) -> Result<()> {
        let (request, data) = auth_key::generation::step1()?;
        let response = self.invoke_plain_request(&request)?;

        let (request, data) = auth_key::generation::step2(data, response)?;
        let response = self.invoke_plain_request(&request)?;

        let (request, data) = auth_key::generation::step3(data, response)?;
        let response = self.invoke_plain_request(&request)?;

        let (auth_key, time_offset) = auth_key::generation::create_key(data, response)?;
        self.protocol.set_auth_key(auth_key, time_offset);

        Ok(())
    }

    /// Invoke a serialized request in plaintext.
    fn invoke_plain_request(&mut self, request: &[u8]) -> Result<Vec<u8>> {
        // Send
        let payload = self.protocol.serialize_plain_message(request);
        self.transport.send(&mut self.stream, &payload)?;

        // Receive
        let response = self.receive_message()?;
        self.protocol
            .deserialize_plain_message(&response)
            .map(|x| x.to_vec())
    }

    /// Block invoking a single Remote Procedure Call and return its result.
    pub fn invoke<R: RPC>(&mut self, request: &R) -> Result<R::Return> {
        let msg_id = self.protocol.enqueue_request(request.to_bytes())?;
        let data = loop {
            // The protocol may generate more outgoing requests, so we need
            // to constantly check for those until we receive a response.
            while let Some(payload) = self.protocol.pop_queue() {
                let encrypted = self.protocol.encrypt_message_data(payload);
                self.transport.send(&mut self.stream, &encrypted)?;
            }

            // Receive a matching `MsgId`
            let response = self.receive_message()?;
            if let Some((response_id, data)) = self.protocol.process_response(&response)? {
                if response_id == msg_id {
                    break data;
                }
            }
        };

        R::Return::from_bytes(&data)
    }

    /// Receives a single message from the server
    fn receive_message(&mut self) -> Result<Vec<u8>> {
        self.transport
            .receive(&mut self.stream)
            .map_err(|e| match e.kind() {
                io::ErrorKind::UnexpectedEof => io::Error::new(io::ErrorKind::ConnectionReset, e),
                _ => e,
            })
    }
}
