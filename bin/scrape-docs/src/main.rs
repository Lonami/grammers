// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use futures::stream::{self, StreamExt};
use select::document::Document;
use select::node::Node;
use select::predicate::{Attr, Element, Name};
use std::collections::{BTreeMap, HashMap};

const CONCURRENCY: usize = 16;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

// TODO add tests for extraction without hitting the network

fn iter_table<F: FnMut(&[Node])>(doc: &Document, id: &str, cols: usize, func: F) {
    if let Some(a) = doc.find(Attr("id", id)).next() {
        let mut elem = a.parent().unwrap();
        loop {
            elem = elem.next().unwrap();
            // Might get sme stray `Text` before the table.
            if elem.is(Element) {
                if elem.is(Name("table")) {
                    elem.find(Name("td"))
                        .collect::<Vec<_>>()
                        .chunks(cols)
                        .for_each(func);
                }
                break;
            }
        }
    }
}

async fn real_main() -> Result<()> {
    let body = reqwest::get("https://core.telegram.org/schema")
        .await?
        .text()
        .await?;

    let doc = Document::from(body.as_ref());
    let pre = doc.find(Name("pre")).next().unwrap();

    let names_to_url = pre
        .find(Name("a"))
        .map(|a| (a.text(), a.attr("href").unwrap().to_string()))
        .collect::<HashMap<String, String>>();

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

        let doc = Document::from(body.as_ref());
        let mut documentation = Documentation {
            description: String::new(),
            parameters: BTreeMap::new(),
            errors: BTreeMap::new(),
        };

        doc.find(Attr("id", "dev_page_content"))
            .next()
            .unwrap()
            .children()
            .take_while(|elem| elem.is(Name("p")))
            .for_each(|elem| {
                documentation.description.push_str(&elem.text());
                documentation.description.push('\n');
            });
        documentation.description = documentation.description.trim().to_string();

        iter_table(&doc, "parameters", 3, |chunk| {
            documentation
                .parameters
                .insert(chunk[0].text(), chunk[2].text());
        });
        iter_table(&doc, "possible-errors", 3, |chunk| {
            documentation.errors.insert(
                chunk[1].text(),
                TlError {
                    code: chunk[0].text().parse().unwrap(),
                    description: chunk[2].text(),
                },
            );
        });

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
        eprintln!("Retrying {total} failed URLs...");
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

    items.sort_by_key(|item| item.url_path.clone());
    println!("{}", serde_json::to_string(&items)?);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    // Making errors (unbalanced blocks) inside a `tokio::main` produces confusing diagnostics.
    // So the "real main" is wrapped by this.
    real_main().await
}
