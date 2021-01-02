// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use futures::stream::{self, StreamExt};
use html5ever::tendril::StrTendril;
use html5ever::tokenizer::{
    BufferQueue, Tag, TagKind, Token, TokenSink, TokenSinkResult, Tokenizer,
};
use std::collections::BTreeMap;
use std::mem;

const CONCURRENCY: usize = 16;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

macro_rules! make_tag_consts {
    ( $( $tag:tt => $constant:ident ),* $(,)? ) => {
        $(
            const $constant: string_cache::Atom<html5ever::LocalNameStaticSet> =
                html5ever::local_name!($tag);
        )*
    };
}

fn parse_html<T: TokenSink>(body: &str, sink: T) -> T {
    let mut input = BufferQueue::new();
    input.push_back(StrTendril::from_slice(body).try_reinterpret().unwrap());
    let mut tok = Tokenizer::new(sink, Default::default());
    let _ = tok.feed(&mut input);
    tok.end();
    tok.sink
}

// TODO add tests for extraction without hitting the network
// TODO allow passing urls as input args, to easily retry on just whatever failed

async fn real_main() -> Result<()> {
    make_tag_consts!(
        "a" => TAG_A,
        "class" => ATTR_CLASS,
        "div" => TAG_DIV,
        "h3" => TAG_H3,
        "href" => ATTR_HREF,
        "id" => ATTR_ID,
        "pre" => TAG_PRE,
        "p" => TAG_P,
        "td" => TAG_TD,
        "tr" => TAG_TR,
    );

    let body = reqwest::get("https://core.telegram.org/schema")
        .await?
        .text()
        .await?;

    struct Sink {
        active: bool,
        current_url: Option<String>,
        current_text: Option<String>,
        results: BTreeMap<String, String>,
    }

    impl TokenSink for Sink {
        type Handle = ();

        fn process_token(&mut self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
            match token {
                Token::TagToken(Tag {
                    kind: TagKind::StartTag,
                    name: TAG_PRE,
                    self_closing: _,
                    attrs,
                }) if attrs.iter().any(|a| a.name.local == ATTR_CLASS) => {
                    self.active = true;
                }
                Token::TagToken(Tag {
                    kind: TagKind::EndTag,
                    name: TAG_PRE,
                    self_closing: _,
                    attrs: _,
                }) => {
                    self.active = false;
                }
                Token::TagToken(Tag {
                    kind: TagKind::StartTag,
                    name: TAG_A,
                    self_closing: _,
                    attrs,
                }) if self.active => {
                    self.current_url = Some(
                        attrs
                            .into_iter()
                            .find(|a| a.name.local == ATTR_HREF)
                            .map(|a| a.value.to_string())
                            .unwrap(),
                    );

                    self.current_text = Some(String::new());
                }
                Token::TagToken(Tag {
                    kind: TagKind::EndTag,
                    name: TAG_A,
                    self_closing: _,
                    attrs: _,
                }) if self.active => {
                    self.results.insert(
                        self.current_text.take().unwrap(),
                        self.current_url.take().unwrap(),
                    );
                }
                Token::CharacterTokens(string) if self.current_text.is_some() => {
                    self.current_text.as_mut().unwrap().push_str(&string);
                }
                _ => {}
            }
            TokenSinkResult::Continue
        }
    }

    let names_to_url = parse_html(
        &body,
        Sink {
            active: false,
            current_url: None,
            current_text: None,
            results: BTreeMap::new(),
        },
    )
    .results;

    #[derive(Debug, serde::Serialize)]
    struct TlError {
        code: i32,
        description: String,
    }

    #[derive(Debug, serde::Serialize)]
    struct Documentation {
        description: String,
        parameters: BTreeMap<String, String>,
        errors: BTreeMap<String, TlError>,
    }

    #[derive(Debug, serde::Serialize)]
    struct Item {
        name: String,
        url_path: String,
        documentation: Documentation,
    }

    async fn process_item(tuple: (String, String)) -> std::result::Result<Item, (String, String)> {
        let (name, url_path) = tuple.clone();

        let mut url = "https://core.telegram.org".to_string();
        url.push_str(&url_path);

        let body = reqwest::get(&url)
            .await
            .map_err(|_| tuple.clone())?
            .text()
            .await
            .map_err(|_| tuple.clone())?;

        #[derive(Debug, PartialEq, Eq)]
        enum State {
            Wait,
            Description,
            Parameters,
            ParameterName {
                name: String,
            },
            ParameterType {
                name: String,
            },
            ParameterDesc {
                name: String,
                desc: String,
            },
            Errors,
            ErrorCode {
                code: String,
            },
            ErrorName {
                code: String,
                name: String,
            },
            ErrorDesc {
                code: String,
                name: String,
                desc: String,
            },
        }

        struct Sink {
            state: State,
            result: Documentation,
        }

        impl TokenSink for Sink {
            type Handle = ();

            fn process_token(&mut self, token: Token, _line_number: u64) -> TokenSinkResult<()> {
                match token {
                    // Starting on a new state to read data.
                    Token::TagToken(Tag {
                        kind: TagKind::StartTag,
                        name: TAG_DIV,
                        self_closing: _,
                        attrs,
                    }) if attrs.iter().any(|a| {
                        a.name.local == ATTR_ID && (&a.value as &str) == "dev_page_content"
                    }) =>
                    {
                        self.state = State::Description;
                    }
                    Token::TagToken(Tag {
                        kind: TagKind::StartTag,
                        name: TAG_A,
                        self_closing: _,
                        attrs,
                    }) => {
                        if let Some(attr) = attrs.iter().find(|a| a.name.local == ATTR_ID) {
                            let id = &attr.value as &str;
                            if id == "parameters" {
                                self.state = State::Parameters;
                            } else if id == "possible-errors" {
                                self.state = State::Errors;
                            }
                        }
                    }

                    Token::TagToken(Tag {
                        kind: TagKind::StartTag,
                        name: TAG_TD,
                        self_closing: _,
                        attrs: _,
                    }) => {
                        self.state = match mem::replace(&mut self.state, State::Wait) {
                            State::Parameters => State::ParameterName {
                                name: String::new(),
                            },
                            State::ParameterName { name } => State::ParameterType { name },
                            State::ParameterType { name } => State::ParameterDesc {
                                name,
                                desc: String::new(),
                            },
                            State::Errors => State::ErrorCode {
                                code: String::new(),
                            },
                            State::ErrorCode { code } => State::ErrorName {
                                code,
                                name: String::new(),
                            },
                            State::ErrorName { code, name } => State::ErrorDesc {
                                code,
                                name,
                                desc: String::new(),
                            },
                            s => s,
                        };
                    }

                    // Reading data.
                    Token::CharacterTokens(string) => match &mut self.state {
                        State::Description => {
                            self.result.description.push_str(&string);
                        }
                        State::ParameterName { name } => {
                            name.push_str(&string);
                        }
                        State::ParameterDesc { desc, .. } => {
                            desc.push_str(&string);
                        }
                        State::ErrorCode { code } => {
                            code.push_str(&string);
                        }
                        State::ErrorName { name, .. } => {
                            name.push_str(&string);
                        }
                        State::ErrorDesc { desc, .. } => {
                            desc.push_str(&string);
                        }
                        _ => {}
                    },

                    // Detecting when a state ends.
                    Token::TagToken(Tag {
                        kind: TagKind::StartTag,
                        name,
                        self_closing: _,
                        attrs: _,
                    }) if self.state == State::Description && name != TAG_P => {
                        self.state = State::Wait;
                    }

                    Token::TagToken(Tag {
                        kind: TagKind::EndTag,
                        name: TAG_TR,
                        self_closing: _,
                        attrs: _,
                    }) => {
                        self.state = match mem::replace(&mut self.state, State::Wait) {
                            State::ParameterDesc { name, desc } => {
                                self.result.parameters.insert(name.trim().to_string(), desc);
                                State::Parameters
                            }
                            State::ErrorDesc { code, name, desc } => {
                                self.result.errors.insert(
                                    name.trim().to_string(),
                                    TlError {
                                        code: code.trim().parse().unwrap_or(-1),
                                        description: desc,
                                    },
                                );
                                State::Errors
                            }
                            s => s,
                        }
                    }

                    Token::TagToken(Tag {
                        kind: TagKind::StartTag,
                        name: TAG_H3,
                        self_closing: _,
                        attrs: _,
                    }) => {
                        self.state = State::Wait;
                    }

                    _ => {}
                }
                TokenSinkResult::Continue
            }
        }

        let documentation = parse_html(
            &body,
            Sink {
                state: State::Wait,
                result: Documentation {
                    description: String::new(),
                    parameters: BTreeMap::new(),
                    errors: BTreeMap::new(),
                },
            },
        )
        .result;

        Ok(Item {
            name,
            url_path,
            documentation,
        })
    }

    let mut i = 0usize;
    let total = names_to_url.len();
    let mut items: Vec<Item> = Vec::with_capacity(total);
    let mut retry = Vec::new();

    let mut buffered =
        stream::iter(names_to_url.into_iter().map(process_item)).buffer_unordered(CONCURRENCY);

    while let Some(result) = buffered.next().await {
        i += 1;
        match result {
            Ok(item) => {
                eprintln!("[{:04}/{:04}] OK: {}", i, total, item.url_path);
                items.push(item);
            }
            Err(tuple) => {
                eprintln!("[{:04}/{:04}] ERROR: {}", i, total, tuple.1);
                retry.push(tuple);
            }
        }
    }

    // Retry the requests that failed when running concurrently, but one by one this time.
    if !retry.is_empty() {
        let mut i = 0usize;
        let total = retry.len();
        eprintln!("Retrying {} failed URLs...", total);
        for tuple in retry {
            i += 1;
            match process_item(tuple).await {
                Ok(item) => {
                    eprintln!("[{:04}/{:04}] OK: {}", i, total, item.url_path);
                    items.push(item);
                }
                Err(tuple) => {
                    eprintln!("[{:04}/{:04}] ERROR: {}", i, total, tuple.1);
                }
            }
        }
    }

    println!("{}", serde_json::to_string(&items)?);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Making errors (unbalanced blocks) inside a `tokio::main` produces confusing diagnostics.
    // So the "real main" is wrapped by this.
    real_main().await
}
