// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{Client, ClientInner, Config};
use crate::client::client::FileDownloader;
use crate::utils::{self, AsyncMutex, Mutex};
use grammers_mtproto::mtp::{self, RpcError};
use grammers_mtproto::transport;
use grammers_mtproto::transport::Full;
use grammers_mtsender::{self as sender, AuthorizationError, InvocationError, Sender};
use grammers_session::{ChatHashCache, MessageBox};
use grammers_tl_types::{self as tl, Deserializable};
use log::{debug, info};
use mtp::Encrypted;
use sender::Enqueuer;
use std::collections::{HashMap, VecDeque};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::sync::{Notify, RwLock};

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

pub(crate) async fn connect_sender(
    dc_id: i32,
    config: &Config,
) -> Result<(Sender<transport::Full, mtp::Encrypted>, Enqueuer), AuthorizationError> {
    let transport = transport::Full::new();

    let addr: SocketAddr = if let Some(ip) = config.params.server_addr {
        ip
    } else {
        DC_ADDRESSES[dc_id as usize].into()
    };

    let (mut sender, request_tx) = if let Some(auth_key) = config.session.dc_auth_key(dc_id) {
        info!(
            "creating a new sender with existing auth key to dc {} {:?}",
            dc_id, addr
        );

        #[cfg(feature = "proxy")]
        if let Some(url) = config.params.proxy_url.as_ref() {
            sender::connect_via_proxy_with_auth(transport, addr, auth_key, url).await?
        } else {
            sender::connect_with_auth(transport, addr, auth_key).await?
        }

        #[cfg(not(feature = "proxy"))]
        sender::connect_with_auth(transport, addr, auth_key).await?
    } else {
        info!(
            "creating a new sender and auth key in dc {} {:?}",
            dc_id, addr
        );

        #[cfg(feature = "proxy")]
        let (sender, tx) = if let Some(url) = config.params.proxy_url.as_ref() {
            sender::connect_via_proxy(transport, addr, url).await?
        } else {
            sender::connect(transport, addr).await?
        };

        #[cfg(not(feature = "proxy"))]
        let (sender, tx) = sender::connect(transport, addr).await?;

        config.session.insert_dc(dc_id, addr, sender.auth_key());
        (sender, tx)
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

    Ok((sender, request_tx))
}

/// Method implementations directly related with network connectivity.
impl Client {
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
    /// use grammers_session::Session;
    ///
    /// // Note: these are example values and are not actually valid.
    /// //       Obtain your own with the developer's phone at https://my.telegram.org.
    /// const API_ID: i32 = 932939;
    /// const API_HASH: &str = "514727c32270b9eb8cc16daf17e21e57";
    ///
    /// # async fn f() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = Client::connect(Config {
    ///     session: Session::load_file_or_create("hello-world.session")?,
    ///     api_id: API_ID,
    ///     api_hash: API_HASH.to_string(),
    ///     params: Default::default(),
    /// }).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn connect(mut config: Config) -> Result<Self, AuthorizationError> {
        let dc_id = config
            .session
            .get_user()
            .map(|u| u.dc)
            .unwrap_or(DEFAULT_DC);
        let (sender, request_tx) = connect_sender(dc_id, &config).await?;
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

        // Pre-allocate the right `VecDeque` size if a limit is given.
        let updates = if let Some(limit) = config.params.update_queue_limit {
            VecDeque::with_capacity(limit)
        } else {
            VecDeque::new()
        };

        // "Remove" the limit to avoid checking for it (and avoid warning).
        if let Some(0) = config.params.update_queue_limit {
            config.params.update_queue_limit = None;
        }

        let self_user = config.session.get_user();

        // TODO Sender doesn't have a way to handle backpressure yet
        let client = Self(Arc::new(ClientInner {
            id: utils::generate_random_id(),
            sender: AsyncMutex::new("client.sender", sender),
            stepping_done: Notify::new(),
            dc_id: Mutex::new("client.dc_id", dc_id),
            config,
            message_box: Mutex::new("client.message_box", message_box),
            chat_hashes: Mutex::new(
                "client.chat_hashes",
                ChatHashCache::new(self_user.map(|u| (u.id, u.bot))),
            ),
            last_update_limit_warn: Mutex::new("client.last_update_limit_warn", None),
            updates: Mutex::new("client.updates", updates),
            request_tx: Mutex::new("client.request_tx", request_tx),
            downloader_map: RwLock::new(HashMap::new()),
        }));

        // Don't bother getting pristine state if we're not logged in.
        if client
            .0
            .message_box
            .lock("client.connect.is_empty")
            .is_empty()
            && client.0.config.session.signed_in()
        {
            match client.invoke(&tl::functions::updates::GetState {}).await {
                Ok(state) => {
                    client
                        .0
                        .message_box
                        .lock("client.connect.set_state")
                        .set_state(state);
                    client.sync_update_state();
                }
                Err(_err) => {
                    // The account may no longer actually be logged in, or it can rarely fail.
                    // `message_box` will try to correct its state as updates arrive.
                }
            }
        }

        Ok(client)
    }

    /// Invoke a raw API call. This directly sends the request to Telegram's servers.
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
    /// # async fn f(client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_tl_types as tl;
    ///
    /// dbg!(client.invoke(&tl::functions::Ping { ping_id: 0 }).await?);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn invoke<R: tl::RemoteCall>(
        &self,
        request: &R,
    ) -> Result<R::Return, InvocationError> {
        self.invoke_request(
            request,
            &self.0.request_tx,
            &self.0.sender,
            &self.0.stepping_done,
        )
        .await
    }

    pub(crate) async fn invoke_request<R: tl::RemoteCall>(
        &self,
        request: &R,
        enqueuer: &Mutex<Enqueuer>,
        sender: &AsyncMutex<Sender<Full, Encrypted>>,
        notifier: &Notify,
    ) -> Result<R::Return, InvocationError> {
        let mut slept_flood = false;
        let sleep_thresh = self.0.config.params.flood_sleep_threshold.unwrap_or(0);

        let mut rx = enqueuer.lock("invoke").enqueue(request);
        loop {
            match rx.try_recv() {
                Ok(response) => match response {
                    Ok(body) => break R::Return::from_bytes(&body).map_err(|e| e.into()),
                    Err(InvocationError::Rpc(RpcError {
                        name,
                        code: 420,
                        value: Some(seconds),
                        ..
                    })) if !slept_flood && seconds <= sleep_thresh => {
                        let delay = std::time::Duration::from_secs(seconds as _);
                        info!(
                            "sleeping on {} for {:?} before retrying {}",
                            name,
                            delay,
                            std::any::type_name::<R>()
                        );
                        tokio::time::sleep(delay).await;
                        slept_flood = true;
                        rx = enqueuer.lock("invoke").enqueue(request);
                        continue;
                    }
                    Err(e) => break Err(e),
                },
                Err(TryRecvError::Empty) => {
                    self.step_sender(sender, notifier).await?;
                }
                Err(TryRecvError::Closed) => {
                    panic!("request channel dropped before receiving a result")
                }
            }
        }
    }

    async fn export_authorization(
        &self,
        target_dc_id: i32,
    ) -> Result<tl::types::auth::ExportedAuthorization, InvocationError> {
        let request = tl::functions::auth::ExportAuthorization {
            dc_id: target_dc_id,
        };
        match self.invoke(&request).await {
            Ok(tl::enums::auth::ExportedAuthorization::Authorization(exported_auth)) => {
                Ok(exported_auth)
            }
            Err(e) => Err(e),
        }
    }

    async fn connect_sender(&self, dc_id: i32) -> Result<Arc<FileDownloader>, InvocationError> {
        let mut mutex = self.0.downloader_map.write().await;
        debug!("Connecting new datacenter {}", dc_id);
        match connect_sender(dc_id, &self.0.config).await {
            Ok((new_sender, new_tx)) => {
                let new_downloader = Arc::new(FileDownloader {
                    sender: AsyncMutex::new("dc_sender", new_sender),
                    request_tx: Mutex::new("dc_enqueuer", new_tx),
                    stepping_done: Notify::new(),
                });

                // export auth
                let authorization = self.export_authorization(dc_id).await?;

                // import into new sender
                let request = tl::functions::auth::ImportAuthorization {
                    id: authorization.id,
                    bytes: authorization.bytes,
                };
                self.invoke_request(
                    &request,
                    &new_downloader.request_tx,
                    &new_downloader.sender,
                    &new_downloader.stepping_done,
                )
                .await?;

                mutex.insert(dc_id, new_downloader.clone());
                Ok(new_downloader.clone())
            }
            Err(e) => panic!("Cannot obtain new sender to datacenter {}", e),
        }
    }

    async fn get_downloader(
        &self,
        dc_id: i32,
    ) -> Result<Option<Arc<FileDownloader>>, InvocationError> {
        return Ok({
            let guard = self.0.downloader_map.read().await;
            guard.get(&dc_id).cloned()
        });
    }

    pub async fn invoke_in_dc<R: tl::RemoteCall>(
        &self,
        request: &R,
        dc_id: i32,
    ) -> Result<R::Return, InvocationError> {
        let downloader = match self.get_downloader(dc_id).await? {
            None => self.connect_sender(dc_id).await?,
            Some(fd) => fd,
        };
        self.invoke_request(
            request,
            &downloader.request_tx,
            &downloader.sender,
            &downloader.stepping_done,
        )
        .await
    }

    /// Perform a single network step.
    ///
    /// Most commonly, you will want to use the higher-level abstraction [`Client::next_update`]
    /// instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// loop {
    ///     // Process network events forever until we gracefully disconnect or get an error.
    ///     client.step().await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn step(&self) -> Result<(), sender::ReadError> {
        self.step_sender(&self.0.sender, &self.0.stepping_done)
            .await
    }
    async fn step_sender(
        &self,
        sender: &AsyncMutex<Sender<Full, Encrypted>>,
        notifier: &Notify,
    ) -> Result<(), sender::ReadError> {
        match sender.try_lock("client.step") {
            Ok(mut sender) => {
                // Sender was unlocked, we're the ones that will perform the network step.
                let updates = sender.step().await?;
                notifier.notify_waiters();
                self.process_socket_updates(updates);

                // TODO request cancellation if this is Err
                // (perhaps a method on the sender to cancel_all)
                Ok(())
            }
            Err(_) => {
                // Someone else is already performing the network step. Wait for the step to
                // complete and return immediately without stepping again. The caller wants
                // *one* step to complete, but it doesn't care *who* completes it.
                notifier.notified().await;
                Ok(())
            }
        }
    }

    /// Run the client by repeatedly calling [`Client::step`] until a graceful disconnection
    /// occurs, or a network error occurs. Incoming updates are ignored and simply dropped.
    /// instead.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// client.run_until_disconnected().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn run_until_disconnected(self) -> Result<(), sender::ReadError> {
        loop {
            // TODO review doc comments regarding disconnects
            self.step().await?;
        }
    }
}
