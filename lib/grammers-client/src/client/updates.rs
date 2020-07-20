use crate::Client;
use futures::stream::StreamExt;
pub use grammers_mtsender::{AuthorizationError, InvocationError};
use grammers_tl_types as tl;

impl Client {
    // TODO while raw access to updates is a start, it's awful to work with
    // because it may have nested Updates or just UpdatesShort
    pub async fn next_updates(&mut self) -> Option<tl::enums::Updates> {
        self.updates.next().await
    }
}
