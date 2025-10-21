use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

const DEFAULT_LOCALE: &str = "en";

/// Hardcoded known `static` options from `functions::help::GetConfig`.
const KNOWN_DC_OPTIONS: [DcOption; 5] = [
    DcOption {
        id: 1,
        address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(149, 154, 175, 53), 443)),
        auth_key: None,
    },
    DcOption {
        id: 2,
        address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(149, 154, 167, 51), 443)),
        auth_key: None,
    },
    DcOption {
        id: 3,
        address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(149, 154, 175, 100), 443)),
        auth_key: None,
    },
    DcOption {
        id: 4,
        address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(149, 154, 167, 92), 443)),
        auth_key: None,
    },
    DcOption {
        id: 5,
        address: SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(91, 108, 56, 190), 443)),
        auth_key: None,
    },
];

#[derive(Clone)]
pub struct Configuration {
    pub api_id: i32,
    pub device_model: String,
    pub system_version: String,
    pub app_version: String,
    pub system_lang_code: String,
    pub lang_code: String,
    pub dc_options: Vec<DcOption>,
}

#[derive(Clone)]
pub struct DcOption {
    pub id: i32,
    pub address: SocketAddr,
    pub auth_key: Option<[u8; 256]>,
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
            dc_options: KNOWN_DC_OPTIONS.to_vec(),
        }
    }
}
