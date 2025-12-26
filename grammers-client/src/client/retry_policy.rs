// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use std::num::NonZeroU32;
use std::ops::ControlFlow;
use std::time::Duration;

use grammers_mtsender::{InvocationError, RpcError};

pub trait RetryPolicy: Send + Sync {
    /// Determines whether the failing request should retry.
    ///
    /// If it should Continue, a sleep duration before retrying is included.
    /// If it should Break, the context error will be propagated to the caller.
    fn should_retry(&self, ctx: &RetryContext) -> ControlFlow<(), Duration>;
}

pub struct RetryContext {
    /// Amount of times the instance of this request has failed.
    pub fail_count: NonZeroU32,
    /// Sum of the durations for all previous continuations (not total time elapsed since first failure).
    pub slept_so_far: Duration,
    /// The most recent error caused by the instance of the request.
    pub error: InvocationError,
}

/// Retry policy that will never retry.
pub struct NoRetries;

impl RetryPolicy for NoRetries {
    fn should_retry(&self, _: &RetryContext) -> ControlFlow<(), Duration> {
        ControlFlow::Break(())
    }
}

/// Retry policy that will retry *once* on flood-wait and slow mode wait errors,
/// if the duration to sleep for is below the threshold.
pub struct AutoSleep {
    /// The threshold below which the library should automatically sleep. For instance, if an
    /// `RpcError { name: "FLOOD_WAIT", value: Some(17) }` (flood, must wait 17 seconds) occurs
    /// and `flood_sleep_threshold` is 20 (seconds), the library will `sleep` automatically for
    /// 17 seconds. If the error was for 21s, it would propagate the error instead.
    pub threshold: Duration,

    /// `Some` if I/O errors should be treated as a flood error that would last the specified duration.
    /// This duration will ignore the `threshold` and always be slept on on the first I/O error.
    pub io_errors_as_flood_of: Option<Duration>,
}

impl RetryPolicy for AutoSleep {
    fn should_retry(&self, ctx: &RetryContext) -> ControlFlow<(), Duration> {
        match ctx.error {
            InvocationError::Rpc(RpcError {
                code: 420,
                value: Some(seconds),
                ..
            }) if ctx.fail_count.get() == 1 && seconds as u64 <= self.threshold.as_secs() => {
                ControlFlow::Continue(Duration::from_secs(seconds as _))
            }
            InvocationError::Io(_) if ctx.fail_count.get() == 1 => {
                if let Some(duration) = self.io_errors_as_flood_of {
                    ControlFlow::Continue(duration)
                } else {
                    ControlFlow::Break(())
                }
            }
            _ => ControlFlow::Break(()),
        }
    }
}
