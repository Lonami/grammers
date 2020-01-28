use grammers_mtsender::MTSender;
use grammers_tl_types::{self as tl, RPC};
use std::io::Result;

// TODO handle PhoneMigrate
const DC_4_ADDRESS: &'static str = "149.154.167.91:443";

/// A client capable of connecting to Telegram and invoking requests.
pub struct Client {
    sender: MTSender,
}

/// Implementors of this trait have a way to turn themselves into the
/// desired input parameter.
pub trait IntoInput<T> {
    fn convert(&self, client: &mut Client) -> T;
}

impl IntoInput<tl::enums::InputPeer> for &str {
    fn convert(&self, client: &mut Client) -> tl::enums::InputPeer {
        unimplemented!();
    }
}

impl Client {
    /// Returns a new client instance connected to Telegram and returns it.
    pub fn new() -> Result<Self> {
        Ok(Client {
            sender: MTSender::connect(DC_4_ADDRESS)?,
        })
    }

    /// Signs in to the bot account associated with this token.
    pub fn bot_sign_in(&mut self, token: &str, api_id: i32, api_hash: &str) -> Result<()> {
        self.invoke(&tl::functions::auth::ImportBotAuthorization {
            flags: 0,
            api_id,
            api_hash: api_hash.to_string(),
            bot_auth_token: token.to_string(),
        })?;

        Ok(())
    }

    /// Resolves a username into the user that owns it, if any.
    pub fn resolve_username(&mut self, username: &str) -> Result<tl::types::User> {
        unimplemented!();
    }

    /// Sends a text message to the desired chat.
    pub fn send_message<C: IntoInput<tl::enums::InputPeer>>(
        &mut self,
        chat: C,
        message: &str,
    ) -> Result<()> {
        unimplemented!();
    }

    /// Invokes a raw request, and returns its result.
    pub fn invoke<R: RPC>(&mut self, request: &R) -> Result<R::Return> {
        self.sender.invoke(request)
    }
}
