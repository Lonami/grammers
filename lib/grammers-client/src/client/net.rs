// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods directly related to the network on the [`Client`] and [`ClientHandle`].

pub use super::updates::UpdateIter;
use super::{Client, ClientHandle, Config, Request, Step};
use futures::future::FutureExt as _;
use futures::{future, pin_mut};
use grammers_mtproto::{mtp, transport};
use grammers_mtsender::{self as sender, AuthorizationError, InvocationError, Sender};
use grammers_tl_types::{self as tl, Deserializable};
use log::info;
use std::net::Ipv4Addr;
use tokio::sync::{mpsc, oneshot};

/// Socket addresses to Telegram datacenters, where the index into this array
/// represents the data center ID.
///
/// The addresses were obtained from the `static` addresses through a call to
/// `functions::help::GetConfig`.
const DC_ADDRESSES: [(Ipv4Addr, u16); 6] = [
    (Ipv4Addr::new(149, 154, 167, 51), 443), // default (2)
    (Ipv4Addr::new(149, 154, 175, 53), 443),
    (Ipv4Addr::new(149, 154, 167, 51), 443),
    (Ipv4Addr::new(149, 154, 175, 100), 443),
    (Ipv4Addr::new(149, 154, 167, 92), 443),
    (Ipv4Addr::new(91, 108, 56, 190), 443),
];

pub(crate) async fn connect_sender(
    dc_id: i32,
    config: &mut Config,
) -> Result<Sender<transport::Full, mtp::Encrypted>, AuthorizationError> {
    let transport = transport::Full::new();

    let addr = DC_ADDRESSES[dc_id as usize];

    let mut sender = if let Some(auth_key) = config.session.auth_key.as_ref() {
        info!(
            "creating a new sender with existing auth key to dc {} {:?}",
            dc_id, addr
        );
        sender::connect_with_auth(transport, addr, auth_key.clone()).await?
    } else {
        info!(
            "creating a new sender and auth key in dc {} {:?}",
            dc_id, addr
        );
        let sender = sender::connect(transport, addr).await?;

        config.session.auth_key = Some(sender.auth_key().clone());
        config.session.save()?;
        sender
    };

    // TODO handle -404 (we had a previously-valid authkey, but server no longer knows about it)
    // TODO all up-to-date server addresses should be stored in the session for future initial connections
    let _remote_config = sender
        .invoke(&tl::functions::InvokeWithLayer {
            layer: tl::LAYER,
            query: tl::functions::InitConnection {
                api_id: config.api_id,
                device_model: config.params.device_model.clone(),
                system_version: config.params.system_version.clone(),
                app_version: config.params.app_version.clone(),
                system_lang_code: config.params.system_lang_code.clone(),
                lang_pack: "".into(),
                lang_code: config.params.lang_code.clone(),
                proxy: None,
                params: None,
                query: tl::functions::help::GetConfig {},
            },
        })
        .await?;

    // TODO use the dc id from the config as "this dc", not the input dc id
    config.session.user_dc = Some(dc_id);
    config.session.save()?;

    Ok(sender)
}

impl Client {
    /// Creates and returns a new client instance upon successful connection to Telegram.
    ///
    /// If the session in the configuration did not have an authorization key, a new one
    /// will be created and the session will be saved with it.
    ///
    /// The connection will be initialized with the data from the input configuration.
    pub async fn connect(mut config: Config) -> Result<Self, AuthorizationError> {
        let sender = connect_sender(config.session.user_dc.unwrap_or(0), &mut config).await?;

        // TODO Sender doesn't have a way to handle backpressure yet
        let (handle_tx, handle_rx) = mpsc::unbounded_channel();
        Ok(Self {
            sender,
            config,
            handle_tx,
            handle_rx,
        })
    }

    /// Invoke a raw API call without the need to use `handle` or `step`.
    pub async fn invoke<R: tl::RemoteCall>(
        &mut self,
        request: &R,
    ) -> Result<R::Return, InvocationError> {
        self.sender.invoke(request).await
    }

    /// Return a new `ClientHandle` that can be used to invoke remote procedure calls.
    pub fn handle(&self) -> ClientHandle {
        ClientHandle {
            tx: self.handle_tx.clone(),
        }
    }

    /// Perform a single network step or processing of incoming requests via handles.
    ///
    /// If a server message is received, requests enqueued via the `handle`'s may have their
    /// result delivered via a channel, and a (possibly empty) list of updates will be returned.
    pub async fn step(&mut self) -> Result<Step, sender::ReadError> {
        let (network, request) = {
            let network = self.sender.step();
            let request = self.handle_rx.recv();
            pin_mut!(network);
            pin_mut!(request);
            match future::select(network, request).await {
                future::Either::Left((network, request)) => {
                    let request = request.now_or_never();
                    (Some(network), request)
                }
                future::Either::Right((request, network)) => {
                    let network = network.now_or_never();
                    (network, Some(request))
                }
            }
        };

        if let Some(request) = request {
            let request = request.expect("mpsc returned None");
            match request {
                Request::Rpc { request, response } => {
                    drop(response.send(self.sender.enqueue_body(request)));
                }
                Request::Disconnect { response } => {
                    drop(response.send(()));
                    return Ok(Step::Disconnected);
                }
            }
        }

        // TODO request cancellation if this is Err
        // (perhaps a method on the sender to cancel_all)
        Ok(Step::Connected {
            updates: if let Some(updates) = network {
                updates?
            } else {
                Vec::new()
            },
        })
    }

    /// Run the client by repeatedly `step`ping the client until a graceful disconnection occurs,
    /// or a network error occurs. Incoming updates are ignored and simply dropped.
    pub async fn run_until_disconnected(mut self) -> Result<(), sender::ReadError> {
        loop {
            match self.step().await? {
                Step::Connected { .. } => continue,
                Step::Disconnected => break Ok(()),
            }
        }
    }
}

impl ClientHandle {
    /// Invoke a raw API call.
    pub async fn invoke<R: tl::RemoteCall>(
        &mut self,
        request: &R,
    ) -> Result<R::Return, InvocationError> {
        let (response, rx) = oneshot::channel();

        drop(self.tx.send(Request::Rpc {
            request: request.to_bytes(),
            response,
        }));

        // First receive the `oneshot::Receiver` with from the `Client`,
        // then `await` on that to receive the response to the request.
        // TODO remove a few some unwraps (same for the drop above)…
        rx.await
            .unwrap()
            .await
            .unwrap()
            .map(|body| R::Return::from_bytes(&body).unwrap())
    }

    /// Gracefully tell the `Client` to disconnect and stop receiving things from the network.
    pub async fn disconnect(&mut self) {
        let (response, rx) = oneshot::channel();

        // TODO handle errors and not just drop them
        drop(self.tx.send(Request::Disconnect { response }));
        rx.await.unwrap();
    }
}
