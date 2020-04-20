// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod errors;

pub use errors::{AuthorizationError, InvocationError};
use futures::channel::{mpsc, oneshot};
use futures::future;
use futures::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use futures::lock::Mutex;
use futures::sink::SinkExt;
use futures::stream::StreamExt;
use grammers_crypto::AuthKey;
use grammers_mtproto::errors::{RequestError, TransportError};
use grammers_mtproto::transports::{Decoder, Encoder, Transport};
use grammers_mtproto::{authentication, MsgId, Mtp, PlainMtp};
use grammers_tl_types::{Deserializable, RemoteCall};
use log::{error, warn};
use std::collections::BTreeMap;
use std::io;
use std::sync::Arc;

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

struct Request {
    data: Vec<u8>,
    response_channel: oneshot::Sender<Response>,
}

type Response = Result<Vec<u8>, RequestError>;

pub struct MtpSender {
    request_channel: mpsc::Sender<Request>,
}

impl MtpSender {
    /// Invoking a single Remote Procedure Call and `await` its result.
    pub async fn invoke<R: RemoteCall>(
        &mut self,
        request: &R,
    ) -> Result<R::Return, InvocationError> {
        loop {
            let (sender, receiver) = oneshot::channel();
            self.request_channel
                .send(Request {
                    data: request.to_bytes(),
                    response_channel: sender,
                })
                .await
                .map_err(|e| {
                    assert!(e.is_disconnected());
                    InvocationError::NotConnected
                })?;

            match receiver
                .await
                .map_err(|_canceled| InvocationError::Dropped)?
            {
                Ok(x) => break Ok(R::Return::from_bytes(&x)?),
                Err(RequestError::RPCError(error)) => {
                    break Err(InvocationError::RPC(error));
                }
                Err(RequestError::Dropped) => {
                    break Err(InvocationError::Dropped);
                }
                Err(RequestError::BadMessage { .. }) => {
                    // Need to retransmit (another loop iteration)
                    continue;
                }
            }
        }
    }
}

pub struct MtpHandler<T: Transport, R: AsyncRead + Unpin, W: AsyncWrite + Unpin> {
    sender: Sender<T::Encoder, W>,
    receiver: Receiver<T::Decoder, R>,
}

impl<T: Transport, R: AsyncRead + Unpin, W: AsyncWrite + Unpin> MtpHandler<T, R, W> {
    pub async fn run(self) {
        let Self { sender, receiver } = self;
        future::join(sender.network_loop(), receiver.network_loop()).await;
    }
}

/// Small adapter between network and transport containers.
// TODO "plain" is probably not the best prefix for this, "plain basic"
struct PlainReceiver<D: Decoder, R: AsyncRead + Unpin> {
    buffer: Box<[u8]>,
    decoder: D,
    in_stream: R,
}

struct Receiver<D: Decoder, R: AsyncRead + Unpin> {
    plain: PlainReceiver<D, R>,
    protocol: Arc<Mutex<Mtp>>,
    response_map: Arc<Mutex<BTreeMap<MsgId, oneshot::Sender<Response>>>>,
}

impl<D: Decoder, R: AsyncRead + Unpin> PlainReceiver<D, R> {
    async fn receive(&mut self) -> io::Result<Vec<u8>> {
        let mut len = 0;
        loop {
            match self.decoder.read(&self.buffer[..len]) {
                // TODO try to avoid to_vec
                Ok(response) => break Ok(response.to_vec()),
                Err(TransportError::MissingBytes(required_len)) => {
                    self.in_stream
                        .read_exact(&mut self.buffer[len..required_len])
                        .await?;
                    len = required_len;
                }
                Err(TransportError::UnexpectedData(what)) => {
                    break Err(io::Error::new(io::ErrorKind::InvalidData, what))
                }
            };
        }
    }
}

impl<D: Decoder, R: AsyncRead + Unpin> Receiver<D, R> {
    async fn network_loop(mut self) {
        loop {
            let response = match self.plain.receive().await {
                Ok(response) => response,
                Err(err) => {
                    warn!("receiving response failed: {:?}", err);
                    break;
                }
            };

            // Pass the response on to the MTP to handle
            let mut protocol_guard = self.protocol.lock().await;

            if let Err(err) = protocol_guard.process_encrypted_response(&response) {
                // TODO some errors here are probably OK; figure out which are resumable
                error!("processing response failed: {:?}", err);
                break;
            }

            // TODO dispatch this somehow
            while let Some(update) = protocol_guard.poll_update() {
                eprintln!("Received update data: {:?}", update);
            }

            // See if there are responses to prior requests
            let mut map_guard = self.response_map.lock().await;
            while let Some((response_id, response)) = protocol_guard.poll_response() {
                if let Some(channel) = map_guard.remove(&response_id) {
                    // Drop the result; if the user closed the channel it simply means
                    // they no longer need this result so it's okay to fail to send it.
                    drop(channel.send(response));
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

/// Small adapter between network and transport containers.
struct PlainSender<E: Encoder, W: AsyncWrite + Unpin> {
    buffer: Box<[u8]>,
    encoder: E,
    out_stream: W,
}

struct Sender<E: Encoder, W: AsyncWrite + Unpin> {
    plain: PlainSender<E, W>,
    protocol: Arc<Mutex<Mtp>>,
    request_channel: mpsc::Receiver<Request>,
    response_map: Arc<Mutex<BTreeMap<MsgId, oneshot::Sender<Response>>>>,
}

impl<E: Encoder, W: AsyncWrite + Unpin> PlainSender<E, W> {
    async fn send(&mut self, payload: &[u8]) -> io::Result<()> {
        let size = self
            .encoder
            .write_into(payload, self.buffer.as_mut())
            .expect("tried to send more than MAXIMUM_DATA in a single frame");

        self.out_stream.write_all(&self.buffer[..size]).await
    }
}

impl<E: Encoder, W: AsyncWrite + Unpin> Sender<E, W> {
    async fn network_loop(mut self) {
        while let Some(request) = self.request_channel.next().await {
            let payload = {
                let mut protocol_guard = self.protocol.lock().await;

                let msg_id = protocol_guard.enqueue_request(request.data);

                // TODO we don't want to serialize as soon as we enqueued.
                //      We want to enqueue many and serialize as soon as we can send more.
                //      Maybe yet another Enqueuer struct?
                //
                // Safe, we know it's Some (we just inserted something)
                let payload = protocol_guard.serialize_encrypted_messages().unwrap();

                // TODO maybe we don't want to do this until we've sent it?
                let mut map_guard = self.response_map.lock().await;
                map_guard.insert(msg_id, request.response_channel);

                payload
            };

            // If sending over IO fails we won't be able to send anything else.
            // Break out of the loop.
            if let Err(err) = self.plain.send(&payload).await {
                warn!("sending payload failed: {:?}", err);
                break;
            }
        }
    }
}

// TODO this needs a better name ("create mtp" is just "Mtp::new()")
pub async fn create_mtp<T: Transport, R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    (in_stream, out_stream): (R, W),
    auth_key: Option<AuthKey>,
) -> Result<(MtpSender, MtpHandler<T, R, W>), AuthorizationError> {
    let (encoder, decoder) = T::instance();

    let mut sender = PlainSender {
        buffer: vec![0; MAXIMUM_DATA].into_boxed_slice(),
        encoder,
        out_stream,
    };

    let mut receiver = PlainReceiver {
        buffer: vec![0; MAXIMUM_DATA].into_boxed_slice(),
        decoder,
        in_stream,
    };

    let mut mtp = PlainMtp::new();

    let auth_key = if let Some(auth_key) = auth_key {
        eprintln!("Using input auth_key");
        auth_key
    } else {
        eprintln!("No input auth_key; generating new one");
        // A sender is not usable without an authorization key; generate one
        // TODO avoid to_vec()'s
        let (request, data) = authentication::step1()?;
        sender.send(&mtp.serialize_plain_message(&request)).await?;
        let response = mtp
            .deserialize_plain_message(&receiver.receive().await?)?
            .to_vec();

        let (request, data) = authentication::step2(data, response)?;
        sender.send(&mtp.serialize_plain_message(&request)).await?;
        let response = mtp
            .deserialize_plain_message(&receiver.receive().await?)?
            .to_vec();

        let (request, data) = authentication::step3(data, response)?;
        sender.send(&mtp.serialize_plain_message(&request)).await?;
        let response = mtp
            .deserialize_plain_message(&receiver.receive().await?)?
            .to_vec();

        // TODO use time_offset
        let (auth_key, _time_offset) = authentication::create_key(data, response)?;
        eprintln!("New auth_key generation success");
        auth_key
    };

    let protocol = Arc::new(Mutex::new(Mtp::new(auth_key)));
    let (request_sender, request_receiver) = mpsc::channel(100);
    let response_map = Arc::new(Mutex::new(BTreeMap::new()));

    let sender = Sender {
        plain: sender,
        request_channel: request_receiver,
        protocol: Arc::clone(&protocol),
        response_map: Arc::clone(&response_map),
    };

    let receiver = Receiver {
        plain: receiver,
        protocol: Arc::clone(&protocol),
        response_map,
    };

    Ok((
        MtpSender {
            request_channel: request_sender,
        },
        MtpHandler { sender, receiver },
    ))
}

#[cfg(feature = "async-std")]
pub async fn connect_mtp<A: std::net::ToSocketAddrs>(
    addr: A,
) -> Result<
    (
        MtpSender,
        MtpHandler<impl Transport, impl AsyncRead + Unpin, impl AsyncWrite + Unpin>,
    ),
    AuthorizationError,
> {
    let stream = async_std::net::TcpStream::connect(addr).await?;
    create_mtp(stream, None).await
}
