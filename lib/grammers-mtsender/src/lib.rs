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
use grammers_crypto::{auth_key, AuthKey};
use grammers_mtproto::errors::{RequestError, TransportError};
use grammers_mtproto::transports::{Decoder, Encoder, Transport};
use grammers_mtproto::{MsgId, Mtp};
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

struct Receiver<D: Decoder, R: AsyncRead + Unpin> {
    buffer: Box<[u8]>,
    protocol: Arc<Mutex<Mtp>>,
    decoder: D,
    in_stream: R,
    response_map: Arc<Mutex<BTreeMap<MsgId, oneshot::Sender<Response>>>>,
}

impl<D: Decoder, R: AsyncRead + Unpin> Receiver<D, R> {
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

    // TODO having such a generic Err (InvocationError) is not ideal
    async fn receive_plain(&mut self) -> Result<Vec<u8>, InvocationError> {
        let response = self.receive().await?;
        Ok(self
            .protocol
            .lock()
            .await
            .deserialize_plain_message(&response)
            .map_err(InvocationError::from)?
            .to_vec())
    }

    async fn network_loop(mut self) {
        loop {
            let response = match self.receive().await {
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

struct Sender<E: Encoder, W: AsyncWrite + Unpin> {
    buffer: Box<[u8]>,
    request_channel: mpsc::Receiver<Request>,
    protocol: Arc<Mutex<Mtp>>,
    encoder: E,
    out_stream: W,
    response_map: Arc<Mutex<BTreeMap<MsgId, oneshot::Sender<Response>>>>,
}

impl<E: Encoder, W: AsyncWrite + Unpin> Sender<E, W> {
    async fn send(&mut self, payload: &[u8]) -> io::Result<()> {
        let size = self
            .encoder
            .write_into(payload, self.buffer.as_mut())
            .expect("tried to send more than MAXIMUM_DATA in a single frame");

        self.out_stream.write_all(&self.buffer[..size]).await
    }

    async fn send_plain(&mut self, payload: &[u8]) -> io::Result<()> {
        let payload = self.protocol.lock().await.serialize_plain_message(payload);
        self.send(&payload).await
    }

    async fn network_loop(mut self) {
        while let Some(request) = self.request_channel.next().await {
            let payload = {
                let mut protocol_guard = self.protocol.lock().await;

                let msg_id = match protocol_guard.enqueue_request(request.data) {
                    Ok(msg_id) => msg_id,
                    Err(_) => {
                        // We were given a request that failed to be serialized, so
                        // notify the error immediately and don't bother sending it.

                        // Drop the result; if the user closed the channel it simply means
                        // they no longer need this result so it's okay to fail to send it.
                        // TODO proper error (with more info)
                        drop(request.response_channel.send(Err(RequestError::Dropped)));
                        continue;
                    }
                };

                // TODO we don't want to serialize as soon as we enqueued.
                //      We want to enqueue many and serialize as soon as we can send more.
                //      Maybe yet another Enqueuer struct?
                //
                // TODO why can serialize_encrypted_messages fail if we checked that in enqueue_request?
                //      furthermore, we always have an authkey by this point so that error is undesirable.
                let payload = match protocol_guard.serialize_encrypted_messages() {
                    Ok(option) => {
                        // Safe, we know it's Some (we just inserted something)
                        option.unwrap()
                    }
                    Err(_) => {
                        // Same as above.
                        // TODO proper error (with more info)
                        drop(request.response_channel.send(Err(RequestError::Dropped)));
                        continue;
                    }
                };

                // TODO maybe we don't want to do this until we've sent it?
                let mut map_guard = self.response_map.lock().await;
                map_guard.insert(msg_id, request.response_channel);

                payload
            };

            // If sending over IO fails we won't be able to send anything else.
            // Break out of the loop.
            if let Err(err) = self.send(&payload).await {
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
    let protocol = Arc::new(Mutex::new(Mtp::new()));
    let (encoder, decoder) = T::instance();
    let (request_sender, request_receiver) = mpsc::channel(100);
    let response_map = Arc::new(Mutex::new(BTreeMap::new()));

    let mut sender = Sender {
        buffer: vec![0; MAXIMUM_DATA].into_boxed_slice(),
        request_channel: request_receiver,
        protocol: Arc::clone(&protocol),
        encoder,
        out_stream,
        response_map: Arc::clone(&response_map),
    };

    let mut receiver = Receiver {
        buffer: vec![0; MAXIMUM_DATA].into_boxed_slice(),
        protocol: Arc::clone(&protocol),
        decoder,
        in_stream,
        response_map,
    };

    if let Some(auth_key) = auth_key {
        eprintln!("Using input auth_key");
        protocol.lock().await.set_auth_key(auth_key, 0);
    } else {
        eprintln!("No input auth_key; generating new one");
        // A sender is not usable without an authorization key; generate one
        let (request, data) = auth_key::generation::step1()?;
        sender.send_plain(&request).await?;
        let response = receiver.receive_plain().await?;

        let (request, data) = auth_key::generation::step2(data, response)?;
        sender.send_plain(&request).await?;
        let response = receiver.receive_plain().await?;

        let (request, data) = auth_key::generation::step3(data, response)?;
        sender.send_plain(&request).await?;
        let response = receiver.receive_plain().await?;

        let (auth_key, time_offset) = auth_key::generation::create_key(data, response)?;
        protocol.lock().await.set_auth_key(auth_key, time_offset);
        eprintln!("New auth_key generation success");
    }

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
