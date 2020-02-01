// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod dialogs;
pub mod types;

use std::convert::TryInto;
use std::io;
use std::time::{SystemTime, UNIX_EPOCH};

use grammers_mtproto::errors::RPCError;
use grammers_mtsender::{MTSender, RequestResult};
use grammers_session::Session;
use grammers_tl_types::{self as tl, Serializable, RPC};

// TODO handle PhoneMigrate
const DC_4_ADDRESS: &'static str = "149.154.167.91:443";

/// When no locale is found, use this one instead.
const DEFAULT_LOCALE: &'static str = "en";

/// A client capable of connecting to Telegram and invoking requests.
pub struct Client {
    api_id: i32,
    sender: MTSender,

    /// The stored phone and its hash from the last `request_login_code` call.
    last_phone_hash: Option<(String, String)>,
}

/// Implementors of this trait have a way to turn themselves into the
/// desired input parameter.
pub trait IntoInput<T> {
    fn convert(&self, client: &mut Client) -> io::Result<T>;
}

impl IntoInput<tl::enums::InputPeer> for tl::types::User {
    fn convert(&self, _client: &mut Client) -> io::Result<tl::enums::InputPeer> {
        if let Some(access_hash) = self.access_hash {
            Ok(tl::enums::InputPeer::InputPeerUser(
                tl::types::InputPeerUser {
                    user_id: self.id,
                    access_hash: access_hash,
                },
            ))
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "user is missing access_hash",
            ))
        }
    }
}

impl IntoInput<tl::enums::InputPeer> for &str {
    fn convert(&self, client: &mut Client) -> io::Result<tl::enums::InputPeer> {
        if let Some(user) = client.resolve_username(self)? {
            user.convert(client)
        } else {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "no user has that username",
            ))
        }
    }
}

/// Generate a random message ID suitable for `send_message`.
fn generate_random_message_id() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time is before epoch")
        .as_nanos() as i64
}

#[derive(Debug)]
pub enum SignInError {
    IO(io::Error),
    NoCodeSent,
    SignUpRequired {
        terms_of_service: Option<tl::types::help::TermsOfService>,
    },
    InvalidCode,
    Other(RPCError),
}

impl From<io::Error> for SignInError {
    fn from(error: io::Error) -> Self {
        Self::IO(error)
    }
}

impl Client {
    /// Returns a new client instance connected to Telegram and returns it.
    ///
    /// This method will generate a new authorization key and connect to a
    /// default datacenter. To prevent logging in every single time, use
    /// [`with_session`] instead, which will reuse a previous session.
    pub fn new() -> io::Result<Self> {
        let mut sender = MTSender::connect(DC_4_ADDRESS)?;
        sender.generate_auth_key()?;
        Self::with_sender(sender)
    }

    /// Configures a new client instance from an existing session and returns
    /// it.
    pub fn with_session(mut session: Box<dyn Session>) -> io::Result<Self> {
        // TODO this doesn't look clean, and configuring the authkey
        //      on the sender this way also seems a bit weird.
        let auth_key;
        let server_id;
        let server_address;
        if let Some((dc_id, dc_addr)) = session.get_user_datacenter() {
            server_id = dc_id;
            server_address = dc_addr;
            auth_key = session.get_auth_key_data(dc_id);
        } else {
            server_id = 4;
            server_address = DC_4_ADDRESS.parse().unwrap();
            auth_key = None;
            session.set_user_datacenter(server_id, &server_address);
            session.save()?;
        }

        let mut sender = MTSender::connect(server_address)?;
        if let Some(auth_key) = auth_key {
            sender.set_auth_key(auth_key);
        } else {
            let auth_key = sender.generate_auth_key()?;
            session.set_auth_key_data(server_id, &auth_key.to_bytes());
            session.save()?;
        }

        Self::with_sender(sender)
    }

    /// Creates a client instance with a sender
    fn with_sender(sender: MTSender) -> io::Result<Self> {
        // TODO user-provided api key
        let mut client = Client {
            api_id: 6,
            sender,
            last_phone_hash: None,
        };
        client.init_connection()?;
        Ok(client)
    }

    /// Returns `true` if the current account is authorized. Otherwise,
    /// logging in will be required before being able to invoke requests.
    pub fn is_authorized(&mut self) -> io::Result<bool> {
        match self.invoke(&tl::functions::updates::GetState {})? {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Requests the login code for the account associated to the given phone
    /// number via another Telegram application or SMS.
    pub fn request_login_code(
        &mut self,
        phone: &str,
        api_id: i32,
        api_hash: &str,
    ) -> io::Result<tl::types::auth::SentCode> {
        let sent_code: tl::types::auth::SentCode = self
            .invoke(&tl::functions::auth::SendCode {
                phone_number: phone.to_string(),
                api_id,
                api_hash: api_hash.to_string(),
                settings: tl::types::CodeSettings {
                    allow_flashcall: false,
                    current_number: false,
                    allow_app_hash: false,
                }
                .into(),
            })??
            .into();

        self.last_phone_hash = Some((phone.to_string(), sent_code.phone_code_hash.clone()));
        Ok(sent_code)
    }

    /// Signs in to the user account. To have the login code be sent, use
    /// [`request_login_code`] first.
    ///
    /// [`request_login_code`]: #method.request_login_code
    pub fn sign_in(&mut self, code: &str) -> Result<tl::types::User, SignInError> {
        let (phone_number, phone_code_hash) = if let Some(t) = self.last_phone_hash.take() {
            t
        } else {
            return Err(SignInError::NoCodeSent);
        };

        match self.invoke(&tl::functions::auth::SignIn {
            phone_number,
            phone_code_hash,
            phone_code: code.to_string(),
        })? {
            Ok(tl::enums::auth::Authorization::Authorization(x)) => {
                // Safe to unwrap, Telegram won't send `UserEmpty` here.
                Ok(x.user.try_into().unwrap())
            }
            Ok(tl::enums::auth::Authorization::AuthorizationSignUpRequired(x)) => {
                Err(SignInError::SignUpRequired {
                    terms_of_service: x.terms_of_service.map(|tos| tos.into()),
                })
            }
            Err(RPCError { name, .. }) if name.starts_with("PHONE_CODE_") => {
                Err(SignInError::InvalidCode)
            }
            Err(error) => Err(SignInError::Other(error)),
        }
    }

    /// Signs in to the bot account associated with this token.
    pub fn bot_sign_in(&mut self, token: &str, api_id: i32, api_hash: &str) -> io::Result<()> {
        self.invoke(&tl::functions::auth::ImportBotAuthorization {
            flags: 0,
            api_id,
            api_hash: api_hash.to_string(),
            bot_auth_token: token.to_string(),
        })??;

        Ok(())
    }

    /// Resolves a username into the user that owns it, if any.
    pub fn resolve_username(&mut self, username: &str) -> io::Result<Option<tl::types::User>> {
        let tl::types::contacts::ResolvedPeer { peer, users, .. } =
            match self.invoke(&tl::functions::contacts::ResolveUsername {
                username: username.into(),
            })?? {
                tl::enums::contacts::ResolvedPeer::ResolvedPeer(x) => x,
            };

        match peer {
            tl::enums::Peer::PeerUser(tl::types::PeerUser { user_id }) => {
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
                        tl::enums::User::UserEmpty(_) => None,
                    })
                    .next());
            }
            tl::enums::Peer::PeerChat(_) => {}
            tl::enums::Peer::PeerChannel(_) => {}
        }

        Ok(None)
    }

    /// Sends a text message to the desired chat.
    pub fn send_message<C: IntoInput<tl::enums::InputPeer>>(
        &mut self,
        chat: C,
        message: &str,
    ) -> io::Result<()> {
        let chat = chat.convert(self)?;
        self.invoke(&tl::functions::messages::SendMessage {
            no_webpage: false,
            silent: false,
            background: false,
            clear_draft: false,
            peer: chat,
            reply_to_msg_id: None,
            message: message.into(),
            random_id: generate_random_message_id(),
            reply_markup: None,
            entities: None,
            schedule_date: None,
        })??;
        Ok(())
    }

    pub fn iter_dialogs(&mut self) -> dialogs::Dialogs {
        dialogs::Dialogs::new(self)
    }

    /// Initializes the connection with Telegram. If this is never done on
    /// a fresh session, then Telegram won't know which layer to use and a
    /// very old one will be used (which we will fail to understand).
    fn init_connection(&mut self) -> io::Result<()> {
        let info = os_info::get();

        let mut system_lang_code = locate_locale::system();
        if system_lang_code.is_empty() {
            system_lang_code.push_str(DEFAULT_LOCALE);
        }

        let mut lang_code = locate_locale::user();
        if lang_code.is_empty() {
            lang_code.push_str(DEFAULT_LOCALE);
        }

        // TODO store config
        self.invoke(&tl::functions::InvokeWithLayer {
            layer: tl::LAYER,
            query: tl::functions::InitConnection {
                api_id: self.api_id,
                device_model: format!("{} {}", info.os_type(), info.bitness()),
                system_version: info.version().to_string(),
                app_version: env!("CARGO_PKG_VERSION").into(),
                system_lang_code,
                lang_pack: "".into(),
                lang_code,
                proxy: None,
                query: tl::functions::help::GetConfig {}.to_bytes(),
            }
            .to_bytes(),
        })??;
        Ok(())
    }

    /// Invokes a raw request, and returns its result.
    pub fn invoke<R: RPC>(&mut self, request: &R) -> RequestResult<R::Return> {
        self.sender.invoke(request)
    }
}
