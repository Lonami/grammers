// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::client::ClientState;
use super::{Client, ClientInner, Config};
use crate::utils;
use grammers_mtsender::utils::sleep;
use grammers_mtsender::{AuthorizationError, InvocationError, RpcError};
use grammers_session::{MessageBoxes, state_to_update_state};
use grammers_tl_types::{self as tl, Deserializable};
use log::info;
use std::sync::{Arc, RwLock};

const DEFAULT_DC: i32 = 2;

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
    /// use grammers_mtsender::{SenderPool, Configuration};
    ///
    /// // Note: these are example values and are not actually valid.
    /// //       Obtain your own with the developer's phone at https://my.telegram.org.
    /// const API_ID: i32 = 932939;
    /// const API_HASH: &str = "514727c32270b9eb8cc16daf17e21e57";
    ///
    /// # async fn f() -> Result<(), Box<dyn std::error::Error>> {
    /// let (_pool, handle, _) = SenderPool::new(Configuration {
    ///     api_id: API_ID,
    ///     ..Default::default()
    /// });
    /// let client = Client::connect(Config {
    ///     session: Session::load_file_or_create("hello-world.session")?,
    ///     api_id: API_ID,
    ///     api_hash: API_HASH.to_string(),
    ///     handle: handle,
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
        let message_box = if config.params.catch_up {
            if let Some(state) = config.session.get_state() {
                MessageBoxes::load(state)
            } else {
                MessageBoxes::new()
            }
        } else {
            // If the user doesn't want to bother with catching up on previous update, start with
            // pristine state instead.
            MessageBoxes::new()
        };

        // "Remove" the limit to avoid checking for it (and avoid warning).
        if let Some(0) = config.params.update_queue_limit {
            config.params.update_queue_limit = None;
        }

        // Don't bother getting pristine update state if we're not logged in.
        let should_get_state = message_box.is_empty() && config.session.signed_in();

        // TODO Sender doesn't have a way to handle backpressure yet
        let client = Self(Arc::new(ClientInner {
            id: utils::generate_random_id(),
            config,
            state: RwLock::new(ClientState { dc_id }),
        }));

        if should_get_state {
            match client.invoke(&tl::functions::updates::GetState {}).await {
                Ok(state) => {
                    client
                        .0
                        .config
                        .session
                        .set_state(state_to_update_state(state));
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
        let dc_id = { self.0.state.read().unwrap().dc_id };
        self.do_invoke_in_dc(dc_id, request.to_bytes())
            .await
            .and_then(|body| R::Return::from_bytes(&body).map_err(|e| e.into()))
    }

    /// Like [`Self::invoke`], but in the specified DC.
    pub async fn invoke_in_dc<R: tl::RemoteCall>(
        &self,
        dc_id: i32,
        request: &R,
    ) -> Result<R::Return, InvocationError> {
        self.do_invoke_in_dc(dc_id, request.to_bytes())
            .await
            .and_then(|body| R::Return::from_bytes(&body).map_err(|e| e.into()))
    }

    async fn do_invoke_in_dc(
        &self,
        dc_id: i32,
        request_body: Vec<u8>,
    ) -> Result<Vec<u8>, InvocationError> {
        let mut slept_flood = false;

        loop {
            match self
                .0
                .config
                .handle
                .invoke_in_dc(dc_id, request_body.clone())
                .await
            {
                Ok(response) => break Ok(response),
                Err(InvocationError::Rpc(RpcError {
                    name,
                    code: 420,
                    value: Some(seconds),
                    ..
                })) if !slept_flood && seconds <= self.0.config.params.flood_sleep_threshold => {
                    let delay = std::time::Duration::from_secs(seconds as _);
                    info!("sleeping on {} for {:?} before retrying", name, delay,);
                    sleep(delay).await;
                    slept_flood = true;
                    continue;
                }
                Err(e) => break Err(e),
            }
        }
    }
}
