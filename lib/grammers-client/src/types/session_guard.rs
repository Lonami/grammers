use crate::Config;
use grammers_session::Session;
use std::ops::{Deref, DerefMut};
use std::sync::MutexGuard;

/// Custom [`MutexGuard`] for [`Session`]
///
/// [`Session`]: grammers_session::Session
/// [`MutexGuard`]: std::sync::MutexGuard
pub struct SessionGuard<'a> {
    inner: MutexGuard<'a, Config>,
}

impl<'a> SessionGuard<'a> {
    pub(crate) fn new(config: MutexGuard<'a, Config>) -> Self {
        Self { inner: config }
    }
}

impl Deref for SessionGuard<'_> {
    type Target = Session;

    fn deref(&self) -> &Session {
        &self.inner.deref().session
    }
}

impl DerefMut for SessionGuard<'_> {
    fn deref_mut(&mut self) -> &mut Session {
        &mut self.inner.deref_mut().session
    }
}
