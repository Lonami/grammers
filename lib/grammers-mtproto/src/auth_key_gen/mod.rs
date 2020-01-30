//! Contains the steps required to generate an authorization key.
//!
//! # Examples
//!
//! ```no_run
//! use std::io::Result;
//! use grammers_mtproto::auth_key_gen;
//!
//! fn send_data_to_server(request: &[u8]) -> Result<Vec<u8>> {
//!     unimplemented!()
//! }
//! 
//! fn main() -> Result<()> {
//!     let (request, data) = auth_key_gen::step1()?;
//!     let response = send_data_to_server(&request)?;
//!
//!     let (request, data) = auth_key_gen::step2(data, response)?;
//!     let response = send_data_to_server(&request)?;
//!
//!     let (request, data) = auth_key_gen::step3(data, response)?;
//!     let response = send_data_to_server(&request)?;
//!
//!     let (auth_key, time_offset) = auth_key_gen::create_key(data, response)?;
//!     // Now you have a secure `auth_key` to send encrypted messages to server.
//!     Ok(())
//! }
//! ```
mod generation;

pub use generation::{create_key, step1, step2, step3};
