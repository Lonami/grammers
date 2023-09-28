use std::ops::ControlFlow;
use std::time::Duration;

pub trait ReconnectionPolicy: Send + Sync {
    fn should_retry(&self, attempts: usize) -> ControlFlow<(), Duration>;
}

pub struct AlwaysReconnect;

impl ReconnectionPolicy for AlwaysReconnect {
    fn should_retry(&self, _: usize) -> ControlFlow<(), Duration> {
        ControlFlow::Continue(Duration::from_secs(0))
    }
}

unsafe impl Send for AlwaysReconnect {}
unsafe impl Sync for AlwaysReconnect {}
