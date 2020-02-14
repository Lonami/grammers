#![cfg(any(feature = "markdown", feature = "html"))]
use grammers_tl_types as tl;

/// The length of a string, according to Telegram.
///
/// Telegram considers the length of the string with surrogate pairs.
fn telegram_string_len(string: &str) -> i32 {
    // https://en.wikipedia.org/wiki/Plane_(Unicode)#Overview
    string.encode_utf16().count() as i32
}

/// Return the offset for the given entity.
fn entity_offset(entity: &tl::enums::MessageEntity) -> i32 {
    use tl::enums::MessageEntity as ME;

    match entity {
        ME::MessageEntityUnknown(e) => e.offset,
        ME::MessageEntityMention(e) => e.offset,
        ME::MessageEntityHashtag(e) => e.offset,
        ME::MessageEntityBotCommand(e) => e.offset,
        ME::MessageEntityUrl(e) => e.offset,
        ME::MessageEntityEmail(e) => e.offset,
        ME::MessageEntityBold(e) => e.offset,
        ME::MessageEntityItalic(e) => e.offset,
        ME::MessageEntityCode(e) => e.offset,
        ME::MessageEntityPre(e) => e.offset,
        ME::MessageEntityTextUrl(e) => e.offset,
        ME::MessageEntityMentionName(e) => e.offset,
        ME::InputMessageEntityMentionName(e) => e.offset,
        ME::MessageEntityPhone(e) => e.offset,
        ME::MessageEntityCashtag(e) => e.offset,
        ME::MessageEntityUnderline(e) => e.offset,
        ME::MessageEntityStrike(e) => e.offset,
        ME::MessageEntityBlockquote(e) => e.offset,
    }
}

#[cfg(feature = "markdown")]
pub fn parse_markdown_message(message: &str) -> (String, Vec<tl::enums::MessageEntity>) {
    use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag};

    let mut text = String::with_capacity(message.len());
    let mut entities = Vec::new();

    let mut offset = 0;
    let mut live_entities = Vec::new();
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
            live_entities.push(tl::types::MessageEntityBold { offset, length: 0 }.into());
        }
        Event::End(Tag::Strong) => {
            let index = live_entities
                .iter_mut()
                .rposition(|e| match e {
                    tl::enums::MessageEntity::MessageEntityBold(e) => {
                        e.length = offset - e.offset;
                        true
                    }
                    _ => false,
                })
                .unwrap();

            entities.push(live_entities.remove(index));
        }

        // *italic text*
        Event::Start(Tag::Emphasis) => {
            live_entities.push(tl::types::MessageEntityItalic { offset, length: 0 }.into());
        }
        Event::End(Tag::Emphasis) => {
            let index = live_entities
                .iter_mut()
                .rposition(|e| match e {
                    tl::enums::MessageEntity::MessageEntityItalic(e) => {
                        e.length = offset - e.offset;
                        true
                    }
                    _ => false,
                })
                .unwrap();

            entities.push(live_entities.remove(index));
        }

        // [text link](https://example.com)
        Event::Start(Tag::Link(_kind, url, _title)) => {
            live_entities.push(
                tl::types::MessageEntityTextUrl {
                    offset,
                    length: 0,
                    url: url.to_string(),
                }
                .into(),
            );
        }
        Event::End(Tag::Link(_kindd, _url, _title)) => {
            let index = live_entities
                .iter_mut()
                .rposition(|e| match e {
                    tl::enums::MessageEntity::MessageEntityTextUrl(e) => {
                        e.length = offset - e.offset;
                        true
                    }
                    _ => false,
                })
                .unwrap();

            entities.push(live_entities.remove(index));
        }

        // ```lang\npre```
        Event::Start(Tag::CodeBlock(kind)) => {
            live_entities.push(
                tl::types::MessageEntityPre {
                    offset,
                    length: 0,
                    language: match kind {
                        CodeBlockKind::Indented => "".to_string(),
                        CodeBlockKind::Fenced(lang) => lang.to_string(),
                    },
                }
                .into(),
            );
        }
        Event::End(Tag::CodeBlock(_kind)) => {
            let index = live_entities
                .iter_mut()
                .rposition(|e| match e {
                    tl::enums::MessageEntity::MessageEntityPre(e) => {
                        e.length = offset - e.offset;
                        true
                    }
                    _ => false,
                })
                .unwrap();

            entities.push(live_entities.remove(index));
        }

        _ => {}
    });

    // When nesting entities, we will close the inner one before the outer one,
    // but we want to have them sorted by offset.
    entities.sort_by(|a, b| entity_offset(a).cmp(&entity_offset(b)));

    (text, entities)
}

#[cfg(feature = "html")]
pub fn parse_html_message(message: &str) -> (String, Vec<tl::enums::MessageEntity>) {
    todo!()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "markdown")]
    fn parse_leading_markdown() {
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
    #[cfg(feature = "markdown")]
    fn parse_trailing_markdown() {
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
    #[cfg(feature = "markdown")]
    fn parse_emoji_markdown() {
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
    #[cfg(feature = "markdown")]
    fn parse_all_entities_markdown() {
        let (text, entities) = parse_markdown_message(
            "Some **bold** (__strong__), *italics* (_cursive_), inline `code`, \
            a\n```rust\npre\n```\nblock, and [links](https://example.com)",
        );

        assert_eq!(
            text,
            "Some bold (strong), italics (cursive), inline code, apre\nblock, and links"
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
                    offset: 53,
                    length: 4,
                    language: "rust".to_string()
                }
                .into(),
                tl::types::MessageEntityTextUrl {
                    offset: 68,
                    length: 5,
                    url: "https://example.com".to_string()
                }
                .into(),
            ]
        );
    }

    #[test]
    #[cfg(feature = "markdown")]
    fn parse_nested_entities_markdown() {
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
}
