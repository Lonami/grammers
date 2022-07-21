// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use clap::StructOpt;
use reqwest::Url;
use scrape_docs::{Result, Scraper, ScraperConfig};

#[derive(Debug, clap::Parser)]
#[clap(author, version, about, long_about = None)]
struct Arguments {
    #[clap(short,long,default_value_t = String::from("https://core.telegram.org"))]
    uri: String,
    #[clap(short, long)]
    layer: Option<String>,
    #[clap(short, long, default_value_t = 16)]
    concurrency: usize,
}
#[tokio::main]
async fn main() -> Result<()> {
    // Making errors (unbalanced blocks) inside a `tokio::main` produces confusing diagnostics.
    // So the "real main" is wrapped by this.
    real_main().await
}
async fn real_main() -> Result<()> {
    let args = Arguments::parse();
    eprintln!("running with inputs : {:?}", &args);
    let scraper = Scraper::new(ScraperConfig {
        base_url: Url::parse(&args.uri).expect("invalid url"),
        concurrency: args.concurrency,
        client: None,
        layer_number: None,
    });
    let items = scraper.scrape().await?;
    println!("{}", serde_json::to_string(&items)?);
    Ok(())
}
