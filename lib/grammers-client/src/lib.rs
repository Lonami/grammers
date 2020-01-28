use grammers_mtsender::MTSender;
use grammers_tl_types::{self as tl, Serializable, RPC};
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
    fn convert(&self, _client: &mut Client) -> tl::enums::InputPeer {
        unimplemented!();
    }
}

impl Client {
    /// Returns a new client instance connected to Telegram and returns it.
    pub fn new() -> Result<Self> {
        let mut sender = MTSender::connect(DC_4_ADDRESS)?;
        sender.generate_auth_key()?;
        let mut client = Client { sender };
        client.init_connection()?;
        Ok(client)
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
    pub fn resolve_username(&mut self, _username: &str) -> Result<tl::types::User> {
        unimplemented!();
    }

    /// Sends a text message to the desired chat.
    pub fn send_message<C: IntoInput<tl::enums::InputPeer>>(
        &mut self,
        _chat: C,
        _message: &str,
    ) -> Result<()> {
        unimplemented!();
    }

    /// Initializes the connection with Telegram. If this is never done on
    /// a fresh session, then Telegram won't know which layer to use and a
    /// very old one will be used (which we will fail to understand).
    fn init_connection(&mut self) -> Result<()> {
        // TODO add layer to tl, and then use that
        let got = self.invoke(&tl::functions::InvokeWithLayer {
            layer: 109,
            query: tl::functions::InitConnection {
                api_id: 6,
                device_model: "Linux".into(),
                system_version: "4.15.0-74-generic".into(),
                app_version: "0.1.0".into(),
                system_lang_code: "en".into(),
                lang_pack: "".into(),
                lang_code: "en".into(),
                proxy: None,
                query: tl::functions::help::GetConfig {}.to_bytes(),
            }
            .to_bytes(),
        })?;
        dbg!(got);
        Ok(())
    }

    /// Invokes a raw request, and returns its result.
    pub fn invoke<R: RPC>(&mut self, request: &R) -> Result<R::Return> {
        self.sender.invoke(request)
    }
}
