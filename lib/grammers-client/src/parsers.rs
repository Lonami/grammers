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
        ATOM_LOCALNAME__63_6F_64_65 as TAG_CODE, ATOM_LOCALNAME__64_65_6C as TAG_DEL,
        ATOM_LOCALNAME__65_6D as TAG_EM, ATOM_LOCALNAME__68_72_65_66 as ATTR_HREF,
        ATOM_LOCALNAME__69 as TAG_I, ATOM_LOCALNAME__70_72_65 as TAG_PRE,
        ATOM_LOCALNAME__73 as TAG_S, ATOM_LOCALNAME__73_74_72_6F_6E_67 as TAG_STRONG,
        ATOM_LOCALNAME__75 as TAG_U,
    };

    struct Sink {
        text: String,
        entities: Vec<tl::enums::MessageEntity>,
        offset: i32,
        live_entities: Vec<tl::enums::MessageEntity>,
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
                        self.live_entities.push(
                            tl::types::MessageEntityBold {
                                offset: self.offset,
                                length: 0,
                            }
                            .into(),
                        );
                    }
                    n if n == TAG_I || n == TAG_EM => {
                        self.live_entities.push(
                            tl::types::MessageEntityItalic {
                                offset: self.offset,
                                length: 0,
                            }
                            .into(),
                        );
                    }
                    n if n == TAG_S || n == TAG_DEL => {
                        self.live_entities.push(
                            tl::types::MessageEntityStrike {
                                offset: self.offset,
                                length: 0,
                            }
                            .into(),
                        );
                    }
                    TAG_U => {
                        self.live_entities.push(
                            tl::types::MessageEntityUnderline {
                                offset: self.offset,
                                length: 0,
                            }
                            .into(),
                        );
                    }
                    TAG_BLOCKQUOTE => {
                        self.live_entities.push(
                            tl::types::MessageEntityBlockquote {
                                offset: self.offset,
                                length: 0,
                            }
                            .into(),
                        );
                    }
                    TAG_CODE => {
                        self.live_entities.push(
                            tl::types::MessageEntityCode {
                                offset: self.offset,
                                length: 0,
                            }
                            .into(),
                        );
                    }
                    TAG_PRE => {
                        dbg!(attrs);
                        self.live_entities.push(
                            tl::types::MessageEntityPre {
                                offset: self.offset,
                                length: 0,
                                language: "".into(),
                            }
                            .into(),
                        );
                    }
                    TAG_A => {
                        self.live_entities.push(
                            tl::types::MessageEntityTextUrl {
                                offset: self.offset,
                                length: 0,
                                url: attrs
                                    .into_iter()
                                    .find(|a| a.name.local == ATTR_HREF)
                                    .map(|a| a.value)
                                    .unwrap()
                                    .to_string(),
                            }
                            .into(),
                        );
                    }
                    _ => {}
                },
                Token::TagToken(Tag {
                    kind: TagKind::EndTag,
                    name,
                    self_closing: _,
                    attrs: _,
                }) => {
                    // TODO We probably should use a macro or something to get rid of this mess
                    match name {
                        n if n == TAG_B || n == TAG_STRONG => {
                            let offset = self.offset;
                            let index = self
                                .live_entities
                                .iter_mut()
                                .rposition(|e| match e {
                                    tl::enums::MessageEntity::MessageEntityBold(e) => {
                                        e.length = offset - e.offset;
                                        true
                                    }
                                    _ => false,
                                })
                                .unwrap();

                            self.entities.push(self.live_entities.remove(index));
                        }
                        n if n == TAG_I || n == TAG_EM => {
                            let offset = self.offset;
                            let index = self
                                .live_entities
                                .iter_mut()
                                .rposition(|e| match e {
                                    tl::enums::MessageEntity::MessageEntityItalic(e) => {
                                        e.length = offset - e.offset;
                                        true
                                    }
                                    _ => false,
                                })
                                .unwrap();

                            self.entities.push(self.live_entities.remove(index));
                        }
                        n if n == TAG_S || n == TAG_DEL => {
                            let offset = self.offset;
                            let index = self
                                .live_entities
                                .iter_mut()
                                .rposition(|e| match e {
                                    tl::enums::MessageEntity::MessageEntityStrike(e) => {
                                        e.length = offset - e.offset;
                                        true
                                    }
                                    _ => false,
                                })
                                .unwrap();

                            self.entities.push(self.live_entities.remove(index));
                        }
                        TAG_U => {
                            let offset = self.offset;
                            let index = self
                                .live_entities
                                .iter_mut()
                                .rposition(|e| match e {
                                    tl::enums::MessageEntity::MessageEntityUnderline(e) => {
                                        e.length = offset - e.offset;
                                        true
                                    }
                                    _ => false,
                                })
                                .unwrap();

                            self.entities.push(self.live_entities.remove(index));
                        }
                        TAG_BLOCKQUOTE => {
                            let offset = self.offset;
                            let index = self
                                .live_entities
                                .iter_mut()
                                .rposition(|e| match e {
                                    tl::enums::MessageEntity::MessageEntityBlockquote(e) => {
                                        e.length = offset - e.offset;
                                        true
                                    }
                                    _ => false,
                                })
                                .unwrap();

                            self.entities.push(self.live_entities.remove(index));
                        }
                        TAG_CODE => {
                            let offset = self.offset;
                            let index = self
                                .live_entities
                                .iter_mut()
                                .rposition(|e| match e {
                                    tl::enums::MessageEntity::MessageEntityCode(e) => {
                                        e.length = offset - e.offset;
                                        true
                                    }
                                    _ => false,
                                })
                                .unwrap();

                            self.entities.push(self.live_entities.remove(index));
                        }
                        TAG_PRE => {
                            let offset = self.offset;
                            let index = self
                                .live_entities
                                .iter_mut()
                                .rposition(|e| match e {
                                    tl::enums::MessageEntity::MessageEntityPre(e) => {
                                        e.length = offset - e.offset;
                                        true
                                    }
                                    _ => false,
                                })
                                .unwrap();

                            self.entities.push(self.live_entities.remove(index));
                        }
                        TAG_A => {
                            let offset = self.offset;
                            let index = self
                                .live_entities
                                .iter_mut()
                                .rposition(|e| match e {
                                    tl::enums::MessageEntity::MessageEntityTextUrl(e) => {
                                        e.length = offset - e.offset;
                                        true
                                    }
                                    _ => false,
                                })
                                .unwrap();

                            self.entities.push(self.live_entities.remove(index));
                        }
                        _ => {}
                    }
                }
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
            live_entities: Vec::new(),
        },
        Default::default(),
    );
    let _ = tok.feed(&mut input);
    tok.end();

    let Sink {
        text, mut entities, ..
    } = tok.sink;

    // When nesting entities, we will close the inner one before the outer one,
    // but we want to have them sorted by offset.
    entities.sort_by(|a, b| entity_offset(a).cmp(&entity_offset(b)));

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
        let (text, entities) =
            parse_html_message("Some <pre><code class=\"language-rust\">rusty</code></pre> code");

        assert_eq!(text, "Some rusty code");
        assert_eq!(
            entities,
            vec![tl::types::MessageEntityPre {
                offset: 5,
                length: 5,
                language: "rust".to_string()
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
