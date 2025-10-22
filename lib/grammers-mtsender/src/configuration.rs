use std::sync::Arc;

use grammers_session::{Session, storages::TlSession};

const DEFAULT_LOCALE: &str = "en";

pub struct Configuration {
    pub api_id: i32,
    pub device_model: String,
    pub system_version: String,
    pub app_version: String,
    pub system_lang_code: String,
    pub lang_code: String,
    pub session: Arc<dyn Session>,
}

impl Default for Configuration {
    fn default() -> Self {
        let info = os_info::get();

        let mut system_lang_code = String::new();
        let mut lang_code = String::new();

        #[cfg(not(target_os = "android"))]
        {
            system_lang_code.push_str(&locate_locale::system());
            lang_code.push_str(&locate_locale::user());
        }
        if system_lang_code.is_empty() {
            system_lang_code.push_str(DEFAULT_LOCALE);
        }
        if lang_code.is_empty() {
            lang_code.push_str(DEFAULT_LOCALE);
        }

        Self {
            api_id: 0,
            device_model: format!("{} {}", info.os_type(), info.bitness()),
            system_version: info.version().to_string(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            system_lang_code,
            lang_code,
            session: Arc::new(TlSession::new()),
        }
    }
}
