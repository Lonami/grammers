// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use super::net::connect_sender;
use super::Client;
use crate::types::LoginToken;
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

/// Method implementations related with the authentication of the user into the API.
///
/// Most requests to the API require the user to have authorized their key, stored in the session,
/// before being able to use them.
impl Client {
    /// Returns `true` if the current account is authorized. Otherwise,
    /// logging in will be required before being able to invoke requests.
    ///
    /// This will likely be the first method you want to call on a connected [`Client`]. After you
    /// determine if the account is authorized or not, you will likely want to use either
    /// [`Client::bot_sign_in`] or [`Client::request_login_code`].
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// if client.is_authorized().await? {
    ///     println!("Client is not authorized, you will need to sign_in!");
    /// } else {
    ///     println!("Client already authorized and ready to use!")
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn is_authorized(&mut self) -> Result<bool, InvocationError> {
        match self.invoke(&tl::functions::updates::GetState {}).await {
            Ok(_) => Ok(true),
            Err(InvocationError::Rpc(_)) => Ok(false),
            Err(err) => Err(err),
        }
    }

    /// Signs in to the bot account associated with this token.
    ///
    /// This is the method you need to call to use the client under a bot account.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // Note: these are example values and are not actually valid.
    /// //       Obtain your own with the developer's phone at https://my.telegram.org.
    /// const API_ID: i32 = 932939;
    /// const API_HASH: &str = "514727c32270b9eb8cc16daf17e21e57";
    ///
    /// // Note: this token is obviously fake as well.
    /// //       Obtain your own by talking to @BotFather via a Telegram app.
    /// const TOKEN: &str = "776609994:AAFXAy5-PawQlnYywUlZ_b_GOXgarR3ah_yq";
    ///
    /// let user = match client.bot_sign_in(TOKEN, API_ID, API_HASH).await {
    ///     Ok(user) => user,
    ///     Err(err) => {
    ///         println!("Failed to sign in as a bot :(\n{}", err);
    ///         return Err(err.into());
    ///     }
    /// };
    ///
    /// println!("Signed in as {}!", user.username.unwrap());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bot_sign_in(
        &mut self,
        token: &str,
        api_id: i32,
        api_hash: &str,
    ) -> Result<tl::types::User, AuthorizationError> {
        // TODO api id and hash are in the config yet we ask them here again (and other sign in methods)
        //      use the values from config instead
        let request = tl::functions::auth::ImportBotAuthorization {
            flags: 0,
            api_id,
            api_hash: api_hash.to_string(),
            bot_auth_token: token.to_string(),
        };

        let result = match self.invoke(&request).await {
            Ok(x) => x,
            Err(InvocationError::Rpc(RpcError { name, value, .. })) if name == "USER_MIGRATE" => {
                self.config.session.auth_key = None;
                self.sender = connect_sender(value.unwrap() as i32, &mut self.config).await?;
                self.invoke(&request).await?
            }
            Err(e) => return Err(e.into()),
        };

        match result {
            tl::enums::auth::Authorization::Authorization(x) => {
                // Safe to unwrap, Telegram won't send `UserEmpty` here.
                Ok(x.user.try_into().unwrap())
            }
            tl::enums::auth::Authorization::SignUpRequired(_) => {
                panic!("API returned SignUpRequired even though we're logging in as a bot");
            }
        }
    }

    /// Requests the login code for the account associated to the given phone
    /// number via another Telegram application or SMS.
    ///
    /// This is the method you need to call before being able to sign in to a user account.
    /// After you obtain the code and it's inside your program (e.g. ask the user to enter it
    /// via the console's standard input), you will need to [`Client::sign_in`] to complete the
    /// process.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // Note: these are example values and are not actually valid.
    /// //       Obtain your own with the developer's phone at https://my.telegram.org.
    /// const API_ID: i32 = 932939;
    /// const API_HASH: &str = "514727c32270b9eb8cc16daf17e21e57";
    ///
    /// // Note: this phone number is obviously fake as well.
    /// //       The phone used here does NOT need to be the same as the one used by the developer
    /// //       to obtain the API ID and hash.
    /// const PHONE: &str = "+1 415 555 0132";
    ///
    /// if !client.is_authorized().await? {
    ///     // We're not logged in, so request the login code.
    ///     client.request_login_code(PHONE, API_ID, API_HASH).await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn request_login_code(
        &mut self,
        phone: &str,
        api_id: i32,
        api_hash: &str,
    ) -> Result<LoginToken, AuthorizationError> {
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

        Ok(LoginToken {
            phone: phone.to_string(),
            phone_code_hash: sent_code.phone_code_hash,
        })
    }

    /// Signs in to the user account.
    ///
    /// You must call [`Client::request_login_code`] before using this method in order to obtain
    /// necessary login token, and also have asked the user for the login code.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// # const API_ID: i32 = 0;
    /// # const API_HASH: &str = "";
    /// # const PHONE: &str = "";
    /// fn ask_code_to_user() -> String {
    ///     unimplemented!()
    /// }
    ///
    /// let token = client.request_login_code(PHONE, API_ID, API_HASH).await?;
    /// let code = ask_code_to_user();
    ///
    /// let user = match client.sign_in(&token, &code).await {
    ///     Ok(user) => user,
    ///     Err(err) => {
    ///         println!("Failed to sign in as a user :(\n{}", err);
    ///         return Err(err.into());
    ///     }
    /// };
    ///
    /// println!("Signed in as {}!", user.first_name.unwrap());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn sign_in(
        &mut self,
        token: &LoginToken,
        code: &str,
    ) -> Result<tl::types::User, SignInError> {
        match self
            .invoke(&tl::functions::auth::SignIn {
                phone_number: token.phone.clone(),
                phone_code_hash: token.phone_code_hash.clone(),
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

    /// Signs up a new user account to Telegram.
    ///
    /// This method should be used after [`Client::sign_in`] fails with
    /// [`SignInError::SignUpRequired`]. This is also the only way to know if a certain phone
    /// number is already reigstered on Telegram or not, by trying and failing to login.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// # let token = client.request_login_code("", 0, "").await?;
    /// # let code = "".to_string();
    ///
    /// use grammers_client::SignInError;
    ///
    /// let user = match client.sign_in(&token, &code).await {
    ///     Ok(_user) => {
    ///         println!("Can't create a new account because one already existed!");
    ///         return Err("account already exists".into());
    ///     },
    ///     Err(SignInError::SignUpRequired { terms_of_service }) => {
    ///         println!("Signing up! You must agree to these TOS: {:?}", terms_of_service);
    ///         client.sign_up(&token, "My first name", "(optional last name)").await?
    ///     }
    ///     Err(err) => {
    ///         println!("Something else went wrong... {}", err);
    ///         return Err(err.into());
    ///     },
    /// };
    ///
    /// println!("Signed up as {}!", user.first_name.unwrap());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn sign_up(
        &mut self,
        token: &LoginToken,
        first_name: &str,
        last_name: &str,
    ) -> Result<tl::types::User, InvocationError> {
        // TODO accept tos?
        match self
            .invoke(&tl::functions::auth::SignUp {
                phone_number: token.phone.clone(),
                phone_code_hash: token.phone_code_hash.clone(),
                first_name: first_name.to_string(),
                last_name: last_name.to_string(),
            })
            .await
        {
            Ok(tl::enums::auth::Authorization::Authorization(x)) => {
                // Safe to unwrap, Telegram won't send `UserEmpty` here.
                Ok(x.user.try_into().unwrap())
            }
            Ok(tl::enums::auth::Authorization::SignUpRequired(_)) => {
                panic!("API returned SignUpRequired even though we just invoked SignUp");
            }
            Err(error) => Err(error),
        }
    }

    /// Signs out of the account authorized by this client's session.
    ///
    /// If the client was not logged in, this method returns false.
    ///
    /// The client is not disconnected after signing out.
    ///
    /// Note that after using this method you will have to sign in again. If all you want to do
    /// is disconnect, simply [`drop`] the [`Client`] instance or use the
    /// [`ClientHandle::disconnect`] method.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(mut client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// if client.sign_out().await? {
    ///     println!("Signed out successfully!");
    /// } else {
    ///     println!("No user was signed in, so nothing has changed...");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// [`ClientHandle::disconnect`]: crate::ClientHandle::disconnect
    pub async fn sign_out(&mut self) -> Result<bool, InvocationError> {
        self.invoke(&tl::functions::auth::LogOut {}).await
    }
}
