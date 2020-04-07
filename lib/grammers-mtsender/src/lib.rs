// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod errors;

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
use grammers_tl_types::RemoteCall;
use std::collections::BTreeMap;
use std::io;
use std::net::ToSocketAddrs;
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

type Response = Vec<u8>;

pub struct MtpSender {
    request_channel: mpsc::Sender<Request>,
}

impl MtpSender {
    /// Invoking a single Remote Procedure Call and `await` its result.
    pub async fn invoke<R: RemoteCall>(&mut self, request: &R) -> Result<Vec<u8>, InvocationError> {
        let (sender, receiver) = oneshot::channel();
        // TODO don't unwrap
        self.request_channel
            .send(Request {
                data: request.to_bytes(),
                response_channel: sender,
            })
            .await
            .unwrap();
        // TODO don't unwrap
        Ok(receiver.await.unwrap())
    }
}

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
                // TODO don't unwrap
                Ok(response) => break response.to_vec(),
                Err(required_len) => {
                    self.in_stream
                        .read_exact(&mut self.buffer[len..required_len])
                        .await
                        .unwrap();
                    len = required_len;
                }
            };
        }
    }

    async fn receive_plain(&mut self) -> Vec<u8> {
        let response = self.receive().await;
        // TODO don't unwrap
        self.protocol
            .lock()
            .await
            .deserialize_plain_message(&response)
            .unwrap()
            .to_vec()
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
                // TODO don't unwrap
                plain_channel.send(plaintext.unwrap().to_vec()).unwrap();
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
                    channel.send(response.unwrap()).unwrap();
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
        let size = self
            .encoder
            .write_into(payload, self.buffer.as_mut())
            .expect("tried to send more than MAXIMUM_DATA in a single frame");

        // TODO don't unwrap
        self.out_stream
            .write_all(&self.buffer[..size])
            .await
            .unwrap();
    }

    async fn send_plain(&mut self, payload: &[u8]) {
        let payload = self.protocol.lock().await.serialize_plain_message(payload);
        self.send(&payload).await
    }

    async fn network_loop(mut self) {
        while let Some(request) = self.request_channel.next().await {
            let payload = {
                let mut protocol_guard = self.protocol.lock().await;
                // TODO properly handle errors
                protocol_guard.enqueue_request(request.data).unwrap();

                // TODO we don't want to serialize as soon as we enqueued.
                //      We want to enqueue many and serialize as soon as we can send more.
                protocol_guard
                    .serialize_encrypted_messages()
                    .unwrap()
                    .unwrap()
            };

            self.send(&payload).await;
            // TODO don't unwrap
            request.response_channel.send(vec![]).unwrap();
        }
    }
}

async fn create_mtp(
    io_stream: impl AsyncRead + AsyncWrite + Clone + Unpin,
    auth_key: Option<AuthKey>,
) -> (
    MtpSender,
    MtpHandler<impl Decoder, impl Encoder, impl AsyncRead + Unpin, impl AsyncWrite + Unpin>,
) {
    let protocol = Arc::new(Mutex::new(Mtp::new()));

    let transport = TransportFull::default();
    let (encoder, decoder) = transport.split();
    let in_stream = io_stream.clone();
    let out_stream = io_stream;
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
        eprintln!("Using input auth_key");
        protocol.lock().await.set_auth_key(auth_key, 0);
    } else {
        eprintln!("No input auth_key; generating new one");
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
        eprintln!("New auth_key generation success");
    }

    (
        MtpSender {
            request_channel: request_sender,
        },
        MtpHandler { sender, receiver },
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
