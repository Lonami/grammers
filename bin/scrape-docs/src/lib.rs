// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICEjNSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use futures::stream::{self, StreamExt};
use regex::Regex;
use reqwest::{Client as ReqwestClient, Url};
use select::document::Document;
use select::node::Node;
use select::predicate::{Attr, Element, Name};
use std::collections::{BTreeMap, HashMap};

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Debug, serde::Serialize)]
pub struct TlError {
    code: i32,
    description: String,
}

#[derive(Debug, serde::Serialize)]
pub struct Documentation {
    description: String,
    parameters: BTreeMap<String, String>,
    errors: BTreeMap<String, TlError>,
}

#[derive(Debug, serde::Serialize)]
pub struct DocumentationItem {
    name: String,
    url_path: String,
    documentation: Documentation,
}

pub struct Scraper {
    client: ReqwestClient,
    concurrency: usize,
    base_url: Url,
    layer_number: Option<String>,
}
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
pub struct ScraperConfig {
    pub client: Option<ReqwestClient>,
    pub concurrency: usize,
    pub base_url: Url,
    pub layer_number: Option<String>,
}
impl Scraper {
    async fn process_item(
        &self,
        tuple: (String, String),
    ) -> std::result::Result<DocumentationItem, (String, String)> {
        let (name, url_path) = tuple.clone();

        let body = self
            .send_request(&url_path)
            .await
            .map_err(|_| tuple.clone())?;

        let documentation = self.extract_item_docs(body);

        Ok(DocumentationItem {
            name,
            url_path: format!("{}{}", self.base_url, url_path),
            documentation,
        })
    }
    pub fn new(conf: ScraperConfig) -> Scraper {
        Self {
            client: conf.client.unwrap_or_default(),
            concurrency: conf.concurrency,
            base_url: conf.base_url,
            layer_number: conf.layer_number,
        }
    }
    fn extract_item_docs(&self, body: String) -> Documentation {
        let re = Regex::new(r#"href="(?P<p>/.+)""#).unwrap();
        let body = re.replace_all(&body, format!(r#"href="{}$p""#, self.base_url));
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
                documentation.description.push_str(&elem.inner_html());
                documentation.description.push('\n');
            });
        documentation.description = documentation.description.trim().to_string();
        iter_table(&doc, "parameters", 3, |chunk| {
            documentation
                .parameters
                .insert(chunk[0].text(), chunk[2].inner_html());
        });
        iter_table(&doc, "possible-errors", 3, |chunk| {
            documentation.errors.insert(
                chunk[1].text(),
                TlError {
                    code: chunk[0].text().parse().unwrap(),
                    description: chunk[2].inner_html(),
                },
            );
        });
        documentation
    }

    pub async fn scrape(&self) -> Result<Vec<DocumentationItem>> {
        let body = self.send_request("/schema").await?;

        let names_to_url = Self::get_schema_links(body);

        let mut i = 0usize;
        let total = names_to_url.len();
        let mut items: Vec<DocumentationItem> = Vec::with_capacity(total);
        let mut retry = Vec::new();
        let mut buffered = stream::iter(names_to_url.into_iter().map(|e| self.process_item(e)))
            .buffer_unordered(self.concurrency);
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
        if !retry.is_empty() {
            let mut i = 0usize;
            let total = retry.len();
            eprintln!("Retrying {} failed URLs...", total);
            for tuple in retry {
                i += 1;
                match self.process_item(tuple).await {
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
        Ok(items)
    }

    async fn send_request(&self, path: &str) -> Result<String> {
        let mut request = self.client.get(format!("{}{}", self.base_url, path));

        if let Some(layer) = &self.layer_number {
            request = request.header("Cookie", format!("stel_dev_layer={}", layer));
        }

        let body = request.send().await?.text().await?;
        Ok(body)
    }

    fn get_schema_links(body: String) -> HashMap<String, String> {
        let doc = Document::from(body.as_ref());
        let pre = doc.find(Name("pre")).next().unwrap();
        let names_to_url = pre
            .find(Name("a"))
            .map(|a| (a.text(), a.attr("href").unwrap().to_string()))
            .collect::<HashMap<String, String>>();
        names_to_url
    }
}
