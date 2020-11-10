// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Methods related to logging in or signing up.

use super::net::connect_sender;
use super::Client;
use crate::types::SentCode;
use grammers_mtproto::mtp::RpcError;
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;
use std::convert::TryInto;
use std::fmt;

/// The error type which is returned when signing in fails.
#[derive(Debug)]
pub enum SignInError {
    SignUpRequired {
        terms_of_service: Option<tl::types::help::TermsOfService>,
    },
    InvalidCode,
    Other(InvocationError),
}

impl fmt::Display for SignInError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SignInError::*;
        match self {
            SignUpRequired {
                terms_of_service: tos,
            } => write!(f, "sign in error: sign up required: {:?}", tos),
            InvalidCode => write!(f, "sign in error: invalid code"),
            Other(e) => write!(f, "sign in error: {}", e),
        }
    }
}

impl std::error::Error for SignInError {}

impl Client {
    /// Returns `true` if the current account is authorized. Otherwise,
    /// logging in will be required before being able to invoke requests.
    pub async fn is_authorized(&mut self) -> Result<bool, InvocationError> {
        match self.invoke(&tl::functions::updates::GetState {}).await {
            Ok(_) => Ok(true),
            Err(InvocationError::Rpc(_)) => Ok(false),
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
    ) -> Result<SentCode, AuthorizationError> {
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
            Err(InvocationError::Rpc(RpcError { name, value, .. })) if name == "PHONE_MIGRATE" => {
                // Since we are not logged in (we're literally requesting for
                // the code to login now), there's no need to export the current
                // authorization and re-import it at a different datacenter.
                //
                // Just connect and generate a new authorization key with it
                // before trying again.
                self.config.session.auth_key = None;
                self.sender = connect_sender(value.unwrap() as i32, &mut self.config).await?;
                self.invoke(&request).await?.into()
            }
            Err(e) => return Err(e.into()),
        };

        Ok(SentCode {
            phone: phone.to_string(),
            phone_code_hash: sent_code.phone_code_hash,
        })
    }

    /// Signs in to the user account. To have the login code be sent, use
    /// [`request_login_code`] first.
    ///
    /// [`request_login_code`]: #method.request_login_code
    pub async fn sign_in(
        &mut self,
        sent: &SentCode,
        code: &str,
    ) -> Result<tl::types::User, SignInError> {
        match self
            .invoke(&tl::functions::auth::SignIn {
                phone_number: sent.phone.clone(),
                phone_code_hash: sent.phone_code_hash.clone(),
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
            Err(InvocationError::Rpc(RpcError { name, .. })) if name.starts_with("PHONE_CODE_") => {
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
            Err(InvocationError::Rpc(RpcError { name, value, .. })) if name == "USER_MIGRATE" => {
                self.config.session.auth_key = None;
                self.sender = connect_sender(value.unwrap() as i32, &mut self.config).await?;
                self.invoke(&request).await?
            }
            Err(e) => return Err(e.into()),
        };

        Ok(())
    }
}
