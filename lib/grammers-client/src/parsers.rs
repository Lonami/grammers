// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
#![cfg(any(feature = "markdown", feature = "html"))]
use grammers_tl_types as tl;

#[cfg(feature = "html")]
const CODE_LANG_PREFIX: &str = "language-";

/// The length of a string, according to Telegram.
///
/// Telegram considers the length of the string with surrogate pairs.
fn telegram_string_len(string: &str) -> i32 {
    // https://en.wikipedia.org/wiki/Plane_(Unicode)#Overview
    string.encode_utf16().count() as i32
}

/// Pushes a new `MessageEntity` instance with zero-length to the specified vector.
///
/// # Examples
///
/// ```
/// let mut vec = Vec::new();
/// push_entity!(MessageEntityBold(1) => vec);
/// push_entity!(MessageEntityPre(2, language = "rust".to_string()) => vec);
/// ```
macro_rules! push_entity {
    ( $ty:ident($offset:expr) => $vector:expr ) => {
        $vector.push(
            tl::types::$ty {
                offset: $offset,
                length: 0,
            }
            .into(),
        )
    };
    ( $ty:ident($offset:expr, $field:ident = $value:expr) => $vector:expr ) => {
        $vector.push(
            tl::types::$ty {
                offset: $offset,
                length: 0,
                $field: $value,
            }
            .into(),
        )
    };
}

/// Updates the length of the latest `MessageEntity` inside the specified vector.
///
/// # Examples
///
/// ```
/// let mut vec = Vec::new();
/// push_entity!(MessageEntityBold(1) => vec);
/// update_entity_len!(MessageEntityBold(2) => vec);
/// ```
macro_rules! update_entity_len {
    ( $ty:ident($end_offset:expr) => $vector:expr ) => {
        let mut remove = false;
        let end_offset = $end_offset;
        let pos = $vector.iter_mut().rposition(|e| match e {
            tl::enums::MessageEntity::$ty(e) => {
                e.length = end_offset - e.offset;
                remove = e.length == 0;
                true
            }
            _ => false,
        });

        if remove {
            $vector.remove(pos.unwrap());
        }
    };
}

#[cfg(feature = "markdown")]
pub fn parse_markdown_message(message: &str) -> (String, Vec<tl::enums::MessageEntity>) {
    use pulldown_cmark::{CodeBlockKind, Event, Parser, Tag};

    let mut text = String::with_capacity(message.len());
    let mut entities = Vec::new();

    let mut offset = 0;
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
            push_entity!(MessageEntityBold(offset) => entities);
        }
        Event::End(Tag::Strong) => {
            update_entity_len!(Bold(offset) => entities);
        }

        // *italic text*
        Event::Start(Tag::Emphasis) => {
            push_entity!(MessageEntityItalic(offset) => entities);
        }
        Event::End(Tag::Emphasis) => {
            update_entity_len!(Italic(offset) => entities);
        }

        // [text link](https://example.com)
        Event::Start(Tag::Link(_kind, url, _title)) => {
            push_entity!(MessageEntityTextUrl(offset, url = url.to_string()) => entities);
        }
        Event::End(Tag::Link(_kindd, _url, _title)) => {
            update_entity_len!(TextUrl(offset) => entities);
        }

        // ```lang\npre```
        Event::Start(Tag::CodeBlock(kind)) => {
            let lang = match kind {
                CodeBlockKind::Indented => "".to_string(),
                CodeBlockKind::Fenced(lang) => lang.to_string(),
            }
            .to_string();

            push_entity!(MessageEntityPre(offset, language = lang) => entities);
        }
        Event::End(Tag::CodeBlock(_kind)) => {
            update_entity_len!(Pre(offset) => entities);
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

#[cfg(feature = "html")]
pub fn parse_html_message(message: &str) -> (String, Vec<tl::enums::MessageEntity>) {
    use html5ever::tendril::StrTendril;
    use html5ever::tokenizer::{
        BufferQueue, Tag, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer,
    };

    // We could also convert the atoms we receive into lowercase strings and
    // match against those, but that would defeat the purpose. We do however
    // give the atoms we use better names.
    use html5ever::{
        ATOM_LOCALNAME__61 as TAG_A, ATOM_LOCALNAME__62 as TAG_B,
        ATOM_LOCALNAME__62_6C_6F_63_6B_71_75_6F_74_65 as TAG_BLOCKQUOTE,
        ATOM_LOCALNAME__63_6C_61_73_73 as ATTR_CLASS, ATOM_LOCALNAME__63_6F_64_65 as TAG_CODE,
        ATOM_LOCALNAME__64_65_6C as TAG_DEL, ATOM_LOCALNAME__65_6D as TAG_EM,
        ATOM_LOCALNAME__68_72_65_66 as ATTR_HREF, ATOM_LOCALNAME__69 as TAG_I,
        ATOM_LOCALNAME__70_72_65 as TAG_PRE, ATOM_LOCALNAME__73 as TAG_S,
        ATOM_LOCALNAME__73_74_72_6F_6E_67 as TAG_STRONG, ATOM_LOCALNAME__75 as TAG_U,
    };

    struct Sink {
        text: String,
        entities: Vec<tl::enums::MessageEntity>,
        offset: i32,
    }

    impl TokenSink for Sink {
        type Handle = ();

        fn process_token(&mut self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
            match token {
                Token::TagToken(Tag {
                    kind: TagKind::StartTag,
                    name,
                    self_closing: _,
                    attrs,
                }) => match name {
                    n if n == TAG_B || n == TAG_STRONG => {
                        push_entity!(MessageEntityBold(self.offset) => self.entities);
                    }
                    n if n == TAG_I || n == TAG_EM => {
                        push_entity!(MessageEntityItalic(self.offset) => self.entities);
                    }
                    n if n == TAG_S || n == TAG_DEL => {
                        push_entity!(MessageEntityStrike(self.offset) => self.entities);
                    }
                    TAG_U => {
                        push_entity!(MessageEntityUnderline(self.offset) => self.entities);
                    }
                    TAG_BLOCKQUOTE => {
                        push_entity!(MessageEntityBlockquote(self.offset) => self.entities);
                    }
                    TAG_CODE => {
                        match self.entities.iter_mut().rev().next() {
                            // If the previous tag is an open `<pre>`, don't add `<code>`;
                            // we most likely want to indicate `class="language-foo"`.
                            Some(tl::enums::MessageEntity::Pre(e)) if e.length == 0 => {
                                e.language = attrs
                                    .into_iter()
                                    .find(|a| {
                                        a.name.local == ATTR_CLASS
                                            && a.value.starts_with(CODE_LANG_PREFIX)
                                    })
                                    .map(|a| a.value[CODE_LANG_PREFIX.len()..].to_string())
                                    .unwrap_or_else(|| "".to_string());
                            }
                            _ => {
                                push_entity!(MessageEntityCode(self.offset) => self.entities);
                            }
                        }
                    }
                    TAG_PRE => {
                        push_entity!(MessageEntityPre(self.offset, language = "".to_string())
                            => self.entities);
                    }
                    TAG_A => {
                        let url = attrs
                            .into_iter()
                            .find(|a| a.name.local == ATTR_HREF)
                            .map(|a| a.value.to_string())
                            .unwrap_or_else(|| "".to_string());

                        push_entity!(MessageEntityTextUrl(self.offset, url = url)
                            => self.entities);
                    }
                    _ => {}
                },
                Token::TagToken(Tag {
                    kind: TagKind::EndTag,
                    name,
                    self_closing: _,
                    attrs: _,
                }) => match name {
                    n if n == TAG_B || n == TAG_STRONG => {
                        update_entity_len!(Bold(self.offset) => self.entities);
                    }
                    n if n == TAG_I || n == TAG_EM => {
                        update_entity_len!(Italic(self.offset) => self.entities);
                    }
                    n if n == TAG_S || n == TAG_DEL => {
                        update_entity_len!(Strike(self.offset) => self.entities);
                    }
                    TAG_U => {
                        update_entity_len!(Underline(self.offset) => self.entities);
                    }
                    TAG_BLOCKQUOTE => {
                        update_entity_len!(Blockquote(self.offset) => self.entities);
                    }
                    TAG_CODE => {
                        match self.entities.iter_mut().rev().next() {
                            // If the previous tag is an open `<pre>`, don't update `<code>` len;
                            // we most likely want to indicate `class="language-foo"`.
                            Some(tl::enums::MessageEntity::Pre(e)) if e.length == 0 => {}
                            _ => {
                                update_entity_len!(Code(self.offset) => self.entities);
                            }
                        }
                    }
                    TAG_PRE => {
                        update_entity_len!(Pre(self.offset) => self.entities);
                    }
                    TAG_A => {
                        update_entity_len!(TextUrl(self.offset) => self.entities);
                    }
                    _ => {}
                },
                Token::CharacterTokens(string) => {
                    self.text.push_str(&string);
                    self.offset += telegram_string_len(&string);
                }
                _ => {}
            }
            TokenSinkResult::Continue
        }
    }

    let mut input = BufferQueue::new();
    input.push_back(StrTendril::from_slice(message).try_reinterpret().unwrap());

    let mut tok = Tokenizer::new(
        Sink {
            text: String::with_capacity(message.len()),
            entities: Vec::new(),
            offset: 0,
        },
        Default::default(),
    );
    let _ = tok.feed(&mut input);
    tok.end();

    let Sink { text, entities, .. } = tok.sink;

    (text, entities)
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

    #[test]
    #[cfg(feature = "html")]
    fn parse_leading_html() {
        // Intentionally use different casing to make sure that is handled well
        let (text, entities) = parse_html_message("<B>Hello</b> world!");
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
    #[cfg(feature = "html")]
    fn parse_trailing_html() {
        let (text, entities) = parse_html_message("Hello <strong>world!</strong>");
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
    #[cfg(feature = "html")]
    fn parse_emoji_html() {
        let (text, entities) = parse_html_message("A <b>little 🦀</b> here");
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
    #[cfg(feature = "html")]
    fn parse_all_entities_html() {
        let (text, entities) = parse_html_message(
            "Some <b>bold</b> (<strong>strong</strong>), <i>italics</i> \
            (<em>cursive</em>), inline <code>code</code>, a <pre>pre</pre> \
            block, and <a href=\"https://example.com\">links</a>",
        );

        assert_eq!(
            text,
            "Some bold (strong), italics (cursive), inline code, a pre block, and links"
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
                    offset: 54,
                    length: 3,
                    language: "".to_string()
                }
                .into(),
                tl::types::MessageEntityTextUrl {
                    offset: 69,
                    length: 5,
                    url: "https://example.com".to_string()
                }
                .into(),
            ]
        );
    }

    #[test]
    #[cfg(feature = "html")]
    fn parse_pre_with_lang_html() {
        let (text, entities) = parse_html_message(
            "Some <pre>pre</pre>, <code>normal</code> and \
            <pre><code class=\"language-rust\">rusty</code></pre> code",
        );

        assert_eq!(text, "Some pre, normal and rusty code");
        assert_eq!(
            entities,
            vec![
                tl::types::MessageEntityPre {
                    offset: 5,
                    length: 3,
                    language: "".to_string()
                }
                .into(),
                tl::types::MessageEntityCode {
                    offset: 10,
                    length: 6,
                }
                .into(),
                tl::types::MessageEntityPre {
                    offset: 21,
                    length: 5,
                    language: "rust".to_string()
                }
                .into(),
            ]
        );
    }

    #[test]
    #[cfg(feature = "html")]
    fn parse_empty_pre_and_lang_html() {
        let (text, entities) = parse_html_message(
            "Some empty <pre></pre> and <code class=\"language-rust\">code</code>",
        );

        assert_eq!(text, "Some empty  and code");
        assert_eq!(
            entities,
            vec![tl::types::MessageEntityCode {
                offset: 16,
                length: 4,
            }
            .into(),]
        );
    }

    #[test]
    #[cfg(feature = "html")]
    fn parse_link_no_href_html() {
        let (text, entities) = parse_html_message("Some <a>empty link</a>, it does nothing");

        assert_eq!(text, "Some empty link, it does nothing");
        assert_eq!(
            entities,
            vec![tl::types::MessageEntityTextUrl {
                offset: 5,
                length: 10,
                url: "".to_string()
            }
            .into(),]
        );
    }

    #[test]
    #[cfg(feature = "html")]
    fn parse_nested_entities_html() {
        let (text, entities) = parse_html_message("Some <b>bold <i>both</b> italics</i>");
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
                    length: 12
                }
                .into(),
            ]
        );
    }
}
