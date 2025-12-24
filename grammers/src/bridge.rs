// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use tokio::sync::mpsc;

use grammers_session::types::PeerId;

pub enum BackendMessage {
    NeedLoginPhone,
    NeedLoginCode,
    NeedLoginPassword { hint: Option<String> },
    LoginSuccess,
    Dialogs(Vec<grammers_client::types::Dialog>),
    Messages(Vec<grammers_client::types::Message>),
}

pub struct BackendContext {
    pub backend_sender: mpsc::UnboundedSender<BackendMessage>,
    pub backend_receiver: mpsc::UnboundedReceiver<FrontendMessage>,
}

pub enum FrontendMessage {
    LoginPhone(String),
    LoginCode(String),
    LoginPassword(String),
    FetchMessages { peer: PeerId },
    SendMessage { peer: PeerId, message: String },
}

pub struct FrontendContext {
    pub frontend_receiver: mpsc::UnboundedReceiver<BackendMessage>,
    pub frontend_sender: mpsc::UnboundedSender<FrontendMessage>,
}
