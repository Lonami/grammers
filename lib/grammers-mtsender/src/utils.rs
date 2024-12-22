// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::time::Duration;
use web_time::Instant;

/// A cancellable timeout for web platforms.
/// It is simply a wrapper around `window.setTimeout` but also makes
/// sure to clear the timeout when dropped to avoid leaking timers.
#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
pub struct Timeout {
    handle: std::cell::OnceCell<i32>,
    inner: wasm_bindgen_futures::JsFuture,
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
impl std::future::Future for Timeout {
    type Output = ();

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        std::pin::Pin::new(&mut self.get_mut().inner)
            .poll(cx)
            .map(|_| ())
    }
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
impl Drop for Timeout {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.get() {
            web_sys::window()
                .unwrap()
                .clear_timeout_with_handle(*handle);
        }
    }
}

#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
impl Timeout {
    pub fn new(duration: Duration) -> Self {
        use wasm_bindgen_futures::js_sys;

        let handle = std::cell::OnceCell::new();
        let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
            handle
                .set(
                    web_sys::window()
                        .unwrap()
                        .set_timeout_with_callback_and_timeout_and_arguments_0(
                            &resolve,
                            duration.as_millis() as i32,
                        )
                        .unwrap(),
                )
                .expect("timeout already set");
        };

        let inner = wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut cb));
        Self { handle, inner }
    }
}

/// a web-friendly version of `tokio::time::sleep`
pub async fn sleep(duration: Duration) {
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    {
        return tokio::time::sleep(duration).await;
    }
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    {
        Timeout::new(duration).await;
    }
}

/// a web-friendly version of `tokio::time::sleep_until`
pub async fn sleep_until(deadline: Instant) {
    #[cfg(not(all(target_arch = "wasm32", target_os = "unknown")))]
    {
        return tokio::time::sleep_until(deadline.into()).await;
    }
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    {
        Timeout::new(deadline - Instant::now()).await;
    }
}
