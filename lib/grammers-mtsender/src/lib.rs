// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![deny(unsafe_code)]

mod configuration;
mod errors;
mod net;
mod reconnection;
mod sender;
mod sender_pool;
pub mod utils;

pub use crate::reconnection::*;
pub use configuration::Configuration;
pub use errors::{AuthorizationError, InvocationError, ReadError, RpcError};
pub use net::ServerAddr;
pub use sender::{Enqueuer, Sender, connect, connect_with_auth, generate_auth_key};
pub use sender_pool::{SenderPool, SenderPoolHandle};
