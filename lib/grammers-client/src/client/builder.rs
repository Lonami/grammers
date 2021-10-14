use crate::{Config, InitParams, SignInError};

use super::Client;
use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_session::Session;
use log;
use std::fmt;
use std::path::Path;
use tokio::io::{self, AsyncBufReadExt, AsyncWriteExt, BufReader};

impl Client {
    /// Create new `ClientBuilder` for more user firendly client setup.
    ///
    /// # Universal example:
    /// ```ignore
    /// let (client, authorized) = Client::builder(API_ID, &API_HASH)
    ///     .interactive(true)
    ///     .session_file("session.session")?
    ///     .connect()
    ///     .await?;
    /// ```
    ///
    /// # Login with bot token:
    /// ```ignore
    /// let (client, authorized) = Client::builder(API_ID, &API_HASH)
    ///     .bot_token("some:bot:token")
    ///     .session_file("session.session")?
    ///     .connect()
    ///     .await?;
    /// ```
    ///
    /// # Login to user account:
    /// NOTE: For user accounts `interactive(true)` is required, because of code prompt.
    /// Otherwise unauthorized client will be returned
    /// ```ignore
    /// let (client, authorized) = Client::builder(API_ID, &API_HASH)
    ///     .interactive(true)
    ///     .session_file("session.session")?
    ///     .phone(Some("123456789"))
    ///     .password(None)
    ///     .show_password_hint(true)
    ///     .connect()
    ///     .await?;
    /// ```
    pub fn builder(api_id: i32, api_hash: &str) -> ClientBuilder {
        ClientBuilder::new(api_id, api_hash)
    }
}

pub struct ClientBuilder {
    api_id: i32,
    api_hash: String,
    bot_token: Option<String>,
    session: Option<Session>,
    phone: Option<String>,
    params: InitParams,
    interactive: bool,
    password_hint: bool,
    password: Option<String>,
}

impl ClientBuilder {
    /// Create new instance of `ClientBuilder` for more user firendly client setup.
    ///
    /// # Universal example:
    /// ```ignore
    /// let (client, authorized) = ClientBuilder::new(API_ID, &API_HASH)
    ///     .interactive(true)
    ///     .session_file("session.session")?
    ///     .connect()
    ///     .await?;
    /// ```
    ///
    /// # Login with bot token:
    /// ```ignore
    /// let (client, authorized) = ClientBuilder::new(API_ID, &API_HASH)
    ///     .bot_token("some:bot:token")
    ///     .session_file("session.session")?
    ///     .connect()
    ///     .await?;
    /// ```
    ///
    /// # Login to user account:
    /// NOTE: For user accounts `interactive(true)` is required, because of code prompt.
    /// Otherwise unauthorized client will be returned
    /// ```ignore
    /// let (client, authorized) = ClientBuilder::new(API_ID, &API_HASH)
    ///     .interactive(true)
    ///     .session_file("session.session")?
    ///     .phone(Some("123456789"))
    ///     .password(None)
    ///     .show_password_hint(true)
    ///     .connect()
    ///     .await?;
    /// ```
    pub fn new(api_id: i32, api_hash: &str) -> ClientBuilder {
        ClientBuilder {
            api_id,
            api_hash: api_hash.to_string(),
            bot_token: None,
            session: None,
            phone: None,
            params: InitParams::default(),
            interactive: false,
            password_hint: false,
            password: None,
        }
    }

    /// Set session parameter for client
    ///
    /// # Example
    /// ```ignore
    /// use grammers_session::Session;
    /// ClientBuilder::new(API_ID, API_HASH)
    ///     .session(Session::load_file_or_create("session.session")?)
    /// ```
    pub fn session(mut self, session: Session) -> Self {
        self.session = Some(session);
        self
    }

    /// Shorthand for setting the session client parameter from path
    /// Equivalent to: `.session(Session::load_file_or_create("session.session")?)`
    pub fn session_file(mut self, path: impl AsRef<Path>) -> Result<Self, ClientBuilderError> {
        self.session = Some(Session::load_file_or_create(path)?);
        Ok(self)
    }

    /// Login using bot token
    pub fn bot_token(mut self, token: &str) -> Self {
        self.bot_token = Some(token.to_string());
        self
    }

    /// Login using phone number
    pub fn phone(mut self, phone: &str) -> Self {
        self.phone = Some(phone.to_string());
        self
    }

    /// Set new client `InitParams`
    pub fn params(mut self, params: InitParams) -> Self {
        self.params = params;
        self
    }

    /// Enable interactive mode (prompt in terminal for missing fields)
    pub fn interactive(mut self, enabled: bool) -> Self {
        self.interactive = enabled;
        self
    }

    /// Wether to display password hint in interactive mode
    pub fn show_password_hint(mut self, show: bool) -> Self {
        self.password_hint = show;
        self
    }

    /// Set the password for logging in
    pub fn password(mut self, password: Option<&str>) -> Self {
        self.password = password.map(String::from);
        self
    }

    /// Prompt for a question in cli
    async fn prompt(question: &str) -> Result<String, ClientBuilderError> {
        let mut stdout = io::stdout();
        stdout.write_all(question.as_bytes()).await?;
        stdout.flush().await?;

        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut output = String::new();
        reader.read_line(&mut output).await?;
        Ok(output.trim().to_string())
    }

    /// Recover terminal from using hidden/conceal mode
    async fn restore_conceal() -> Result<(), ClientBuilderError> {
        let mut stdout = io::stdout();
        stdout.write_all(&[27, 91, 48, 109]).await?;
        stdout.flush().await?;
        Ok(())
    }

    /// Create client and try to log in.
    /// Returns client instance and wether the client is authorized.
    /// Should return unauthorized only if interactive is disabled and logging in into a user account
    pub async fn connect(mut self) -> Result<(Client, bool), ClientBuilderError> {
        // Get session and create client
        let session = match self.session {
            Some(session) => {
                self.session = None;
                session
            }
            None => {
                log::warn!("No session specified! Using default");
                Session::new()
            }
        };
        let mut client = Client::connect(Config {
            session,
            api_id: self.api_id,
            api_hash: self.api_hash.clone(),
            params: self.params.clone(),
        })
        .await?;

        if client.is_authorized().await? {
            return Ok((client, true));
        }

        // Missing bot token and phone number
        if self.bot_token.is_none() && self.phone.is_none() {
            if !self.interactive {
                return Err(ClientBuilderError::MissingParameters(
                    "bot_token or phone number",
                ));
            }
            let answer = Self::prompt("Enter phone number or bot token: ").await?;
            if answer.contains(":") {
                self.bot_token = Some(answer);
            } else {
                self.phone = Some(answer);
            }
        }
        // Login using bot token
        if let Some(token) = self.bot_token {
            client
                .bot_sign_in(&token, self.api_id, &self.api_hash)
                .await?;
            return Ok((client, true));
        }
        // Unauthorized (can't prompt for code)
        if !self.interactive {
            return Ok((client, false));
        }
        // Interactive user login
        let token = client
            .request_login_code(self.phone.as_ref().unwrap(), self.api_id, &self.api_hash)
            .await?;
        let code = Self::prompt("Enter the code you received: ").await?;
        match client.sign_in(&token, &code).await {
            Ok(_) => Ok((client, true)),
            Err(SignInError::PasswordRequired(password_token)) => {
                // Try saved password
                if let Some(password) = &self.password {
                    match client
                        .check_password(password_token.clone(), password)
                        .await
                    {
                        Err(SignInError::InvalidPassword) => {
                            log::warn!("Invalid password!");
                        }
                        r => {
                            r?;
                            return Ok((client, true));
                        }
                    };
                }
                // `\x1B[8;32m` = conceal/hidden
                let prompt = if self.password_hint && password_token.hint().is_some() {
                    format!(
                        "Enter your password (hint: {}) (hidden): \x1B[8;32m",
                        password_token.hint().unwrap()
                    )
                } else {
                    "Enter your password (hidden): \x1B[8;32m".to_string()
                };
                let answer = match Self::prompt(&prompt).await {
                    Ok(answer) => answer,
                    Err(e) => {
                        Self::restore_conceal().await?;
                        return Err(e.into());
                    }
                };
                Self::restore_conceal().await?;
                client.check_password(password_token, &answer).await?;
                Ok((client, true))
            }
            Err(e) => Err(e.into()),
        }
    }
}

#[derive(Debug)]
pub enum ClientBuilderError {
    IO(std::io::Error),
    AuthorizationError(AuthorizationError),
    MissingParameters(&'static str),
    SignInError(SignInError),
    InvocationError(InvocationError),
}

impl fmt::Display for ClientBuilderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientBuilderError::IO(e) => write!(f, "IO error: {}", e),
            ClientBuilderError::AuthorizationError(e) => write!(f, "Authorization error: {}", e),
            ClientBuilderError::MissingParameters(e) => write!(f, "Missing parameters: {}", e),
            ClientBuilderError::SignInError(e) => write!(f, "Sign in error: {}", e),
            ClientBuilderError::InvocationError(e) => write!(f, "Other error: {}", e),
        }
    }
}

impl From<std::io::Error> for ClientBuilderError {
    fn from(e: std::io::Error) -> Self {
        ClientBuilderError::IO(e)
    }
}

impl From<AuthorizationError> for ClientBuilderError {
    fn from(e: AuthorizationError) -> Self {
        ClientBuilderError::AuthorizationError(e)
    }
}

impl From<SignInError> for ClientBuilderError {
    fn from(e: SignInError) -> Self {
        ClientBuilderError::SignInError(e)
    }
}

impl From<InvocationError> for ClientBuilderError {
    fn from(e: InvocationError) -> Self {
        ClientBuilderError::InvocationError(e)
    }
}

impl std::error::Error for ClientBuilderError {}
