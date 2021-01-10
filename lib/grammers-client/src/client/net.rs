// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
pub use super::updates::UpdateIter;
use super::{Client, ClientHandle, Config, Request, Step};
use crate::types::{ChatHashCache, MessageBox};
use grammers_mtproto::{mtp, transport};
use grammers_mtsender::{self as sender, AuthorizationError, InvocationError, Sender};
use grammers_session::Session;
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
    (Ipv4Addr::new(0, 0, 0, 0), 0),
    (Ipv4Addr::new(149, 154, 175, 53), 443),
    (Ipv4Addr::new(149, 154, 167, 51), 443),
    (Ipv4Addr::new(149, 154, 175, 100), 443),
    (Ipv4Addr::new(149, 154, 167, 92), 443),
    (Ipv4Addr::new(91, 108, 56, 190), 443),
];

const DEFAULT_DC: i32 = 2;

pub(crate) async fn connect_sender<S: Session>(
    dc_id: i32,
    config: &mut Config<S>,
) -> Result<Sender<transport::Full, mtp::Encrypted>, AuthorizationError> {
    let transport = transport::Full::new();

    let addr = DC_ADDRESSES[dc_id as usize];

    let mut sender = if let Some(auth_key) = config.session.dc_auth_key(dc_id) {
        info!(
            "creating a new sender with existing auth key to dc {} {:?}",
            dc_id, addr
        );
        sender::connect_with_auth(transport, addr, auth_key).await?
    } else {
        info!(
            "creating a new sender and auth key in dc {} {:?}",
            dc_id, addr
        );
        let sender = sender::connect(transport, addr).await?;

        config.session.insert_dc(dc_id, addr, &sender.auth_key());
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

    Ok(sender)
}

/// Method implementations directly related with network connectivity.
impl<S: Session> Client<S> {
    /// Creates and returns a new client instance upon successful connection to Telegram.
    ///
    /// If the session in the configuration did not have an authorization key, a new one
    /// will be created and the session will be saved with it.
    ///
    /// The connection will be initialized with the data from the input configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_client::{Client, Config};
    /// use grammers_session::FileSession;
    ///
    /// // Note: these are example values and are not actually valid.
    /// //       Obtain your own with the developer's phone at https://my.telegram.org.
    /// const API_ID: i32 = 932939;
    /// const API_HASH: &str = "514727c32270b9eb8cc16daf17e21e57";
    ///
    /// # async fn f() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect(Config {
    ///     session: FileSession::load_or_create("hello-world.session")?,
    ///     api_id: API_ID,
    ///     api_hash: API_HASH.to_string(),
    ///     params: Default::default(),
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(mut config: Config<S>) -> Result<Self, AuthorizationError> {
        let dc_id = config.session.user_dc().unwrap_or(DEFAULT_DC);
        let sender = connect_sender(dc_id, &mut config).await?;
        let message_box = if config.params.catch_up {
            if let Some(state) = config.session.get_state() {
                MessageBox::load(state)
            } else {
                MessageBox::new()
            }
        } else {
            // If the user doesn't want to bother with catching up on previous update, start with
            // pristine state instead.
            MessageBox::new()
        };

        // TODO Sender doesn't have a way to handle backpressure yet
        let (handle_tx, handle_rx) = mpsc::unbounded_channel();
        let mut client = Self {
            sender,
            dc_id,
            config,
            handle_tx,
            handle_rx,
            message_box,
            chat_hashes: ChatHashCache::new(),
        };

        // Don't bother getting pristine state if we're not logged in.
        if client.message_box.is_empty() && client.config.session.signed_in() {
            match client.invoke(&tl::functions::updates::GetState {}).await {
                Ok(state) => {
                    client.message_box.set_state(state);
                    client.sync_update_state();
                }
                Err(_) => {
                    // The account may no longer actually be logged in, or it can rarely fail.
                    // `message_box` will try to correct its state as updates arrive.
                }
            }
        }

        Ok(client)
    }

    /// Invoke a raw API call without the need to use a [`Client::handle`] or having to repeatedly
    /// call [`Client::step`]. This directly sends the request to Telegram's servers.
    ///
    /// Using function definitions corresponding to a different layer is likely to cause the
    /// responses to the request to not be understood.
    ///
    /// <div class="stab unstable">
    ///
    /// **Warning**: this method is **not** part of the stability guarantees of semantic
    /// versioning. It **may** break during *minor* version changes (but not on patch version
    /// changes). Use with care.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::Client<grammers_session::MemorySession>) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_tl_types as tl;
    ///
    /// dbg!(client.invoke(&tl::functions::Ping { ping_id: 0 }).await?);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn invoke<R: tl::RemoteCall>(
        &mut self,
        request: &R,
    ) -> Result<R::Return, InvocationError> {
        self.sender.invoke(request).await
    }

    /// Return a new [`ClientHandle`] that can be used to invoke remote procedure calls.
    ///
    /// # Examples
    ///
    /// ```
    /// use tokio::task;
    ///
    /// # async fn f(mut client: grammers_client::Client<grammers_session::MemorySession>) -> Result<(), Box<dyn std::error::Error>> {
    /// // Obtain a handle. After this you can obtain more by using `client_handle.clone()`.
    /// let mut client_handle = client.handle();
    ///
    /// // Run the network loop. This is necessary, or no network events will be processed!
    /// let network_handle = task::spawn(async move { client.run_until_disconnected().await });
    ///
    /// // Use the `client_handle` to your heart's content, maybe you just want to disconnect:
    /// client_handle.disconnect().await;
    ///
    /// // Joining on the spawned task lets us access the result from `run_until_disconnected`,
    /// // so we can verify everything went fine. You could also just drop this though.
    /// network_handle.await?;
    /// # Ok(())
    /// # }
    ///
    pub fn handle(&self) -> ClientHandle {
        ClientHandle {
            tx: self.handle_tx.clone(),
        }
    }

    /// Perform a single network step or processing of incoming requests via handles.
    ///
    /// If a server message is received, requests enqueued via the [`ClientHandle`]s may have
    /// their result delivered via a channel, and a (possibly empty) list of updates will be
    /// returned.
    ///
    /// The other return values are graceful disconnection, or a read error.
    ///
    /// Most commonly, you will want to use the higher-level abstraction [`Client::next_updates`]
    /// instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::Client<grammers_session::MemorySession>) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::NetworkStep;
    ///
    /// loop {
    ///     // Process network events forever until we gracefully disconnect or get an error.
    ///     match client.step().await? {
    ///         NetworkStep::Connected { .. } => continue,
    ///         NetworkStep::Disconnected => break,
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn step(&mut self) -> Result<Step, sender::ReadError> {
        let (network, request) = {
            tokio::select! {
                network = self.sender.step() => (Some(network), None),
                request = self.handle_rx.recv() => (None, Some(request)),
            }
        };

        if let Some(request) = request {
            let request = request.expect("mpsc returned None");
            match request {
                Request::Rpc { request, response } => {
                    // Channel will return `Err` if the `ClientHandle` lost interest, just drop the error.
                    drop(response.send(self.sender.enqueue_body(request)));
                }
                Request::Disconnect { response } => {
                    // Channel will return `Err` if the `ClientHandle` lost interest, just drop the error.
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

    /// Run the client by repeatedly calling [`Client::step`] until a graceful disconnection
    /// occurs, or a network error occurs. Incoming updates are ignored and simply dropped.
    /// instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::Client<grammers_session::MemorySession>) -> Result<(), Box<dyn std::error::Error>> {
    /// client.run_until_disconnected().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run_until_disconnected(mut self) -> Result<(), sender::ReadError> {
        loop {
            match self.step().await? {
                Step::Connected { .. } => continue,
                Step::Disconnected => break Ok(()),
            }
        }
    }
}

/// Method implementations directly related with network connectivity.
impl ClientHandle {
    /// Invoke a raw API call.
    ///
    /// Using function definitions corresponding to a different layer is likely to cause the
    /// responses to the request to not be understood.
    ///
    /// <div class="stab unstable">
    ///
    /// **Warning**: this method is **not** part of the stability guarantees of semantic
    /// versioning. It **may** break during *minor* version changes (but not on patch version
    /// changes). Use with care.
    ///
    /// </div>
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::ClientHandle) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_tl_types as tl;
    ///
    /// dbg!(client.invoke(&tl::functions::Ping { ping_id: 0 }).await?);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn invoke<R: tl::RemoteCall>(
        &mut self,
        request: &R,
    ) -> Result<R::Return, InvocationError> {
        let (response, rx) = oneshot::channel();

        // TODO add a test this (using handle with client dropped)
        if let Err(_) = self.tx.send(Request::Rpc {
            request: request.to_bytes(),
            response,
        }) {
            // `Client` was dropped, can no longer send requests
            return Err(InvocationError::Dropped);
        }

        // First receive the `oneshot::Receiver` with from the `Client`,
        // then `await` on that to receive the response body for the request.
        if let Ok(response) = rx.await {
            if let Ok(result) = response.await {
                match result {
                    Ok(body) => R::Return::from_bytes(&body).map_err(|e| e.into()),
                    Err(e) => Err(e),
                }
            } else {
                // `Sender` dropped, won't be receiving a response for this
                Err(InvocationError::Dropped)
            }
        } else {
            // `Client` dropped, won't be receiving a response for this
            Err(InvocationError::Dropped)
        }
    }

    /// Gracefully tell the [`Client`] that created this handle to disconnect and stop receiving
    /// things from the network.
    ///
    /// If the client has already been dropped (and thus disconnected), this method does nothing.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::ClientHandle) {
    /// client.disconnect().await;
    /// # }
    /// ```
    pub async fn disconnect(&mut self) {
        let (response, rx) = oneshot::channel();

        if let Ok(_) = self.tx.send(Request::Disconnect { response }) {
            // It's fine to drop errors here, it means the channel was dropped by the `Client`.
            drop(rx.await);
        } else {
            // `Client` is already dropped, no need to disconnect again.
        }
    }
}
