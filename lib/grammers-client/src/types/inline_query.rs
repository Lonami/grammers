use crate::{client::Client, utils::generate_random_id, InputMessage};
use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;

/// Represents an inline query update, which occurs when you sign in as a bot and a user sends an
/// inline query such as `@bot query`.
pub struct InlineQuery {
    query: tl::types::UpdateBotInlineQuery,
    client: Client,
}

/// An inline query answer builder.
pub struct Answer {
    request: tl::functions::messages::SetInlineBotResults,
    client: Client,
}

impl InlineQuery {
    pub(crate) fn new(client: &Client, query: tl::types::UpdateBotInlineQuery) -> Self {
        Self {
            query,
            client: client.clone(),
        }
    }

    // User that sent the query.
    pub fn user_id(&self) -> i32 {
        self.query.user_id
    }

    // The text of the inline query.
    pub fn text(&self) -> &str {
        self.query.query.as_str()
    }

    /// Answer the inline query.
    // TODO: add example
    pub fn answer(&self, results: Vec<tl::enums::InputBotInlineResult>) -> Answer {
        Answer {
            request: tl::functions::messages::SetInlineBotResults {
                gallery: false,
                private: false,
                query_id: self.query.query_id,
                results,
                cache_time: 0,
                next_offset: None,
                switch_pm: None,
            },
            client: self.client.clone(),
        }
    }
}

impl Answer {
    /// Whether the results should show as a gallery (grid) or not.
    /// Defaults to false.
    pub fn gallery(mut self, gallery: bool) -> Self {
        self.request.gallery = gallery;
        self
    }

    /// Whether the results should be cached by Telegram (not private) or by the user's client
    /// (private).
    /// Defaults to false.
    pub fn private(mut self, private: bool) -> Self {
        self.request.private = private;
        self
    }

    /// For how long this result should be cached on the user's client. Defaults to 0 for no
    /// cache.
    pub fn cache_time(mut self, cache_time: i32) -> Self {
        self.request.cache_time = cache_time;
        self
    }

    /// The offset the client will send when the user scrolls the results and it repeats the
    /// request.
    pub fn next_offset(mut self, next_offset: impl Into<String>) -> Self {
        self.request.next_offset = Some(next_offset.into());
        self
    }

    /// If set, this text will be shown in the results to allow the user to switch to private
    /// messages.
    pub fn switch_pm(mut self, text: impl Into<String>, start_param: impl Into<String>) -> Self {
        self.request.switch_pm = Some(tl::enums::InlineBotSwitchPm::Pm(
            tl::types::InlineBotSwitchPm {
                text: text.into(),
                start_param: start_param.into(),
            },
        ));
        self
    }

    /// Answers the inline query with the given results.
    pub async fn send(self) -> Result<(), InvocationError> {
        self.client.invoke(&self.request).await?;
        Ok(())
    }
}

pub struct Article {
    title: String,
    description: Option<String>,
    url: Option<String>,
    thumb_url: Option<String>,
    input_message: InputMessage,
}

impl Article {
    pub fn new(title: impl Into<String>, input_message: InputMessage) -> Self {
        Self {
            title: title.into(),
            description: None,
            url: None,
            thumb_url: None,
            input_message,
        }
    }

    /// Short description of the result.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// URL of the result.
    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    /// URL of the thumbnail for the result.
    pub fn thumb_url(mut self, thumb_url: impl Into<String>) -> Self {
        self.thumb_url = Some(thumb_url.into());
        self
    }
}

impl From<Article> for tl::enums::InputBotInlineResult {
    fn from(article: Article) -> Self {
        Self::Result(tl::types::InputBotInlineResult {
            id: generate_random_id().to_string(),
            r#type: "article".into(),
            title: Some(article.title),
            description: article.description,
            url: article.url,
            thumb: article.thumb_url.map(|url| {
                tl::enums::InputWebDocument::Document(tl::types::InputWebDocument {
                    url,
                    size: 0,
                    mime_type: "image/jpeg".into(),
                    attributes: vec![],
                })
            }),
            content: None,
            // TODO: also allow other types of messages than text
            send_message: tl::enums::InputBotInlineMessage::Text(
                tl::types::InputBotInlineMessageText {
                    no_webpage: !article.input_message.link_preview,
                    message: article.input_message.text,
                    entities: Some(article.input_message.entities),
                    reply_markup: article.input_message.reply_markup,
                },
            ),
        })
    }
}
