// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::{Client, ClientInner};
use crate::client::client::Configuration;
use crate::utils;
use grammers_mtsender::utils::sleep;
use grammers_mtsender::{InvocationError, RpcError, SenderPool};
use grammers_tl_types::{self as tl, Deserializable};
use log::info;
use std::sync::Arc;

/// Method implementations directly related with network connectivity.
impl Client {
    /// Creates and returns a new client instance upon successful connection to Telegram.
    ///
    /// If the session in the configuration did not have an authorization key, a new one
    /// will be created and the session will be saved with it.
    ///
    /// The connection will be initialized with the data from the input configuration.
    ///
    /// The [`SenderPoolHandle`] does not keep a reference to the [`Session`] or `api_id`,
    /// but the [`SenderPool`] itself does, so the latter is used as input to guarantee that
    /// the values are correctly shared between the pool and the client handles.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    /// use grammers_client::Client;
    /// use grammers_session::storages::TlSession;
    /// use grammers_mtsender::SenderPool;
    ///
    /// // Note: these are example values and are not actually valid.
    /// //       Obtain your own with the developer's phone at https://my.telegram.org.
    /// const API_ID: i32 = 932939;
    ///
    /// # async fn f() -> Result<(), Box<dyn std::error::Error>> {
    /// let session = Arc::new(TlSession::load_file_or_create("hello-world.session")?);
    /// let pool = SenderPool::new(Arc::clone(&session), API_ID);
    /// let client = Client::new(&pool);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(sender_pool: &SenderPool) -> Self {
        Self::with_configuration(sender_pool, Default::default())
    }

    /// Like [`Self::new`] but with a custom [`Configuration`].
    pub fn with_configuration(sender_pool: &SenderPool, configuration: Configuration) -> Self {
        // TODO Sender doesn't have a way to handle backpressure yet
        Self(Arc::new(ClientInner {
            id: utils::generate_random_id(),
            session: Arc::clone(&sender_pool.runner.session),
            api_id: sender_pool.runner.api_id,
            handle: sender_pool.handle.clone(),
            configuration,
        }))
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
        let dc_id = self.0.session.home_dc_id();
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
                })) if !slept_flood && seconds <= self.0.configuration.flood_sleep_threshold => {
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
