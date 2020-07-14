use crate::Client;
use grammers_mtproto::errors::RpcError;
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;
use std::convert::TryInto;
use std::io;

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

impl Client {
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
                self.replace_mtsender(value.unwrap() as i32).await?;
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
                self.replace_mtsender(value.unwrap() as i32).await?;
                self.init_invoke(&request).await?
            }
            Err(e) => return Err(e.into()),
        };

        Ok(())
    }
}
