// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Contains functions to build reply markups.
//!
//! These can only be used by bot accounts when sending
//! messages through [`InputMessage::reply_markup`].
//!
//! Each function returns a concrete builder-like type that
//! may be further configured via their inherent methods.
//!
//! The trait is used to group all types as "something that
//! may be used as a reply markup".
//!
//! [`InputMessage::reply_markup`]: crate::message::InputMessage::reply_markup

use grammers_tl_types as tl;

use super::{Button, Key};

const EMPTY_KEYBOARD_MARKUP: tl::types::ReplyKeyboardMarkup = tl::types::ReplyKeyboardMarkup {
    resize: false,
    single_use: false,
    selective: false,
    persistent: false,
    rows: Vec::new(),
    placeholder: None,
};

/// Markup to be used as the intended way to reply to the message it is attached to.
pub struct ReplyMarkup {
    pub raw: tl::enums::ReplyMarkup,
}

impl ReplyMarkup {
    /// Define inline buttons for a message.
    ///
    /// These will display right under the message.
    ///
    /// You cannot add images to the buttons, but you can use emoji (simply copy-paste them into your
    /// code, or use the correct escape sequence, or using any other input methods you like).
    ///
    /// You will need to provide a matrix of [`Button`], that is, a vector that contains the
    /// rows from top to bottom, where the rows consist of a vector of buttons from left to right.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(client: &mut grammers_client::Client, peer: grammers_session::types::PeerRef) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::message::{InputMessage, ReplyMarkup, Button};
    ///
    /// let artist = "Krewella";
    /// let markup = ReplyMarkup::from_buttons(&[
    ///     vec![Button::data(format!("Song by {}", artist), b"play")],
    ///     vec![Button::data("Previous", b"prev"), Button::data("Next", b"next")],
    /// ]);
    /// client.send_message(peer, InputMessage::new().text("Select song").reply_markup(markup)).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_buttons(buttons: &[Vec<Button>]) -> Self {
        Self {
            raw: tl::enums::ReplyMarkup::ReplyInlineMarkup(tl::types::ReplyInlineMarkup {
                rows: buttons
                    .into_iter()
                    .map(|row| {
                        tl::types::KeyboardButtonRow {
                            buttons: row.into_iter().map(|button| button.raw.clone()).collect(),
                        }
                        .into()
                    })
                    .collect(),
            }),
        }
    }

    /// Creates a [`ReplyMarkup::from_buttons`] with a single row.
    pub fn from_buttons_row(buttons: &[Button]) -> Self {
        Self {
            raw: tl::enums::ReplyMarkup::ReplyInlineMarkup(tl::types::ReplyInlineMarkup {
                rows: vec![tl::enums::KeyboardButtonRow::Row(
                    tl::types::KeyboardButtonRow {
                        buttons: buttons
                            .into_iter()
                            .map(|button| button.raw.clone())
                            .collect(),
                    },
                )],
            }),
        }
    }

    /// Creates a [`ReplyMarkup::from_buttons`] with a single column.
    pub fn from_buttons_col(buttons: &[Button]) -> Self {
        Self {
            raw: tl::enums::ReplyMarkup::ReplyInlineMarkup(tl::types::ReplyInlineMarkup {
                rows: buttons
                    .into_iter()
                    .map(|button| {
                        tl::enums::KeyboardButtonRow::Row(tl::types::KeyboardButtonRow {
                            buttons: vec![button.raw.clone()],
                        })
                    })
                    .collect(),
            }),
        }
    }

    /// Define a custom keyboard, replacing the user's own virtual keyboard.
    ///
    /// This will be displayed below the input message field for users, and on mobile devices, this
    /// also hides the virtual keyboard (effectively "replacing" it).
    ///
    /// You cannot add images to the buttons, but you can use emoji (simply copy-paste them into your
    /// code, or use the correct escape sequence, or using any other input methods you like).
    ///
    /// You will need to provide a matrix of [`Key`], that is, a vector that contains the
    /// rows from top to bottom, where the rows consist of a vector of buttons from left to right.
    ///
    /// The return type may continue to be configured before being used.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(client: &mut grammers_client::Client, peer: grammers_session::types::PeerRef) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::message::{InputMessage, ReplyMarkup, Key};
    ///
    /// let markup = ReplyMarkup::from_keys(&[
    ///     vec![Key::text("Accept")],
    ///     vec![Key::text("Cancel"), Key::text("Try something else")],
    /// ]);
    /// client.send_message(peer, InputMessage::new().text("What do you want to do?").reply_markup(markup)).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_keys(keys: &[Vec<Key>]) -> Self {
        Self {
            raw: tl::enums::ReplyMarkup::ReplyKeyboardMarkup(tl::types::ReplyKeyboardMarkup {
                rows: keys
                    .into_iter()
                    .map(|row| {
                        tl::types::KeyboardButtonRow {
                            buttons: row.into_iter().map(|key| key.raw.clone()).collect(),
                        }
                        .into()
                    })
                    .collect(),
                ..EMPTY_KEYBOARD_MARKUP
            }),
        }
    }

    /// Creates a [`ReplyMarkup::from_keys`] with a single row.
    pub fn from_keys_row(keys: &[Key]) -> Self {
        Self {
            raw: tl::enums::ReplyMarkup::ReplyKeyboardMarkup(tl::types::ReplyKeyboardMarkup {
                rows: vec![tl::enums::KeyboardButtonRow::Row(
                    tl::types::KeyboardButtonRow {
                        buttons: keys.into_iter().map(|key| key.raw.clone()).collect(),
                    },
                )],
                ..EMPTY_KEYBOARD_MARKUP
            }),
        }
    }

    /// Creates a [`ReplyMarkup::from_keys`] with a single column.
    pub fn from_keys_col(keys: &[Key]) -> Self {
        Self {
            raw: tl::enums::ReplyMarkup::ReplyKeyboardMarkup(tl::types::ReplyKeyboardMarkup {
                rows: keys
                    .into_iter()
                    .map(|key| {
                        tl::enums::KeyboardButtonRow::Row(tl::types::KeyboardButtonRow {
                            buttons: vec![key.raw.clone()],
                        })
                    })
                    .collect(),
                ..EMPTY_KEYBOARD_MARKUP
            }),
        }
    }

    /// Hide a previously-sent keyboard.
    ///
    /// See the return type for further configuration options.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(client: &mut grammers_client::Client, peer: grammers_session::types::PeerRef) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::message::{InputMessage, ReplyMarkup};
    ///
    /// let markup = ReplyMarkup::hide();
    /// client.send_message(peer, InputMessage::new().text("Bot keyboards removed.").reply_markup(markup)).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn hide() -> Self {
        Self {
            raw: tl::enums::ReplyMarkup::ReplyKeyboardHide(tl::types::ReplyKeyboardHide {
                selective: false,
            }),
        }
    }

    /// "Forces" the user to send a reply.
    ///
    /// This will cause the user's application to automatically select the message for replying to it,
    /// although the user is still able to dismiss the reply and send a normal message.
    ///
    /// See the return type for further configuration options.
    ///
    /// # Examples
    ///
    /// ```
    /// # async fn f(client: &mut grammers_client::Client, peer: grammers_session::types::PeerRef) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_client::message::{InputMessage, ReplyMarkup};
    ///
    /// let markup = ReplyMarkup::force_reply().single_use();
    /// client.send_message(peer, InputMessage::new().text("Reply me!").reply_markup(markup)).await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn force_reply() -> Self {
        Self {
            raw: tl::enums::ReplyMarkup::ReplyKeyboardForceReply(
                tl::types::ReplyKeyboardForceReply {
                    single_use: false,
                    selective: false,
                    placeholder: None,
                },
            ),
        }
    }

    /// Requests clients to resize the keyboard vertically for optimal fit (e.g., make the
    /// keyboard smaller if there are just two rows of buttons). Otherwise, the custom keyboard
    /// is always of the same height as the virtual keyboard.
    ///
    /// Only has effect on reply markups that use keys.
    pub fn fit_size(mut self) -> Self {
        use grammers_tl_types::enums::ReplyMarkup as RM;
        match &mut self.raw {
            RM::ReplyKeyboardMarkup(keyboard) => keyboard.resize = true,
            RM::ReplyKeyboardHide(_)
            | RM::ReplyKeyboardForceReply(_)
            | RM::ReplyInlineMarkup(_) => {}
        }
        self
    }

    /// Requests clients to hide the keyboard as soon as it's been used.
    ///
    /// The keyboard will still be available, but clients will automatically display the usual
    /// letter-keyboard in the conversation – the user can press a special button in the input field to
    /// see the custom keyboard again.
    ///
    /// Only has effect on reply markups that use keys or when forcing the user to reply.
    pub fn single_use(mut self) -> Self {
        use grammers_tl_types::enums::ReplyMarkup as RM;
        match &mut self.raw {
            RM::ReplyKeyboardForceReply(keyboard) => keyboard.single_use = true,
            RM::ReplyKeyboardMarkup(keyboard) => keyboard.single_use = true,
            RM::ReplyKeyboardHide(_) | RM::ReplyInlineMarkup(_) => {}
        }
        self
    }

    /// Force the markup to only apply to specific users.
    ///
    /// The selected user will be either the people @-mentioned in the text of the `Message`
    /// object, or if the bot's message is a reply, the sender of the original message.
    ///
    /// Has no effect on markups that consist of inline buttons.
    pub fn selective(mut self) -> Self {
        use grammers_tl_types::enums::ReplyMarkup as RM;
        match &mut self.raw {
            RM::ReplyKeyboardHide(keyboard) => keyboard.selective = true,
            RM::ReplyKeyboardForceReply(keyboard) => keyboard.selective = true,
            RM::ReplyKeyboardMarkup(keyboard) => keyboard.selective = true,
            RM::ReplyInlineMarkup(_) => {}
        }
        self
    }
}
