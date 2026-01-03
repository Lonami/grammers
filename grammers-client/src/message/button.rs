// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Custom keyboards and inline buttons used by messages.

use grammers_tl_types as tl;

/// Inline button to be used as the reply markup underneath a message.
pub struct Button {
    pub raw: tl::enums::KeyboardButton,
}

/// Key to be used as the reply markup of an alternate virtual keyboard.
pub struct Key {
    pub raw: tl::enums::KeyboardButton,
}

impl Button {
    /// Creates a button that will trigger a [`crate::update::CallbackQuery`] when clicked.
    ///
    /// Although any combination of bytes can be used (including null, and not just UTF-8), [there is
    /// a limit](https://core.telegram.org/bots/api#inlinekeyboardbutton) to how long the payload data
    /// can be (see the description for the `callback_data` field for an up-to-date value). If you
    /// need to store more data than that, consider storing the real data in some form of database,
    /// and a reference to that data's row in the button's payload.
    ///
    /// Both the text and bytes data must be non-empty.
    pub fn data<T: Into<String>, B: Into<Vec<u8>>>(text: T, bytes: B) -> Button {
        Button {
            raw: tl::types::KeyboardButtonCallback {
                text: text.into(),
                data: bytes.into(),
                requires_password: false,
            }
            .into(),
        }
    }

    /// Creates a button to force the user to switch to inline mode (perform inline queries).
    ///
    /// Pressing the button will insert the bot's username and the specified inline query in the input
    /// field.
    pub fn switch<T: Into<String>, Q: Into<String>>(text: T, query: Q) -> Button {
        Button {
            raw: tl::types::KeyboardButtonSwitchInline {
                text: text.into(),
                query: query.into(),
                same_peer: true,
                peer_types: None,
            }
            .into(),
        }
    }
    /// Creates a button identical to [`Self::switch`], except the user will be prompted to select a
    /// different peer.
    ///
    /// Pressing the button will prompt the user to select one of their peers, open that peer and
    /// insert the bot's username and the specified inline query in the input field.
    pub fn switch_elsewhere<T: Into<String>, Q: Into<String>>(text: T, query: Q) -> Button {
        Button {
            raw: tl::types::KeyboardButtonSwitchInline {
                text: text.into(),
                query: query.into(),
                same_peer: false,
                peer_types: None,
            }
            .into(),
        }
    }

    /// Creates a button that when clicked will ask the user if they want to open the specified URL.
    ///
    /// The URL will be visible to the user before it's opened unless it's trusted (such as Telegram's
    /// domain).
    pub fn url<T: Into<String>, U: Into<String>>(text: T, url: U) -> Button {
        Button {
            raw: tl::types::KeyboardButtonUrl {
                text: text.into(),
                url: url.into(),
            }
            .into(),
        }
    }

    /// Creates a button that when clicked will open the specified URL in an in-app browser.
    pub fn webview<T: Into<String>, U: Into<String>>(text: T, url: U) -> Button {
        Button {
            raw: tl::types::KeyboardButtonWebView {
                text: text.into(),
                url: url.into(),
            }
            .into(),
        }
    }
}

impl Key {
    /// Creates a simple keyboard key.
    ///
    /// When pressed, the button's text will be sent as a normal message, as if the user had typed it.
    pub fn text<T: Into<String>>(text: T) -> Key {
        Key {
            raw: tl::types::KeyboardButton { text: text.into() }.into(),
        }
    }

    /// Creates a keyboard key to request the user's contact information (including the phone).
    pub fn request_phone<T: Into<String>>(text: T) -> Key {
        Key {
            raw: tl::types::KeyboardButtonRequestPhone { text: text.into() }.into(),
        }
    }

    /// Creates a keyboard key to request the user's current geo-location.
    pub fn request_geo<T: Into<String>>(text: T) -> Key {
        Key {
            raw: tl::types::KeyboardButtonRequestGeoLocation { text: text.into() }.into(),
        }
    }

    /// Creates a keyboard key that will direct the user to create and send a poll when pressed.
    ///
    /// This is only available in direct conversations with the user.
    pub fn request_poll<T: Into<String>>(text: T) -> Key {
        Key {
            raw: tl::types::KeyboardButtonRequestPoll {
                text: text.into(),
                quiz: None,
            }
            .into(),
        }
    }

    /// Creates a keyboard key identical to [`Self::request_poll`], except the poll requested must be a quiz.
    ///
    /// This is only available in direct conversations with the user.
    pub fn request_quiz<T: Into<String>>(text: T) -> Key {
        Key {
            raw: tl::types::KeyboardButtonRequestPoll {
                text: text.into(),
                quiz: Some(true),
            }
            .into(),
        }
    }
}

/*
TODO implement other buttons
(with password) keyboardButtonCallback#35bbdb6b flags:# requires_password:flags.0?true text:string data:bytes = KeyboardButton;
keyboardButtonUrlAuth#10b78d29 flags:# text:string fwd_text:flags.0?string url:string button_id:int = KeyboardButton;
keyboardButtonGame#50f41ccf text:string = KeyboardButton;
keyboardButtonBuy#afd93fbb text:string = KeyboardButton;
inputKeyboardButtonUrlAuth#d02e7fd4 flags:# request_write_access:flags.0?true text:string fwd_text:flags.1?string url:string bot:InputUser = KeyboardButton;
*/
