use futures_util::future::Either;
use grammers_mtsender::InvocationError;
use grammers_session::PackedChat;
use grammers_tl_types as tl;
use std::future::Future;
use tl::enums::SendMessageAction;

use crate::Client;

pub struct ActionSender {
    pub(crate) client: Client,
    pub(crate) chat: PackedChat,
    pub(crate) topic_id: Option<i32>,
}

impl ActionSender {
    /// Set a topic id
    pub fn topic_id(mut self, topic_id: i32) -> Self {
        self.topic_id = Some(topic_id);
        self
    }

    /// Cancel any actions
    pub async fn cancel(&self) -> Result<(), InvocationError> {
        self.oneshot(SendMessageAction::SendMessageCancelAction)
            .await?;

        Ok(())
    }

    /// Do a one-shot set action request
    pub async fn oneshot<A: Into<SendMessageAction>>(
        &self,
        action: A,
    ) -> Result<(), InvocationError> {
        self.client
            .invoke(&tl::functions::messages::SetTyping {
                peer: self.chat.to_input_peer(),
                top_msg_id: self.topic_id,
                action: action.into(),
            })
            .await?;

        Ok(())
    }

    /// Repeat set action request until the future is done
    ///
    /// # Example
    ///
    /// ```
    /// # use std::time::Duration;
    ///
    /// # async fn f(chat: grammers_client::types::Chat, client: grammers_client::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// use grammers_tl_types as tl;
    ///
    /// let heavy_task = async {
    ///     tokio::time::sleep(Duration::from_secs(10)).await;
    ///
    ///     42
    /// };
    ///
    /// tokio::pin!(heavy_task);
    ///
    /// let (task_result, _) = client
    ///     .action(&chat)
    ///     .repeat(
    ///         // most clients doesn't actually show progress of an action
    ///         || tl::types::SendMessageUploadDocumentAction { progress: 0 },
    ///         Duration::from_secs(4), // repeat request every 4 seconds,
    ///         heavy_task
    ///     )
    ///     .await;
    ///
    /// // Note: repeat function does not cancel actions automatically, they will just fade away
    ///
    /// assert_eq!(task_result, 42);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn repeat<
        Action: Into<SendMessageAction>,
        Func: Fn() -> Action,
        T,
        Fut: Future<Output = T> + Unpin,
    >(
        &self,
        action: Func,
        repeat_delay: std::time::Duration,
        mut future: Fut,
    ) -> (T, Result<(), InvocationError>) {
        let mut request_result = Ok(());

        let future_output = loop {
            if request_result.is_err() {
                // Don't try to make a request again
                return (future.await, request_result);
            }

            let action = async {
                request_result = self.oneshot(action().into()).await;
                tokio::time::sleep(repeat_delay).await;
            };

            tokio::pin!(action);

            match futures_util::future::select(action, &mut future).await {
                Either::Left((_, _)) => continue,
                Either::Right((output, _)) => break output,
            }
        };

        (future_output, request_result)
    }
}
