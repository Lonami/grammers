// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
//pub mod iterators;
//mod parsers;
pub mod types;

use std::convert::TryInto;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::{SystemTime, UNIX_EPOCH};

use async_std::net::TcpStream;
use async_std::task::{self, JoinHandle};
use grammers_mtproto::errors::RpcError;
use grammers_mtproto::transports::TransportFull;
use grammers_mtsender::{create_mtp, MtpSender};
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_session::Session;
use grammers_tl_types::{self as tl, Deserializable, RemoteCall, Serializable};

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

/// Generate a random message ID suitable for `send_message`.
fn generate_random_message_id() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before epoch")
        .as_nanos() as i64
}

#[derive(Debug)]
pub enum SignInError {
    NoCodeSent,
    SignUpRequired {
        terms_of_service: Option<tl::types::help::TermsOfService>,
    },
    InvalidCode,
    Other(InvocationError),
}

impl From<io::Error> for SignInError {
    fn from(error: io::Error) -> Self {
        Self::Other(error.into())
    }
}

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

impl Client {
    /// Creates and returns a new client instance upon successful connection to Telegram.
    ///
    /// If the session in the configuration did not have an authorization key, a new one
    /// will be created and the session will be saved with it.
    pub async fn connect(config: Config) -> Result<Self, AuthorizationError> {
        let addr = if let Some((_dc_id, addr)) = config.session.user_dc {
            addr
        } else {
            let (addr, port) = DC_ADDRESSES[0];
            SocketAddr::new(IpAddr::V4(addr), port)
        };

        let stream = TcpStream::connect(addr).await?;
        let in_stream = stream.clone();
        let out_stream = stream;

        let (sender, _updates, handler) =
            create_mtp::<TransportFull, _, _>((in_stream, out_stream), None).await?;
        let handler = task::spawn(handler.run());

        let mut client = Client {
            config,
            sender,
            _handler: handler,
            last_phone_hash: None,
        };
        client.init_connection().await?;
        Ok(client)
    }

    /// Returns `true` if the current account is authorized. Otherwise,
    /// logging in will be required before being able to invoke requests.
    pub async fn is_authorized(&mut self) -> Result<bool, InvocationError> {
        match self.invoke(&tl::functions::updates::GetState {}).await {
            Ok(_) => Ok(true),
            Err(InvocationError::RPC(_)) => Ok(false),
            Err(err) => Err(err),
        }
    }

    /// Requests the login code for the account associated to the given phone
    /// number via another Telegram application or SMS.
    pub async fn request_login_code(
        &mut self,
        phone: &str,
        api_id: i32,
        api_hash: &str,
    ) -> Result<tl::types::auth::SentCode, AuthorizationError> {
        let request = tl::functions::auth::SendCode {
            phone_number: phone.to_string(),
            api_id,
            api_hash: api_hash.to_string(),
            settings: tl::types::CodeSettings {
                allow_flashcall: false,
                current_number: false,
                allow_app_hash: false,
            }
            .into(),
        };

        let sent_code: tl::types::auth::SentCode = match self.invoke(&request).await {
            Ok(x) => x.into(),
            Err(InvocationError::RPC(RpcError { name, value, .. })) if name == "PHONE_MIGRATE" => {
                // Since we are not logged in (we're literally requesting for
                // the code to login now), there's no need to export the current
                // authorization and re-import it at a different datacenter.
                //
                // Just connect and generate a new authorization key with it
                // before trying again. Don't want to replace `self.sender`
                // unless the entire process succeeds.
                self.replace_mtsender(value.unwrap() as i32)?;
                self.init_invoke(&request).await?.into()
            }
            Err(e) => return Err(e.into()),
        };

        self.last_phone_hash = Some((phone.to_string(), sent_code.phone_code_hash.clone()));
        Ok(sent_code)
    }

    /// Signs in to the user account. To have the login code be sent, use
    /// [`request_login_code`] first.
    ///
    /// [`request_login_code`]: #method.request_login_code
    pub async fn sign_in(&mut self, code: &str) -> Result<tl::types::User, SignInError> {
        let (phone_number, phone_code_hash) = if let Some(t) = self.last_phone_hash.take() {
            t
        } else {
            return Err(SignInError::NoCodeSent);
        };

        match self
            .invoke(&tl::functions::auth::SignIn {
                phone_number,
                phone_code_hash,
                phone_code: code.to_string(),
            })
            .await
        {
            Ok(tl::enums::auth::Authorization::Authorization(x)) => {
                // Safe to unwrap, Telegram won't send `UserEmpty` here.
                Ok(x.user.try_into().unwrap())
            }
            Ok(tl::enums::auth::Authorization::SignUpRequired(x)) => {
                Err(SignInError::SignUpRequired {
                    terms_of_service: x.terms_of_service.map(|tos| tos.into()),
                })
            }
            Err(InvocationError::RPC(RpcError { name, .. })) if name.starts_with("PHONE_CODE_") => {
                Err(SignInError::InvalidCode)
            }
            Err(error) => Err(SignInError::Other(error)),
        }
    }

    /// Signs in to the bot account associated with this token.
    pub async fn bot_sign_in(
        &mut self,
        token: &str,
        api_id: i32,
        api_hash: &str,
    ) -> Result<(), AuthorizationError> {
        let request = tl::functions::auth::ImportBotAuthorization {
            flags: 0,
            api_id,
            api_hash: api_hash.to_string(),
            bot_auth_token: token.to_string(),
        };

        let _result = match self.invoke(&request).await {
            Ok(x) => x,
            Err(InvocationError::RPC(RpcError { name, value, .. })) if name == "USER_MIGRATE" => {
                self.replace_mtsender(value.unwrap() as i32)?;
                self.init_invoke(&request).await?
            }
            Err(e) => return Err(e.into()),
        };

        Ok(())
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
    fn replace_mtsender(&mut self, _server_id: i32) -> Result<(), AuthorizationError> {
        todo!() /*
                let server_address = DC_ADDRESSES[server_id as usize].parse().unwrap();
                self.session.set_user_datacenter(server_id, &server_address);
                self.session.save()?;

                // Don't replace `self.sender` unless the entire process succeeds.
                self.sender = {
                    let mut sender = MTSender::connect(server_address)?;
                    let auth_key = sender.generate_auth_key()?;
                    self.session
                        .set_auth_key_data(server_id, &auth_key.to_bytes());
                    self.session.save()?;
                    sender
                };

                Ok(())*/
    }

    /// Resolves a username into the user that owns it, if any.
    pub async fn resolve_username(
        &mut self,
        username: &str,
    ) -> Result<Option<tl::types::User>, InvocationError> {
        let tl::enums::contacts::ResolvedPeer::Peer(tl::types::contacts::ResolvedPeer {
            peer,
            users,
            ..
        }) = self
            .invoke(&tl::functions::contacts::ResolveUsername {
                username: username.into(),
            })
            .await?;

        match peer {
            tl::enums::Peer::User(tl::types::PeerUser { user_id }) => {
                return Ok(users
                    .into_iter()
                    .filter_map(|user| match user {
                        tl::enums::User::User(user) => {
                            if user.id == user_id {
                                Some(user)
                            } else {
                                None
                            }
                        }
                        tl::enums::User::Empty(_) => None,
                    })
                    .next());
            }
            tl::enums::Peer::Chat(_) => {}
            tl::enums::Peer::Channel(_) => {}
        }

        Ok(None)
    }

    /// Sends a text message to the desired chat.
    // TODO don't require nasty InputPeer
    pub async fn send_message(
        &mut self,
        chat: tl::enums::InputPeer,
        message: types::Message,
    ) -> Result<(), InvocationError> {
        self.invoke(&tl::functions::messages::SendMessage {
            no_webpage: !message.link_preview,
            silent: message.silent,
            background: message.background,
            clear_draft: message.clear_draft,
            peer: chat,
            reply_to_msg_id: message.reply_to,
            message: message.text,
            random_id: generate_random_message_id(),
            reply_markup: message.reply_markup,
            entities: if message.entities.is_empty() {
                None
            } else {
                Some(message.entities)
            },
            schedule_date: message.schedule_date,
        })
        .await?;
        Ok(())
    }

    // TODO don't keep this, it should be implicit
    pub async fn input_peer_for_username(
        &mut self,
        username: &str,
    ) -> Result<tl::enums::InputPeer, InvocationError> {
        if username.eq_ignore_ascii_case("me") {
            Ok(tl::enums::InputPeer::PeerSelf(tl::types::InputPeerSelf {}))
        } else if let Some(user) = self.resolve_username(username).await? {
            Ok(tl::types::InputPeerUser {
                user_id: user.id,
                access_hash: user.access_hash.unwrap(), // TODO don't unwrap
            }
            .into())
        } else {
            // TODO same rationale as IntoInput<tl::enums::InputPeer> for tl::types::User
            Err(InvocationError::IO(io::Error::new(
                io::ErrorKind::NotFound,
                "no user has that username",
            )))
        }
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
