use std::time::{SystemTime, UNIX_EPOCH};

use grammers_tl_types as tl;

// https://github.com/telegramdesktop/tdesktop/blob/e7fbcce9d9f0a8944eb2c34e74bd01b8776cb891/Telegram/SourceFiles/data/data_scheduled_messages.h#L52
const SCHEDULE_ONCE_ONLINE: i32 = 0x7FFFFFFE;

#[derive(Default)]
pub struct Message {
    pub(crate) background: bool,
    pub(crate) clear_draft: bool,
    pub(crate) entities: Vec<tl::enums::MessageEntity>,
    pub(crate) link_preview: bool,
    pub(crate) reply_markup: Option<tl::enums::ReplyMarkup>,
    pub(crate) reply_to: Option<i32>,
    pub(crate) schedule_date: Option<i32>,
    pub(crate) silent: bool,
    pub(crate) text: String,
}

impl Message {
    /// Whether to "send this message as a background message".
    ///
    /// This description is taken from https://core.telegram.org/method/messages.sendMessage.
    pub fn background(mut self, background: bool) -> Self {
        self.background = background;
        self
    }

    /// Whether the draft in this chat, if any, should be cleared.
    pub fn clear_draft(mut self, clear_draft: bool) -> Self {
        self.clear_draft = clear_draft;
        self
    }

    /// The entities within the message (such as bold, italics, etc.).
    pub fn entities(mut self, entities: Vec<tl::enums::MessageEntity>) -> Self {
        self.entities = entities;
        self
    }

    /// Whether the link preview be shown for the message.
    ///
    /// This has no effect when sending media, which cannot contain a link preview.
    pub fn link_preview(mut self, link_preview: bool) -> Self {
        self.link_preview = link_preview;
        self
    }

    /// The reply markup to show under the message.
    ///
    /// User accounts cannot use markup, and it will be ignored if set.
    pub fn reply_markup(mut self, reply_markup: Option<tl::enums::ReplyMarkup>) -> Self {
        self.reply_markup = reply_markup;
        self
    }

    /// The message identifier to which this message should reply to, if any.
    ///
    /// Otherwise, this message will not be a reply to any other.
    pub fn reply_to(mut self, reply_to: Option<i32>) -> Self {
        self.reply_to = reply_to;
        self
    }

    /// If set to a distant enough future time, the message won't be sent immediately,
    /// and instead it will be scheduled to be automatically sent at a later time.
    ///
    /// This scheduling is done server-side, and may not be accurate to the second.
    ///
    /// Bot accounts cannot schedule messages.
    pub fn schedule_date(mut self, schedule_date: Option<SystemTime>) -> Self {
        self.schedule_date = schedule_date.map(|t| {
            t.duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i32)
                .unwrap_or(0)
        });
        self
    }

    /// Schedule the message to be sent once the person comes online.
    ///
    /// This only works in private chats, and only if the person has their
    /// last seen visible.
    ///
    /// Bot accounts cannot schedule messages.
    pub fn schedule_once_online(mut self) -> Self {
        self.schedule_date = Some(SCHEDULE_ONCE_ONLINE);
        self
    }

    /// Whether the message should notify people or not.
    ///
    /// Defaults to `false`, which means it will notify them. Set it to `true`
    /// to alter this behaviour.
    pub fn silent(mut self, silent: bool) -> Self {
        self.silent = silent;
        self
    }

    /// Builds a new message using the given plaintext as the message contents.
    pub fn text<T: AsRef<str>>(s: T) -> Self {
        Self {
            text: s.as_ref().to_string(),
            ..Self::default()
        }
    }

    /// Builds a new message from the given markdown-formatted string as the
    /// message contents and entities.
    ///
    /// Note that Telegram only supports a very limited subset of entities:
    /// bold, italic, underline, strikethrough, code blocks, pre blocks and inline links.
    #[cfg(feature = "markdown")]
    pub fn markdown<T: AsRef<str>>(s: T) -> Self {
        let (text, entities) = crate::parsers::parse_markdown_message(s.as_ref());
        Self {
            text,
            entities,
            ..Self::default()
        }
    }

    /// Builds a new message from the given HTML-formatted string as the
    /// message contents and entities.
    ///
    /// Note that Telegram only supports a very limited subset of entities:
    /// bold, italic, underline, strikethrough, code blocks, pre blocks and inline links.
    #[cfg(feature = "html")]
    pub fn html<T: AsRef<str>>(s: T) -> Self {
        let (text, entities) = crate::parsers::parse_html_message(s.as_ref());
        Self {
            text,
            entities,
            ..Self::default()
        }
    }
}

impl From<&str> for Message {
    fn from(text: &str) -> Self {
        Message::text(text)
    }
}

impl From<String> for Message {
    fn from(text: String) -> Self {
        Message {
            text,
            ..Self::default()
        }
    }
}
