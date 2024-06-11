// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use std::ops::ControlFlow;
use std::time::Duration;

/// a simple **Reconnection** Handler.
///
/// with implementing this trait and passing it to the `InitParams` inside the `Client` you can have your own
/// custom implementations for handling connection failures.
///
/// the default implementation is **NoReconnect** which does not handle anything! there is also a `FixedReconnect`
/// which sets a fixed attempt count and a duration
///
/// note that this will return a `ControlFlow<(), Duration>` which tells the handler either `Break` the Connection Attempt *or*
/// `Continue` After the Given `Duration`
pub trait ReconnectionPolicy: Send + Sync {
    ///this function will indicate that the handler should attempt for a new *reconnection* or not.
    ///
    /// it accepts a `attempts` which is the amount of reconnection tries that has been made already
    fn should_retry(&self, attempts: usize) -> ControlFlow<(), Duration>;
}

/// the default implementation of the **ReconnectionPolicy**.
pub struct NoReconnect;

/// simple *Fixed* sized implementation for the **ReconnectionPolicy** trait.
pub struct FixedReconnect {
    pub attempts: usize,
    pub delay: Duration,
}

impl ReconnectionPolicy for FixedReconnect {
    fn should_retry(&self, attempts: usize) -> ControlFlow<(), Duration> {
        if attempts <= self.attempts {
            ControlFlow::Continue(self.delay)
        } else {
            ControlFlow::Break(())
        }
    }
}

impl ReconnectionPolicy for NoReconnect {
    fn should_retry(&self, _: usize) -> ControlFlow<(), Duration> {
        ControlFlow::Break(())
    }
}
