use grammers_crypto::AuthKey;
use grammers_mtproto::transports::{Transport, TransportFull};
use grammers_mtproto::{auth_key, MTProto};
use grammers_tl_types::{self as tl, Deserializable, RPC};
/// A Mobile Transport sender, using the [Mobile Transport Protocol]
/// underneath.
///
/// [Mobile Transport Protocol]: https://core.telegram.org/mtproto
use std::io::{self, Result};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

pub struct MTSender {
    protocol: MTProto,
    stream: TcpStream,
    // TODO let the user change the type of transport used
    transport: TransportFull,
}

impl MTSender {
    pub fn connect<A: ToSocketAddrs>(addr: A, protocol: MTProto) -> Result<Self> {
        let stream = TcpStream::connect(addr)?;
        stream.set_read_timeout(Some(Duration::from_secs(2)));
        Ok(Self {
            protocol,
            stream,
            transport: TransportFull::new(),
        })
    }

    /// Performs the handshake necessary to generate a new authorization
    /// key that can be used to safely transmit data to and from the server.
    ///
    /// See also: https://core.telegram.org/mtproto/auth_key.
    pub fn generate_auth_key(&mut self) -> Result<AuthKey> {
        // Step 1: Request PQ.
        let nonce = auth_key::generation::generate_nonce();
        let res_pq = match self.invoke_plain_request(tl::functions::ReqPqMulti {
            nonce: nonce.clone(),
        })? {
            tl::enums::ResPQ::ResPQ(x) => x,
        };

        let pq = auth_key::generation::validate_pq(&nonce, &res_pq)?;

        // Step 2: DH Exchange.
        let req_dh = auth_key::generation::construct_req_dh_params(pq, &nonce, &res_pq)?;
        let server_dh_params = self.invoke_plain_request(req_dh)?;
        auth_key::generation::validate_server_dh_params(server_dh_params);

        unimplemented!("finish generate auth key");
    }

    fn invoke_plain_request<R: RPC>(&mut self, request: R) -> Result<R::Return> {
        let payload = self.protocol.serialize_plain_message(request.to_bytes());
        self.transport.send(&mut self.stream, &payload)?;
        let response = self
            .transport
            .receive(&mut self.stream)
            .map_err(|e| match e.kind() {
                io::ErrorKind::UnexpectedEof => io::Error::new(io::ErrorKind::ConnectionReset, e),
                _ => e,
            })?;
        let body = self.protocol.deserialize_plain_message(&response)?;
        R::Return::from_bytes(body)
    }
}
