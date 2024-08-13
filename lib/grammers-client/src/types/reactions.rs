// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use grammers_tl_types as tl;
use tl::enums::Reaction;

#[derive(Clone, Debug)]
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
}

impl Default for InputReactions {
    fn default() -> Self {
        Self {
            reactions: vec![],
            add_to_recent: false,
            big: false,
        }
    }
}

impl Into<InputReactions> for String {
    fn into(self) -> InputReactions {
        InputReactions::emoticon(self)
    }
}

impl Into<InputReactions> for &str {
    fn into(self) -> InputReactions {
        InputReactions::emoticon(self)
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

impl Into<Vec<Reaction>> for InputReactions {
    fn into(self) -> Vec<Reaction> {
        return self.reactions;
    }
}
