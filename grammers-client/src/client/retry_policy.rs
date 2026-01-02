// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Retry policy trait and built-in policies for use in [`ClientConfiguration`].
//!
//! [`ClientConfiguration`]: crate::ClientConfiguration

use std::num::NonZeroU32;
use std::ops::ControlFlow;
use std::time::Duration;

use grammers_mtsender::{InvocationError, RpcError};

/// This trait controls how the [`Client`] should behave when
/// an invoked request fails with an [`InvocationError`].
///
/// [`Client`]: crate::Client
pub trait RetryPolicy: Send + Sync {
    /// Determines whether the failing request should retry.
    ///
    /// If it should Continue, a sleep duration before retrying is included.\
    /// If it should Break, the context error will be propagated to the caller.
    fn should_retry(&self, ctx: &RetryContext) -> ControlFlow<(), Duration>;
}

/// Context passed to [`RetryPolicy::should_retry`].
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

/// Retry policy that will retry *once* on flood-wait and slow mode wait errors.
///
/// The library will sleep only if the duration to sleep for is below or equal to the threshold.
pub struct AutoSleep {
    /// The (inclusive) threshold below which the library should automatically sleep.
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

impl Default for AutoSleep {
    /// Returns an instance with a threshold of 60 seconds.
    ///
    /// I/O errors will be treated as if they were a 1-second flood.
    fn default() -> Self {
        Self {
            threshold: Duration::from_secs(60),
            io_errors_as_flood_of: Some(Duration::from_secs(1)),
        }
    }
}
