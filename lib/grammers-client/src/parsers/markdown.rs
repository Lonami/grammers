// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
#![cfg(feature = "markdown")]

use super::common::{
    after, before, inject_into_message, telegram_string_len, Segment, MENTION_URL_PREFIX,
};
use crate::update_entity_len;
use grammers_tl_types as tl;
use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag};

pub fn parse_markdown_message(message: &str) -> (String, Vec<tl::enums::MessageEntity>) {
    let mut text = String::with_capacity(message.len());
    let mut entities = Vec::new();

    let mut offset = 0;
    let length = 0;
    Parser::new(message).for_each(|event| match event {
        // text
        Event::Text(string) => {
            text.push_str(&string);
            offset += telegram_string_len(&string);
        }

        // `code`
        Event::Code(string) => {
            text.push_str(&string);
            let length = telegram_string_len(&string);
            entities.push(tl::types::MessageEntityCode { offset, length }.into());
            offset += length;
        }

        // **bold text**
        Event::Start(Tag::Strong) => {
            entities.push(tl::types::MessageEntityBold { offset, length }.into());
        }
        Event::End(Tag::Strong) => {
            update_entity_len!(Bold(offset) in entities);
        }

        // *italic text*
        Event::Start(Tag::Emphasis) => {
            entities.push(tl::types::MessageEntityItalic { offset, length }.into());
        }
        Event::End(Tag::Emphasis) => {
            update_entity_len!(Italic(offset) in entities);
        }

        // [text link](https://example.com) or [user mention](tg://user?id=12345678)
        Event::Start(Tag::Link(_kind, url, _title)) => {
            if url.starts_with(MENTION_URL_PREFIX) {
                let user_id = url[MENTION_URL_PREFIX.len()..].parse::<i64>().unwrap();
                entities.push(
                    tl::types::MessageEntityMentionName {
                        offset,
                        length,
                        user_id,
                    }
                    .into(),
                );
            } else {
                entities.push(
                    tl::types::MessageEntityTextUrl {
                        offset,
                        length,
                        url: url.to_string(),
                    }
                    .into(),
                );
            }
        }
        Event::End(Tag::Link(_kindd, url, _title)) => {
            if url.starts_with(MENTION_URL_PREFIX) {
                update_entity_len!(MentionName(offset) in entities);
            } else {
                update_entity_len!(TextUrl(offset) in entities);
            }
        }

        // ```lang\npre```
        Event::Start(Tag::CodeBlock(kind)) => {
            let lang = match kind {
                CodeBlockKind::Indented => "".to_string(),
                CodeBlockKind::Fenced(lang) => lang.to_string(),
            }
            .to_string();

            entities.push(
                tl::types::MessageEntityPre {
                    offset,
                    length,
                    language: lang,
                }
                .into(),
            );
        }
        Event::End(Tag::CodeBlock(_kind)) => {
            update_entity_len!(Pre(offset) in entities);
        }
        // "\\\n"
        Event::HardBreak => {
            text.push('\n');
            offset += 1;
        }
        // "\n\n"
        Event::End(Tag::Paragraph) => {
            text.push_str("\n\n");
            offset += 2;
        }
        _ => {}
    });

    text.truncate(text.trim_end().len());
    (text, entities)
}

pub fn generate_markdown_message(message: &str, entities: &[tl::enums::MessageEntity]) -> String {
    // Getting this wrong isn't the end of the world so the wildcard pattern is used
    // (but it would still be a shame for it to be wrong).
    let mut insertions = Vec::with_capacity(
        entities
            .iter()
            .map(|entity| match entity {
                ME::Bold(_) => 2,
                ME::Italic(_) => 2,
                ME::Code(_) => 2,
                ME::Pre(e) => {
                    if e.language.is_empty() {
                        2
                    } else {
                        4
                    }
                }
                ME::TextUrl(_) => 4,
                ME::MentionName(_) => 4,
                _ => 0,
            })
            .sum(),
    );
    use tl::enums::MessageEntity as ME;
    entities
        .iter()
        .enumerate()
        .for_each(|(i, entity)| match entity {
            ME::Unknown(_) => {}
            ME::Mention(_) => {}
            ME::Hashtag(_) => {}
            ME::BotCommand(_) => {}
            ME::Url(_) => {}
            ME::Email(_) => {}
            ME::Bold(e) => {
                insertions.push((before(i, 0, e.offset), Segment::Fixed("**")));
                insertions.push((after(i, 0, e.offset + e.length), Segment::Fixed("**")));
            }
            ME::Italic(e) => {
                insertions.push((before(i, 0, e.offset), Segment::Fixed("_")));
                insertions.push((after(i, 0, e.offset + e.length), Segment::Fixed("_")));
            }
            ME::Code(e) => {
                insertions.push((before(i, 0, e.offset), Segment::Fixed("`")));
                insertions.push((after(i, 0, e.offset + e.length), Segment::Fixed("`")));
            }
            ME::Pre(e) => {
                if e.language.is_empty() {
                    insertions.push((before(i, 0, e.offset), Segment::Fixed("```\n")));
                    insertions.push((after(i, 0, e.offset + e.length), Segment::Fixed("```\n")));
                } else {
                    insertions.push((before(i, 0, e.offset), Segment::Fixed("```")));
                    insertions.push((before(i, 1, e.offset), Segment::String(&e.language)));
                    insertions.push((before(i, 2, e.offset), Segment::Fixed("\n")));
                    insertions.push((after(i, 0, e.offset + e.length), Segment::Fixed("```\n")));
                }
            }
            ME::TextUrl(e) => {
                insertions.push((before(i, 0, e.offset), Segment::Fixed("[")));
                insertions.push((after(i, 0, e.offset + e.length), Segment::Fixed("](")));
                insertions.push((after(i, 1, e.offset + e.length), Segment::String(&e.url)));
                insertions.push((after(i, 2, e.offset + e.length), Segment::Fixed(")")));
            }
            ME::MentionName(e) => {
                insertions.push((before(i, 0, e.offset), Segment::Fixed("[")));
                insertions.push((
                    after(i, 0, e.offset + e.length),
                    Segment::Fixed("](tg://user?id="),
                ));
                insertions.push((after(i, 1, e.offset + e.length), Segment::Number(e.user_id)));
                insertions.push((after(i, 2, e.offset + e.length), Segment::Fixed(")")));
            }
            ME::InputMessageEntityMentionName(_) => {}
            ME::Phone(_) => {}
            ME::Cashtag(_) => {}
            ME::Underline(_) => {}
            ME::Strike(_) => {}
            ME::Blockquote(_) => {}
            ME::BankCard(_) => {}
            ME::Spoiler(_) => {}
            ME::CustomEmoji(_) => {}
        });

    inject_into_message(message, insertions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_leading() {
        let (text, entities) = parse_markdown_message("**Hello** world!");
        assert_eq!(text, "Hello world!");
        assert_eq!(
            entities,
            vec![tl::types::MessageEntityBold {
                offset: 0,
                length: 5
            }
            .into()]
        );
    }

    #[test]
    fn parse_trailing() {
        let (text, entities) = parse_markdown_message("Hello **world!**");
        assert_eq!(text, "Hello world!");
        assert_eq!(
            entities,
            vec![tl::types::MessageEntityBold {
                offset: 6,
                length: 6
            }
            .into()]
        );
    }

    #[test]
    fn parse_emoji() {
        let (text, entities) = parse_markdown_message("A **little 🦀** here");
        assert_eq!(text, "A little 🦀 here");
        assert_eq!(
            entities,
            vec![tl::types::MessageEntityBold {
                offset: 2,
                length: 9
            }
            .into()]
        );
    }

    #[test]
    fn parse_all_entities() {
        let (text, entities) = parse_markdown_message(
            "Some **bold** (__strong__), *italics* (_cursive_), inline `code`, \
            a\n```rust\npre\n```\nblock, a [**link**](https://example.com), and \
            [mentions](tg://user?id=12345678)",
        );

        assert_eq!(
            text,
            "Some bold (strong), italics (cursive), inline code, a\n\npre\nblock, a link, and mentions"
        );
        assert_eq!(
            entities,
            vec![
                tl::types::MessageEntityBold {
                    offset: 5,
                    length: 4
                }
                .into(),
                tl::types::MessageEntityBold {
                    offset: 11,
                    length: 6
                }
                .into(),
                tl::types::MessageEntityItalic {
                    offset: 20,
                    length: 7
                }
                .into(),
                tl::types::MessageEntityItalic {
                    offset: 29,
                    length: 7
                }
                .into(),
                tl::types::MessageEntityCode {
                    offset: 46,
                    length: 4
                }
                .into(),
                tl::types::MessageEntityPre {
                    offset: 55,
                    length: 4,
                    language: "rust".to_string()
                }
                .into(),
                tl::types::MessageEntityTextUrl {
                    offset: 68,
                    length: 4,
                    url: "https://example.com".to_string()
                }
                .into(),
                tl::types::MessageEntityBold {
                    offset: 68,
                    length: 4,
                }
                .into(),
                tl::types::MessageEntityMentionName {
                    offset: 78,
                    length: 8,
                    user_id: 12345678
                }
                .into(),
            ]
        );
    }

    #[test]
    fn parse_nested_entities() {
        // CommonMark won't allow the following: "Some **bold _both** italics_"
        let (text, entities) = parse_markdown_message("Some **bold _both_** _italics_");
        assert_eq!(text, "Some bold both italics");
        assert_eq!(
            entities,
            vec![
                tl::types::MessageEntityBold {
                    offset: 5,
                    length: 9
                }
                .into(),
                tl::types::MessageEntityItalic {
                    offset: 10,
                    length: 4
                }
                .into(),
                tl::types::MessageEntityItalic {
                    offset: 15,
                    length: 7
                }
                .into(),
            ]
        );
    }

    #[test]
    fn parse_then_unparse() {
        let markdown = "Some **bold 🤷🏽‍♀️**, _italics_, inline `🤷🏽‍♀️ code`, \
        a\n\n```rust\npre\n```\nblock, a [**link**](https://example.com), and \
        [mentions](tg://user?id=12345678)";
        let (text, entities) = parse_markdown_message(markdown);
        let generated = generate_markdown_message(&text, &entities);
        assert_eq!(generated, markdown);
    }

    #[test]
    fn parse_then_unparse_overlapping() {
        let markdown = "_a_[**b**](https://example.com)`c`";
        let (text, entities) = parse_markdown_message(markdown);
        let generated = generate_markdown_message(&text, &entities);
        assert_eq!(generated, markdown);
    }
}
