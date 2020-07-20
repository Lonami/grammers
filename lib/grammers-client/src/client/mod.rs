// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//pub mod iterators;
//mod parsers;
mod auth;
mod chats;
mod messages;
mod updates;

use async_std::net::TcpStream;
use async_std::task::{self, JoinHandle};
pub use auth::SignInError;
use grammers_crypto::auth_key::AuthKey;
use grammers_mtproto::transports::TransportFull;
use grammers_mtsender::{create_mtp, MtpSender};
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_session::Session;
use grammers_tl_types::{self as tl, Deserializable, RemoteCall, Serializable};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

//pub use iterators::Dialogs;

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

/// When no locale is found, use this one instead.
const DEFAULT_LOCALE: &str = "en";

/// A client capable of connecting to Telegram and invoking requests.
pub struct Client {
    config: Config,
    sender: MtpSender,
    updates: crate::UpdateStream,
    _handler: JoinHandle<()>,

    /// The stored phone and its hash from the last `request_login_code` call.
    last_phone_hash: Option<(String, String)>,
}

/*
/// Implementors of this trait have a way to turn themselves into the
/// desired input parameter.
pub trait IntoInput<T> {
    fn convert(&self, client: &mut Client) -> Result<T, InvocationError>;
}

impl IntoInput<tl::enums::InputPeer> for tl::types::User {
    fn convert(&self, _client: &mut Client) -> Result<tl::enums::InputPeer, InvocationError> {
        if let Some(access_hash) = self.access_hash {
            Ok(tl::enums::InputPeer::User(
                tl::types::InputPeerUser {
                    user_id: self.id,
                    access_hash,
                },
            ))
        } else {
            // TODO how should custom "into input" errors be handled?
            // this is pretty much the only case where we need a custom one,
            // maybe a "conversion failure" which is either invocation or custom
            Err(InvocationError::IO(io::Error::new(
                io::ErrorKind::NotFound,
                "user is missing access_hash",
            )))
        }
    }
}

impl IntoInput<tl::enums::InputPeer> for &str {
    fn convert(&self, client: &mut Client) -> Result<tl::enums::InputPeer, InvocationError> {
        if self.eq_ignore_ascii_case("me") {
            Ok(tl::enums::InputPeer::PeerSelf(
                tl::types::InputPeerSelf {},
            ))
        } else if let Some(user) = client.resolve_username(self)? {
            user.convert(client)
        } else {
            // TODO same rationale as IntoInput<tl::enums::InputPeer> for tl::types::User
            Err(InvocationError::IO(io::Error::new(
                io::ErrorKind::NotFound,
                "no user has that username",
            )))
        }
    }
}
*/

pub struct Config {
    pub session: Session,
    pub api_id: i32,
    pub api_hash: String,
    pub params: InitParams,
}

pub struct InitParams {
    pub device_model: String,
    pub system_version: String,
    pub app_version: String,
    pub system_lang_code: String,
    pub lang_code: String,
}

impl Default for InitParams {
    fn default() -> Self {
        let info = os_info::get();

        let mut system_lang_code = locate_locale::system();
        if system_lang_code.is_empty() {
            system_lang_code.push_str(DEFAULT_LOCALE);
        }

        let mut lang_code = locate_locale::user();
        if lang_code.is_empty() {
            lang_code.push_str(DEFAULT_LOCALE);
        }

        Self {
            device_model: format!("{} {}", info.os_type(), info.bitness()),
            system_version: info.version().to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            system_lang_code,
            lang_code,
        }
    }
}

// TODO narrow the error
// TODO not entirely happy with the fact we need crypto just for AuthKey, should re-export (same in sessions)
async fn create_sender(
    dc_id: i32,
    auth_key: &mut Option<AuthKey>,
) -> Result<(MtpSender, crate::UpdateStream, JoinHandle<()>), AuthorizationError> {
    let server_address = {
        let (addr, port) = DC_ADDRESSES[dc_id as usize];
        SocketAddr::new(IpAddr::V4(addr), port)
    };

    let stream = TcpStream::connect(server_address).await?;
    let in_stream = stream.clone();
    let out_stream = stream;

    let (sender, updates, handler) =
        create_mtp::<TransportFull, _, _>((in_stream, out_stream), auth_key).await?;

    let handler = task::spawn(handler.run());
    Ok((sender, updates, handler))
}

impl Client {
    /// Creates and returns a new client instance upon successful connection to Telegram.
    ///
    /// If the session in the configuration did not have an authorization key, a new one
    /// will be created and the session will be saved with it.
    pub async fn connect(mut config: Config) -> Result<Self, AuthorizationError> {
        // TODO we don't handle -404 (unknown good authkey) here, need recreate
        let dc_id = config.session.user_dc.unwrap_or(0);
        let (sender, updates, handler) = create_sender(dc_id, &mut config.session.auth_key).await?;
        config.session.save()?;

        let mut client = Client {
            config,
            sender,
            updates,
            _handler: handler,
            last_phone_hash: None,
        };
        client.init_connection().await?;
        Ok(client)
    }

    /// Replace the current `MTSender` with one connected to a different datacenter.
    ///
    /// This process is not quite a migration, since it will ignore any previous
    /// authorization key.
    ///
    /// The sender will not be replaced unless the entire process succeeds.
    ///
    /// After the sender is replaced, the next request should use `init_invoke`.
    ///
    /// # Panics
    ///
    /// If the server ID is not within the known identifiers.
    async fn replace_mtsender(&mut self, dc_id: i32) -> Result<(), AuthorizationError> {
        self.config.session.auth_key = None;
        let (sender, updates, handler) =
            create_sender(dc_id, &mut self.config.session.auth_key).await?;
        self.config.session.user_dc = Some(dc_id);
        self.config.session.save()?;

        self.sender = sender;
        self.updates = updates;
        self._handler = handler;

        Ok(())
    }

    /// Initializes the connection with Telegram. If this is never done on
    /// a fresh session, then Telegram won't know which layer to use and a
    /// very old one will be used (which we will fail to understand).
    async fn init_connection(&mut self) -> Result<(), InvocationError> {
        // TODO store config
        let _config = self.init_invoke(&tl::functions::help::GetConfig {}).await?;
        Ok(())
    }

    /// Wraps the request in `invokeWithLayer(initConnection(...))` and
    /// invokes that. Should be used by the first request after connect.
    async fn init_invoke<R: RemoteCall>(
        &mut self,
        request: &R,
    ) -> Result<R::Return, InvocationError> {
        // TODO figure out what we're doing wrong because Telegram seems to
        //      reply some constructor we are unaware of, even though we
        //      explicitly did invokeWithLayer. this will fail, because
        //      we want to return the right type (before we ignored it).
        //
        // a second call to getConfig will work just fine though.
        //
        // this also seems to have triggered RPC_CALL_FAIL
        let data = self
            .invoke(&tl::functions::InvokeWithLayer {
                layer: tl::LAYER,
                query: tl::functions::InitConnection {
                    api_id: self.config.api_id,
                    device_model: self.config.params.device_model.clone(),
                    system_version: self.config.params.system_version.clone(),
                    app_version: self.config.params.app_version.clone(),
                    system_lang_code: self.config.params.system_lang_code.clone(),
                    lang_pack: "".into(),
                    lang_code: self.config.params.lang_code.clone(),
                    proxy: None,
                    query: request.to_bytes().into(),
                }
                .to_bytes()
                .into(),
            })
            .await?;

        Ok(R::Return::from_bytes(&data.0)?)
    }

    /// Invokes a raw request, and returns its result.
    pub async fn invoke<R: RemoteCall>(
        &mut self,
        request: &R,
    ) -> Result<R::Return, InvocationError> {
        self.sender.invoke(request).await
    }
}
