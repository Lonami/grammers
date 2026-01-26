// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use grammers_crypto::two_factor_auth::{calculate_2fa, check_p_and_g};
use grammers_mtsender::InvocationError;
use grammers_session::types::{PeerInfo, UpdateState, UpdatesState};
use grammers_tl_types as tl;

use base64::Engine;

use super::Client;
use crate::peer::User;
use crate::utils;

/// The error type which is returned when signing in fails.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum SignInError {
    /// Sign-up with an official client is required.
    ///
    /// Third-party applications, such as those developed with *grammers*,
    /// cannot be used to register new accounts. For more details, see this
    /// [comment regarding third-party app sign-ups](https://bugs.telegram.org/c/25410/1):
    /// > \[…] if a user doesn’t have a Telegram account yet,
    /// > they will need to create one first using an official mobile Telegram app.
    SignUpRequired,
    /// The account has 2FA enabled, and the password is required.
    PasswordRequired(PasswordToken),
    /// The code used to complete login was not valid.
    InvalidCode,
    /// The 2FA password used to complete login was not valid.
    InvalidPassword(PasswordToken),
    /// A generic invocation error occured.
    Other(InvocationError),
}

impl fmt::Display for SignInError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use SignInError::*;
        match self {
            SignUpRequired => write!(f, "sign in error: sign up with official client required"),
            PasswordRequired(_password) => write!(f, "2fa password required"),
            InvalidCode => write!(f, "sign in error: invalid code"),
            InvalidPassword(_password) => write!(f, "invalid password"),
            Other(e) => write!(f, "sign in error: {e}"),
        }
    }
}

impl std::error::Error for SignInError {}

/// Login token needed to continue the login process after sending the code.
pub struct LoginToken {
    pub(crate) phone: String,
    pub(crate) phone_code_hash: String,
}

/// Password token needed to complete a 2FA login.
#[derive(Debug)]
pub struct PasswordToken {
    pub(crate) password: tl::types::account::Password,
}

impl PasswordToken {
    pub fn new(password: tl::types::account::Password) -> Self {
        PasswordToken { password }
    }

    pub fn hint(&self) -> Option<&str> {
        self.password.hint.as_deref()
    }
}

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
    /// # async fn f(client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// if client.is_authorized().await? {
    ///     println!("Client already authorized and ready to use!");
    /// } else {
    ///     println!("Client is not authorized, you will need to sign_in!");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn is_authorized(&self) -> Result<bool, InvocationError> {
        match self.invoke(&tl::functions::updates::GetState {}).await {
            Ok(_) => Ok(true),
            Err(InvocationError::Rpc(e)) if e.code == 401 => Ok(false),
            Err(err) => Err(err),
        }
    }

    async fn complete_login(
        &self,
        auth: tl::types::auth::Authorization,
    ) -> Result<User, InvocationError> {
        // In the extremely rare case where `Err` happens, there's not much we can do.
        // `message_box` will try to correct its state as updates arrive.
        let update_state = self.invoke(&tl::functions::updates::GetState {}).await.ok();

        let user = User::from_raw(self, auth.user);
        let auth = user.to_ref().await.unwrap().auth;

        self.0
            .session
            .cache_peer(&PeerInfo::User {
                id: user.id().bare_id(),
                auth: Some(auth),
                bot: Some(user.is_bot()),
                is_self: Some(true),
            })
            .await;
        if let Some(tl::enums::updates::State::State(state)) = update_state {
            self.0
                .session
                .set_update_state(UpdateState::All(UpdatesState {
                    pts: state.pts,
                    qts: state.qts,
                    date: state.date,
                    seq: state.seq,
                    channels: Vec::new(),
                }))
                .await;
        }

        Ok(user)
    }

    /// Signs in to the bot account associated with this token.
    ///
    /// This is the method you need to call to use the client under a bot account.
    ///
    /// It is recommended to save the session on successful login. Some session storages will do this
    /// automatically. If saving fails, it is recommended to [`Client::sign_out`]. If the session is never
    /// saved post-login, then the authorization will be "lost" in the list of logged-in clients, since it
    /// is unaccessible.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // Note: these values are obviously fake.
    /// //       Obtain your own with the developer's phone at https://my.telegram.org.
    /// const API_HASH: &str = "514727c32270b9eb8cc16daf17e21e57";
    /// //       Obtain your own by talking to @BotFather via a Telegram app.
    /// const TOKEN: &str = "776609994:AAFXAy5-PawQlnYywUlZ_b_GOXgarR3ah_yq";
    ///
    /// let user = match client.bot_sign_in(TOKEN, API_HASH).await {
    ///     Ok(user) => user,
    ///     Err(err) => {
    ///         println!("Failed to sign in as a bot :(\n{}", err);
    ///         return Err(err.into());
    ///     }
    /// };
    ///
    /// if let Some(first_name) = user.first_name() {
    ///     println!("Signed in as {}!", first_name);
    /// } else {
    ///     println!("Signed in!");
    /// }
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub async fn bot_sign_in(&self, token: &str, api_hash: &str) -> Result<User, InvocationError> {
        let request = tl::functions::auth::ImportBotAuthorization {
            flags: 0,
            api_id: self.0.api_id,
            api_hash: api_hash.to_string(),
            bot_auth_token: token.to_string(),
        };

        let result = match self.invoke(&request).await {
            Ok(x) => x,
            Err(InvocationError::Rpc(err)) if err.code == 303 => {
                let old_dc_id = self.0.session.home_dc_id();
                let new_dc_id = err.value.unwrap() as i32;
                // Disconnect from current DC to cull the now-unused connection.
                // This also gives a chance for the new home DC to export its authorization
                // if there's a need to connect back to the old DC after having logged in.
                self.0.handle.disconnect_from_dc(old_dc_id);
                self.0.session.set_home_dc_id(new_dc_id).await;
                self.invoke(&request).await?
            }
            Err(e) => return Err(e.into()),
        };

        match result {
            tl::enums::auth::Authorization::Authorization(x) => {
                self.complete_login(x).await.map_err(Into::into)
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
    /// # async fn f(client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// // Note: these values are obviously fake.
    /// //       Obtain your own with the developer's phone at https://my.telegram.org.
    /// const API_HASH: &str = "514727c32270b9eb8cc16daf17e21e57";
    /// //       The phone used here does NOT need to be the same as the one used by the developer
    /// //       to obtain the API ID and hash.
    /// const PHONE: &str = "+1 415 555 0132";
    ///
    /// if !client.is_authorized().await? {
    ///     // We're not logged in, so request the login code.
    ///     client.request_login_code(PHONE, API_HASH).await?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn request_login_code(
        &self,
        phone: &str,
        api_hash: &str,
    ) -> Result<LoginToken, InvocationError> {
        let request = tl::functions::auth::SendCode {
            phone_number: phone.to_string(),
            api_id: self.0.api_id,
            api_hash: api_hash.to_string(),
            settings: tl::types::CodeSettings {
                allow_flashcall: false,
                current_number: false,
                allow_app_hash: false,
                allow_missed_call: false,
                allow_firebase: false,
                logout_tokens: None,
                token: None,
                app_sandbox: None,
                unknown_number: false,
            }
            .into(),
        };

        use tl::enums::auth::SentCode as SC;

        let sent_code: tl::types::auth::SentCode = match self.invoke(&request).await {
            Ok(x) => match x {
                SC::Code(code) => code,
                SC::Success(_) => panic!("should not have logged in yet"),
                SC::PaymentRequired(_) => unimplemented!(),
            },
            Err(InvocationError::Rpc(err)) if err.code == 303 => {
                let old_dc_id = self.0.session.home_dc_id();
                let new_dc_id = err.value.unwrap() as i32;
                // Disconnect from current DC to cull the now-unused connection.
                // This also gives a chance for the new home DC to export its authorization
                // if there's a need to connect back to the old DC after having logged in.
                self.0.handle.disconnect_from_dc(old_dc_id);
                self.0.session.set_home_dc_id(new_dc_id).await;
                match self.invoke(&request).await? {
                    SC::Code(code) => code,
                    SC::Success(_) => panic!("should not have logged in yet"),
                    SC::PaymentRequired(_) => unimplemented!(),
                }
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
    /// It is recommended to save the session on successful login. Some session storages will do this
    /// automatically. If saving fails, it is recommended to [`Client::sign_out`]. If the session is never
    /// saved post-login, then the authorization will be "lost" in the list of logged-in clients, since it
    /// is unaccessible.
    ///
    /// # Examples
    ///
    /// ```
    /// # use grammers_client::SignInError;
    ///
    ///  async fn f(client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// # const API_HASH: &str = "";
    /// # const PHONE: &str = "";
    /// fn ask_code_to_user() -> String {
    ///     unimplemented!()
    /// }
    ///
    /// let token = client.request_login_code(PHONE, API_HASH).await?;
    /// let code = ask_code_to_user();
    ///
    /// let user = match client.sign_in(&token, &code).await {
    ///     Ok(user) => user,
    ///     Err(SignInError::PasswordRequired(_token)) => panic!("Please provide a password"),
    ///     Err(SignInError::SignUpRequired) => panic!("Sign up required"),
    ///     Err(err) => {
    ///         println!("Failed to sign in as a user :(\n{}", err);
    ///         return Err(err.into());
    ///     }
    /// };
    ///
    /// if let Some(first_name) = user.first_name() {
    ///     println!("Signed in as {}!", first_name);
    /// } else {
    ///   println!("Signed in!");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn sign_in(&self, token: &LoginToken, code: &str) -> Result<User, SignInError> {
        match self
            .invoke(&tl::functions::auth::SignIn {
                phone_number: token.phone.clone(),
                phone_code_hash: token.phone_code_hash.clone(),
                phone_code: Some(code.to_string()),
                email_verification: None,
            })
            .await
        {
            Ok(tl::enums::auth::Authorization::Authorization(x)) => {
                self.complete_login(x).await.map_err(SignInError::Other)
            }
            Ok(tl::enums::auth::Authorization::SignUpRequired(_)) => {
                Err(SignInError::SignUpRequired)
            }
            Err(err) if err.is("SESSION_PASSWORD_NEEDED") => {
                let password_token = self.get_password_information().await;
                match password_token {
                    Ok(token) => Err(SignInError::PasswordRequired(token)),
                    Err(e) => Err(SignInError::Other(e)),
                }
            }
            Err(err) if err.is("PHONE_CODE_*") => Err(SignInError::InvalidCode),
            Err(error) => Err(SignInError::Other(error)),
        }
    }

    /// Extract information needed for the two-factor authentication
    /// It's called automatically when we get SESSION_PASSWORD_NEEDED error during sign in.
    async fn get_password_information(&self) -> Result<PasswordToken, InvocationError> {
        let request = tl::functions::account::GetPassword {};

        let password: tl::types::account::Password = self.invoke(&request).await?.into();

        Ok(PasswordToken::new(password))
    }

    /// Sign in using two-factor authentication (user password).
    ///
    /// [`PasswordToken`] can be obtained from [`SignInError::PasswordRequired`] error after the
    /// [`Client::sign_in`] method fails.
    ///
    /// # Examples
    ///
    /// ```
    /// use grammers_client::SignInError;
    ///
    /// # async fn f(client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// # const API_HASH: &str = "";
    /// # const PHONE: &str = "";
    /// fn get_user_password(hint: &str) -> Vec<u8> {
    ///     unimplemented!()
    /// }
    ///
    /// # let token = client.request_login_code(PHONE, API_HASH).await?;
    /// # let code = "";
    ///
    /// // ... enter phone number, request login code ...
    ///
    /// let user = match client.sign_in(&token, &code).await {
    ///     Err(SignInError::PasswordRequired(password_token) ) => {
    ///         let mut password = get_user_password(password_token.hint().unwrap());
    ///
    ///         client
    ///             .check_password(password_token, password)
    ///             .await.unwrap()
    ///     }
    ///     Ok(user) => user,
    ///     Ok(_) => panic!("Sign in required"),
    ///     Err(err) => {
    ///         panic!("Failed to sign in as a user :(\n{err}");
    ///     }
    /// };
    /// # Ok(())
    /// # }
    /// ```
    pub async fn check_password(
        &self,
        password_token: PasswordToken,
        password: impl AsRef<[u8]>,
    ) -> Result<User, SignInError> {
        let mut password_info = password_token.password;
        let current_algo = password_info.current_algo.clone().unwrap();
        let mut params = utils::extract_password_parameters(&current_algo);

        // Telegram sent us incorrect parameters, trying to get them again
        if !check_p_and_g(params.2, params.3) {
            password_info = self
                .get_password_information()
                .await
                .map_err(SignInError::Other)?
                .password;
            params =
                utils::extract_password_parameters(password_info.current_algo.as_ref().unwrap());
            if !check_p_and_g(params.2, params.3) {
                panic!("Failed to get correct password information from Telegram")
            }
        }

        let (salt1, salt2, p, g) = params;

        let g_b = password_info.srp_b.clone().unwrap();
        let a = password_info.secure_random.clone();

        let (m1, g_a) = calculate_2fa(salt1, salt2, p, g, g_b, a, password);

        let check_password = tl::functions::auth::CheckPassword {
            password: tl::enums::InputCheckPasswordSrp::Srp(tl::types::InputCheckPasswordSrp {
                srp_id: password_info.srp_id.clone().unwrap(),
                a: g_a.to_vec(),
                m1: m1.to_vec(),
            }),
        };

        match self.invoke(&check_password).await {
            Ok(tl::enums::auth::Authorization::Authorization(x)) => {
                self.complete_login(x).await.map_err(SignInError::Other)
            }
            Ok(tl::enums::auth::Authorization::SignUpRequired(_x)) => panic!("Unexpected result"),
            Err(err) if err.is("PASSWORD_HASH_INVALID") => {
                Err(SignInError::InvalidPassword(PasswordToken {
                    password: password_info,
                }))
            }
            Err(error) => Err(SignInError::Other(error)),
        }
    }

    /// Signs out of the account authorized by this client's session.
    ///
    /// If the client was not logged in, this method returns false.
    ///
    /// The client is not disconnected after signing out.
    ///
    /// Note that after using this method you will have to sign in again. If all you want to do
    /// is disconnect, simply [`drop`] the [`Client`] instance.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// if client.sign_out().await.is_ok() {
    ///     println!("Signed out successfully!");
    /// } else {
    ///     println!("No user was signed in, so nothing has changed...");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn sign_out(&self) -> Result<tl::enums::auth::LoggedOut, InvocationError> {
        self.invoke(&tl::functions::auth::LogOut {}).await
    }

    /// Signals all clients sharing the same sender pool to disconnect.
    pub fn disconnect(&self) {
        self.0.handle.quit();
    }
}

// QR Login functionality

/// Status of the QR login process
#[derive(Debug, Clone, PartialEq)]
pub enum QrLoginStatus {
    Idle,
    Waiting,
    Expired,
    Success,
    PasswordRequired(Option<String>), // 2FA required, with optional hint
    Error(String),
}

/// Information about the current QR login state
pub struct QrLoginInfo {
    pub qr_url: String,
    pub expires_unix: u64,
    pub expires_in_seconds: i64,
    pub status: QrLoginStatus,
}

impl Client {
    /// Export login token for QR code login
    pub async fn export_login_token(
        &self,
        api_id: i32,
        api_hash: &str,
    ) -> Result<tl::enums::auth::LoginToken, InvocationError> {
        let request = tl::functions::auth::ExportLoginToken {
            api_id,
            api_hash: api_hash.to_string(),
            except_ids: vec![],
        };

        self.invoke(&request).await
    }

    /// Import login token for DC migration
    /// Import login token for DC migration - switches home DC before importing
    pub async fn import_login_token(
        &self,
        token: Vec<u8>,
        dc_id: i32,
    ) -> Result<tl::enums::auth::LoginToken, InvocationError> {
        // Handle DC migration by switching home DC
        self.handle_qr_login_migration(dc_id).await?;

        let request = tl::functions::auth::ImportLoginToken { token };

        // Import on the new home DC (after migration)
        match self.invoke(&request).await {
            Ok(result) => Ok(result),
            Err(InvocationError::Rpc(ref err)) if err.name == "SESSION_PASSWORD_NEEDED" => {
                // If password is needed, we return an error that can be handled by the caller
                Err(InvocationError::Rpc(err.clone()))
            }
            Err(e) => Err(e),
        }
    }

    /// Handle DC migration during QR login by switching home DC
    pub(crate) async fn handle_qr_login_migration(
        &self,
        new_dc_id: i32,
    ) -> Result<(), InvocationError> {
        let old_dc = self.0.session.home_dc_id();
        self.0.handle.disconnect_from_dc(old_dc);
        self.0.session.set_home_dc_id(new_dc_id).await;
        Ok(())
    }

    /// Finalize QR login by completing the authorization process
    pub async fn finalize_qr_login(
        &self,
        auth: tl::types::auth::Authorization,
    ) -> Result<User, InvocationError> {
        self.complete_login(auth).await
    }

    /// Get password information for 2FA authentication
    pub async fn qr_get_password_token(&self) -> Result<PasswordToken, InvocationError> {
        self.get_password_information().await
    }

    /// Fetches 2FA password token (hint, SRP params) so the app can prompt for password.
    pub async fn get_password_token(&self) -> Result<PasswordToken, InvocationError> {
        self.get_password_information().await
    }

    /// Convert raw token bytes to base64url encoded string
    fn encode_token_to_base64url(&self, token_bytes: &[u8]) -> String {
        // Use URL-safe base64 encoding without padding
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(token_bytes)
    }

    /// Generate QR code URL from token bytes
    fn generate_qr_url(&self, token_bytes: &[u8]) -> String {
        let encoded_token = self.encode_token_to_base64url(token_bytes);
        format!("tg://login?token={}", encoded_token)
    }

    /// Start QR login process and return initial QR information
    pub async fn start_qr_login(
        &self,
        api_id: i32,
        api_hash: &str,
    ) -> Result<QrLoginInfo, InvocationError> {
        let login_token = match self.export_login_token(api_id, api_hash).await {
            Ok(token) => token,
            Err(InvocationError::Rpc(err)) if err.name == "SESSION_PASSWORD_NEEDED" => {
                // Return PasswordRequired status instead of failing
                return Ok(QrLoginInfo {
                    qr_url: "".to_string(),
                    expires_unix: 0,
                    expires_in_seconds: 0,
                    status: QrLoginStatus::PasswordRequired(None), // We'll get the hint separately
                });
            }
            Err(e) => return Err(e),
        };

        match login_token {
            tl::enums::auth::LoginToken::Token(token) => {
                let qr_url = self.generate_qr_url(&token.token);
                let expires_unix = token.expires as u64;
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let expires_in_seconds = token.expires as i64 - current_time as i64;

                Ok(QrLoginInfo {
                    qr_url,
                    expires_unix,
                    expires_in_seconds,
                    status: QrLoginStatus::Waiting,
                })
            }
            tl::enums::auth::LoginToken::MigrateTo(migrate_to) => {
                // Handle migration by switching home DC and importing the token on the new DC
                self.handle_qr_login_migration(migrate_to.dc_id).await?;

                let import_request = tl::functions::auth::ImportLoginToken {
                    token: migrate_to.token,
                };

                let import_result = match self.invoke(&import_request).await {
                    Ok(result) => result,
                    Err(InvocationError::Rpc(err)) if err.name == "SESSION_PASSWORD_NEEDED" => {
                        // Return PasswordRequired status instead of failing
                        return Ok(QrLoginInfo {
                            qr_url: "".to_string(),
                            expires_unix: 0,
                            expires_in_seconds: 0,
                            status: QrLoginStatus::PasswordRequired(None), // We'll get the hint separately
                        });
                    }
                    Err(e) => return Err(e),
                };
                match import_result {
                    tl::enums::auth::LoginToken::Success(success) => {
                        // Successfully logged in after migration
                        match success.authorization {
                            tl::enums::auth::Authorization::Authorization(auth) => {
                                // Complete the login and establish session on the new DC
                                let _user = self.complete_login(auth).await?;

                                // Ensure session is properly established on the new DC
                                // This ensures is_authorized() will work correctly
                                Ok(QrLoginInfo {
                                    qr_url: "".to_string(),
                                    expires_unix: 0,
                                    expires_in_seconds: 0,
                                    status: QrLoginStatus::Success,
                                })
                            }
                            tl::enums::auth::Authorization::SignUpRequired(_) => {
                                Err(InvocationError::Rpc(grammers_mtsender::RpcError {
                                    code: 400,
                                    name: "SIGN_UP_REQUIRED".to_string(),
                                    value: None,
                                    caused_by: None,
                                }))
                            }
                        }
                    }
                    tl::enums::auth::LoginToken::Token(token) => {
                        // Got a new token after migration
                        let qr_url = self.generate_qr_url(&token.token);
                        let expires_unix = token.expires as u64;
                        let current_time = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let expires_in_seconds = token.expires as i64 - current_time as i64;

                        Ok(QrLoginInfo {
                            qr_url,
                            expires_unix,
                            expires_in_seconds,
                            status: QrLoginStatus::Waiting,
                        })
                    }
                    tl::enums::auth::LoginToken::MigrateTo(_) => {
                        // Unexpected double migration
                        Err(InvocationError::Rpc(grammers_mtsender::RpcError {
                            code: 400,
                            name: "UNEXPECTED_MIGRATION".to_string(),
                            value: None,
                            caused_by: None,
                        }))
                    }
                }
            }
            tl::enums::auth::LoginToken::Success(success) => {
                // Already logged in
                match success.authorization {
                    tl::enums::auth::Authorization::Authorization(auth) => self
                        .complete_login(auth)
                        .await
                        .map(|_| QrLoginInfo {
                            qr_url: "".to_string(),
                            expires_unix: 0,
                            expires_in_seconds: 0,
                            status: QrLoginStatus::Success,
                        })
                        .map_err(|e| e),
                    tl::enums::auth::Authorization::SignUpRequired(_) => {
                        Err(InvocationError::Rpc(grammers_mtsender::RpcError {
                            code: 400,
                            name: "SIGN_UP_REQUIRED".to_string(),
                            value: None,
                            caused_by: None,
                        }))
                    }
                }
            }
        }
    }

    /// Perform continuous QR login with automatic token refresh
    pub async fn start_continuous_qr_login(
        &self,
        api_id: i32,
        api_hash: String,
    ) -> Result<
        (
            tokio::sync::watch::Sender<QrLoginInfo>,
            tokio::sync::broadcast::Sender<()>,
            tokio::task::JoinHandle<Result<User, InvocationError>>,
        ),
        InvocationError,
    > {
        // Create a QR info channel and a cancellation channel
        let (qr_info_tx, _) = tokio::sync::watch::channel(QrLoginInfo {
            qr_url: "".to_string(),
            expires_unix: 0,
            expires_in_seconds: 0,
            status: QrLoginStatus::Idle,
        });
        let (cancellation_tx, _) = tokio::sync::broadcast::channel(1);

        // Create a clone of the client to move into the task
        let client_clone = self.clone();
        let qr_info_tx_clone = qr_info_tx.clone();
        let cancellation_tx_clone = cancellation_tx.clone();

        let join_handle = tokio::spawn(async move {
            let mut _current_expiration = 0;
            let mut _current_token = Vec::<u8>::new();
            // Used to track current token and expiration during QR refresh

            loop {
                // Check for cancellation
                let mut cancel_subscriber = cancellation_tx_clone.subscribe();
                let cancellation_result = tokio::select! {
                    _ = cancel_subscriber.recv() => {
                        return Err(InvocationError::Rpc(grammers_mtsender::RpcError {
                            code: 406,
                            name: "LOGIN_CANCELLED".to_string(),
                            value: None,
                            caused_by: None,
                        }));
                    },
                    result = client_clone.export_login_token(api_id, &api_hash) => {
                        match result {
                            Ok(token) => Ok(token),
                            Err(InvocationError::Rpc(err)) if err.name == "SESSION_PASSWORD_NEEDED" => {
                                // Return error indicating password is required
                                Err(InvocationError::Rpc(err))
                            },
                            Err(e) => Err(e),
                        }
                    },
                };

                let login_token = match cancellation_result {
                    Ok(token) => token,
                    Err(InvocationError::Rpc(err)) if err.name == "SESSION_PASSWORD_NEEDED" => {
                        // Propagate the password needed error
                        return Err(InvocationError::Rpc(err));
                    }
                    Err(e) => return Err(e),
                };

                match login_token {
                    tl::enums::auth::LoginToken::Token(token) => {
                        _current_token = token.token.clone();
                        _current_expiration = token.expires as u64;

                        // Send QR info update
                        let qr_url = client_clone.generate_qr_url(&token.token);
                        let expires_unix = token.expires as u64;
                        let current_time = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let expires_in_seconds = token.expires as i64 - current_time as i64;

                        let qr_info = QrLoginInfo {
                            qr_url,
                            expires_unix,
                            expires_in_seconds,
                            status: QrLoginStatus::Waiting,
                        };

                        // Send the QR info update (ignore if no receivers)
                        let _ = qr_info_tx_clone.send(qr_info);

                        // Sleep until 5 seconds before expiration to refresh
                        let time_until_expiry = if _current_expiration > current_time {
                            _current_expiration - current_time
                        } else {
                            0
                        };

                        // Sleep for 1 second or until close to expiry
                        let sleep_duration = std::cmp::min(time_until_expiry, 5) as u64;
                        if sleep_duration > 0 {
                            let sleep_future =
                                tokio::time::sleep(Duration::from_secs(sleep_duration));
                            let mut cancel_subscriber = cancellation_tx_clone.subscribe();
                            tokio::select! {
                                _ = sleep_future => {},
                                _ = cancel_subscriber.recv() => {
                                    return Err(InvocationError::Rpc(grammers_mtsender::RpcError {
                                        code: 406,
                                        name: "LOGIN_CANCELLED".to_string(),
                                        value: None,
                                        caused_by: None,
                                    }));
                                }
                            }
                        }
                    }
                    tl::enums::auth::LoginToken::MigrateTo(migrate_to) => {
                        // Handle migration by switching home DC and importing the token on the new DC
                        client_clone
                            .handle_qr_login_migration(migrate_to.dc_id)
                            .await?;

                        let import_request = tl::functions::auth::ImportLoginToken {
                            token: migrate_to.token,
                        };

                        match client_clone.invoke(&import_request).await {
                            Ok(import_result) => {
                                match import_result {
                                    tl::enums::auth::LoginToken::Success(success) => {
                                        match success.authorization {
                                            tl::enums::auth::Authorization::Authorization(auth) => {
                                                return client_clone.finalize_qr_login(auth).await;
                                            }
                                            tl::enums::auth::Authorization::SignUpRequired(_) => {
                                                return Err(InvocationError::Rpc(
                                                    grammers_mtsender::RpcError {
                                                        code: 400,
                                                        name: "SIGN_UP_REQUIRED".to_string(),
                                                        value: None,
                                                        caused_by: None,
                                                    },
                                                ));
                                            }
                                        }
                                    }
                                    tl::enums::auth::LoginToken::Token(token) => {
                                        // Got a new token, continue the loop
                                        _current_token = token.token.clone();
                                        _current_expiration = token.expires as u64;

                                        // Send QR info update
                                        let qr_url = client_clone.generate_qr_url(&token.token);
                                        let expires_unix = token.expires as u64;
                                        let current_time = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs();
                                        let expires_in_seconds =
                                            token.expires as i64 - current_time as i64;

                                        let qr_info = QrLoginInfo {
                                            qr_url,
                                            expires_unix,
                                            expires_in_seconds,
                                            status: QrLoginStatus::Waiting,
                                        };

                                        // Send the QR info update (ignore if no receivers)
                                        let _ = qr_info_tx_clone.send(qr_info);

                                        let current_time = SystemTime::now()
                                            .duration_since(UNIX_EPOCH)
                                            .unwrap()
                                            .as_secs();
                                        let time_until_expiry =
                                            if _current_expiration > current_time {
                                                _current_expiration - current_time
                                            } else {
                                                0
                                            };

                                        let sleep_duration =
                                            std::cmp::min(time_until_expiry, 5) as u64;
                                        if sleep_duration > 0 {
                                            let sleep_future = tokio::time::sleep(
                                                Duration::from_secs(sleep_duration),
                                            );
                                            let mut cancel_subscriber =
                                                cancellation_tx_clone.subscribe();
                                            tokio::select! {
                                                _ = sleep_future => {},
                                                _ = cancel_subscriber.recv() => {
                                                    return Err(InvocationError::Rpc(grammers_mtsender::RpcError {
                                                        code: 406,
                                                        name: "LOGIN_CANCELLED".to_string(),
                                                        value: None,
                                                        caused_by: None,
                                                    }));
                                                }
                                            }
                                        }
                                    }
                                    tl::enums::auth::LoginToken::MigrateTo(_) => {
                                        // This shouldn't happen, but continue anyway
                                        continue;
                                    }
                                }
                            }
                            Err(InvocationError::Rpc(err))
                                if err.name == "SESSION_PASSWORD_NEEDED" =>
                            {
                                // Return error indicating password is required
                                return Err(InvocationError::Rpc(err));
                            }
                            Err(e) => return Err(e),
                        }
                    }
                    tl::enums::auth::LoginToken::Success(success) => {
                        // Login successful
                        match success.authorization {
                            tl::enums::auth::Authorization::Authorization(auth) => {
                                return client_clone.complete_login(auth).await;
                            }
                            tl::enums::auth::Authorization::SignUpRequired(_) => {
                                return Err(InvocationError::Rpc(grammers_mtsender::RpcError {
                                    code: 400,
                                    name: "SIGN_UP_REQUIRED".to_string(),
                                    value: None,
                                    caused_by: None,
                                }));
                            }
                        }
                    }
                }
            }
        });

        Ok((qr_info_tx, cancellation_tx, join_handle))
    }
}
