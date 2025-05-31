// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console

mod backend;
mod bridge;
mod config;
mod frontend;

use bridge::{BackendMessage, FrontendMessage};
use simple_logger::SimpleLogger;
use std::error::Error;
use tokio::sync::mpsc;

fn main() -> Result<(), Box<dyn Error>> {
    SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

    let (backend_sender, frontend_receiver) = mpsc::unbounded_channel::<BackendMessage>();
    let (frontend_sender, backend_receiver) = mpsc::unbounded_channel::<FrontendMessage>();

    let _backend_handle = slint::spawn_local(async_compat::Compat::new(async move {
        let _ = backend::main(bridge::BackendContext {
            backend_sender,
            backend_receiver,
        })
        .await;
        let _ = slint::quit_event_loop();
    }))
    .expect("backend task to start");

    frontend::main(bridge::FrontendContext {
        frontend_receiver,
        frontend_sender,
    })
    .map_err(|e| e.into())
}
