// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! This module contains several functions to build a reply markup usable by bots when sending
//! messages through [`crate::InputMessage::reply_markup`].
//!
//! Each function returns a concrete builder-like type that may be further configured via their
//! inherent methods.
//!
//! The trait is used to group all types as "something that may be used as a  reply markup".
use super::button;
use grammers_tl_types as tl;

#[doc(hidden)]
pub struct Markup(pub(crate) tl::enums::ReplyMarkup);

/// Trait used by types that can be interpreted as a raw reply markup.
pub trait ReplyMarkup {
    fn to_reply_markup(&self) -> Markup;
}

/// Structure holding the state for inline reply markups.
///
/// See [`inline`] for usage examples.
pub struct Inline(tl::types::ReplyInlineMarkup);

/// Structure holding the state for keyboard reply markups.
///
/// See [`keyboard`] for usage examples.
pub struct Keyboard(tl::types::ReplyKeyboardMarkup);

/// Structure holding the state for reply markups that hide previous keyboards.
///
/// See [`hide`] for usage examples.
pub struct Hide(tl::types::ReplyKeyboardHide);

/// Structure holding the state for reply markups that force a reply.
///
/// See [`force_reply`] for usage examples.
pub struct ForceReply(tl::types::ReplyKeyboardForceReply);

impl ReplyMarkup for Inline {
    fn to_reply_markup(&self) -> Markup {
        Markup(self.0.clone().into())
    }
}

impl ReplyMarkup for Keyboard {
    fn to_reply_markup(&self) -> Markup {
        Markup(self.0.clone().into())
    }
}

impl ReplyMarkup for Hide {
    fn to_reply_markup(&self) -> Markup {
        Markup(self.0.clone().into())
    }
}

impl ReplyMarkup for ForceReply {
    fn to_reply_markup(&self) -> Markup {
        Markup(self.0.clone().into())
    }
}

/// Define inline buttons for a message.
///
/// These will display right under the message.
///
/// You cannot add images to the buttons, but you can use emoji (simply copy-paste them into your
/// code, or use the correct escape sequence, or using any other input methods you like).
///
/// You will need to provide a matrix of [`button::Inline`], that is, a vector that contains the
/// rows from top to bottom, where the rows consist of a vector of buttons from left to right.
/// See the [`button`] module to learn what buttons are available.
///
/// # Examples
///
/// ```
/// # async fn f(client: &mut grammers_client::Client, chat: &grammers_client::types::Chat) -> Result<(), Box<dyn std::error::Error>> {
/// use grammers_client::{InputMessage, reply_markup, button};
///
/// let artist = "Krewella";
/// client.send_message(chat, InputMessage::text("Select song").reply_markup(&reply_markup::keyboard(vec![
///     vec![button::text(format!("Song by {}", artist))],
///     vec![button::text("Previous"), button::text("Next")],
/// ]))).await?;
/// # Ok(())
/// # }
/// ```
pub fn inline<B: Into<Vec<Vec<button::Inline>>>>(buttons: B) -> Inline {
    Inline(tl::types::ReplyInlineMarkup {
        rows: buttons
            .into()
            .into_iter()
            .map(|row| {
                tl::types::KeyboardButtonRow {
                    buttons: row.into_iter().map(|button| button.0).collect(),
                }
                .into()
            })
            .collect(),
    })
}

/// Define a custom keyboard, replacing the user's own virtual keyboard.
///
/// This will be displayed below the input message field for users, and on mobile devices, this
/// also hides the virtual keyboard (effectively "replacing" it).
///
/// You cannot add images to the buttons, but you can use emoji (simply copy-paste them into your
/// code, or use the correct escape sequence, or using any other input methods you like).
///
/// You will need to provide a matrix of [`button::Inline`], that is, a vector that contains the
/// rows from top to bottom, where the rows consist of a vector of buttons from left to right.
/// See the [`button`] module to learn what buttons are available.
///
/// See the return type for further configuration options.
///
/// # Examples
///
/// ```
/// # async fn f(client: &mut grammers_client::Client, chat: &grammers_client::types::Chat) -> Result<(), Box<dyn std::error::Error>> {
/// use grammers_client::{InputMessage, reply_markup, button};
///
/// client.send_message(chat, InputMessage::text("What do you want to do?").reply_markup(&reply_markup::keyboard(vec![
///     vec![button::text("Accept")],
///     vec![button::text("Cancel"), button::text("Try something else")],
/// ]))).await?;
/// # Ok(())
/// # }
/// ```
pub fn keyboard<B: Into<Vec<Vec<button::Keyboard>>>>(buttons: B) -> Keyboard {
    Keyboard(tl::types::ReplyKeyboardMarkup {
        resize: false,
        single_use: false,
        selective: false,
        rows: buttons
            .into()
            .into_iter()
            .map(|row| {
                tl::types::KeyboardButtonRow {
                    buttons: row.into_iter().map(|button| button.0).collect(),
                }
                .into()
            })
            .collect(),
    })
}

/// Hide a previously-sent keyboard.
///
/// See the return type for further configuration options.
///
/// # Examples
///
/// ```
/// # async fn f(client: &mut grammers_client::Client, chat: &grammers_client::types::Chat) -> Result<(), Box<dyn std::error::Error>> {
/// use grammers_client::{InputMessage, reply_markup};
///
/// client.send_message(chat, InputMessage::text("Bot keyboards removed.").reply_markup(&reply_markup::hide())).await?;
/// # Ok(())
/// # }
/// ```
pub fn hide() -> Hide {
    Hide(tl::types::ReplyKeyboardHide { selective: false })
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
/// # async fn f(client: &mut grammers_client::Client, chat: &grammers_client::types::Chat) -> Result<(), Box<dyn std::error::Error>> {
/// use grammers_client::{InputMessage, reply_markup};
///
/// let markup = reply_markup::force_reply().single_use();
/// client.send_message(chat, InputMessage::text("Reply me!").reply_markup(&markup)).await?;
/// # Ok(())
/// # }
/// ```
pub fn force_reply() -> ForceReply {
    ForceReply(tl::types::ReplyKeyboardForceReply {
        single_use: false,
        selective: false,
    })
}

impl Keyboard {
    /// Requests clients to resize the keyboard vertically for optimal fit (e.g., make the
    /// keyboard smaller if there are just two rows of buttons). Otherwise, the custom keyboard
    /// is always of the same height as the virtual keyboard.
    pub fn fit_size(mut self) -> Self {
        self.0.resize = true;
        self
    }

    /// Requests clients to hide the keyboard as soon as it's been used.
    ///
    /// The keyboard will still be available, but clients will automatically display the usual
    /// letter-keyboard in the chat – the user can press a special button in the input field to
    /// see the custom keyboard again.
    pub fn single_use(mut self) -> Self {
        self.0.single_use = true;
        self
    }

    /// Force the reply to specific users only.
    ///
    /// The selected user will be either the people @_mentioned in the text of the `Message`
    /// object, or if the bot's message is a reply, the sender of the original message.
    pub fn selective(mut self) -> Self {
        self.0.selective = true;
        self
    }
}

impl Hide {
    /// Hide the keyboard for specific users only.
    ///
    /// The selected user will be either the people @_mentioned in the text of the `Message`
    /// object, or if the bot's message is a reply, the sender of the original message.
    pub fn selective(mut self) -> Self {
        self.0.selective = true;
        self
    }
}

impl ForceReply {
    /// Requests clients to hide the keyboard as soon as it's been used.
    ///
    /// The keyboard will still be available, but clients will automatically display the usual
    /// letter-keyboard in the chat – the user can press a special button in the input field to
    /// see the custom keyboard again.
    pub fn single_use(mut self) -> Self {
        self.0.single_use = true;
        self
    }

    /// Force the reply to specific users only.
    ///
    /// The selected user will be either the people @_mentioned in the text of the `Message`
    /// object, or if the bot's message is a reply, the sender of the original message.
    pub fn selective(mut self) -> Self {
        self.0.selective = true;
        self
    }
}
