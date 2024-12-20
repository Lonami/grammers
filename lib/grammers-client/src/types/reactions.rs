// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use grammers_tl_types as tl;
use tl::enums::Reaction;

#[derive(Clone, Debug, Default)]
pub struct InputReactions {
    pub(crate) reactions: Vec<Reaction>,
    pub(crate) add_to_recent: bool,
    pub(crate) big: bool,
}

impl InputReactions {
    /// Make reaction animation big.
    pub fn big(mut self) -> Self {
        self.big = true;
        self
    }

    /// Add this reaction to the recent reactions list.
    ///
    /// More about that: \
    /// https://core.telegram.org/api/reactions#recent-reactions
    pub fn add_to_recent(mut self) -> Self {
        self.add_to_recent = true;
        self
    }

    /// Create new InputReactions with one emoticon reaction
    pub fn emoticon<S: Into<String>>(emoticon: S) -> Self {
        Self {
            reactions: vec![Reaction::Emoji(tl::types::ReactionEmoji {
                emoticon: emoticon.into(),
            })],
            ..Self::default()
        }
    }

    /// Create new InputReactions with one custom emoji reaction
    pub fn custom_emoji(document_id: i64) -> Self {
        Self {
            reactions: vec![Reaction::CustomEmoji(tl::types::ReactionCustomEmoji {
                document_id,
            })],
            ..Self::default()
        }
    }

    /// Create an empty InputReactions which will remove reactions
    pub fn remove() -> Self {
        Self::default()
    }
}

impl From<String> for InputReactions {
    fn from(val: String) -> Self {
        InputReactions::emoticon(val)
    }
}

impl From<&str> for InputReactions {
    fn from(val: &str) -> Self {
        InputReactions::emoticon(val)
    }
}

impl From<Vec<Reaction>> for InputReactions {
    fn from(reactions: Vec<Reaction>) -> Self {
        Self {
            reactions,
            ..Self::default()
        }
    }
}

impl From<InputReactions> for Vec<Reaction> {
    fn from(val: InputReactions) -> Self {
        val.reactions
    }
}
