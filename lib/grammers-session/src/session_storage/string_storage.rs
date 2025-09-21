use std::sync::{Arc, Mutex};
use base64::Engine;
use snafu::{Snafu, prelude::*};
use grammers_tl_types::{Deserializable, Serializable};
use crate::session_storage::{SessionProvider, SessionProviderError};
use crate::session_storage::error::{DecodeStringSnafu, InvalidFormatSnafu};

/// String-based session storage implementation
///
/// This structure provides session data storage in the form of a base64-encoded string.
pub struct StringSessionStorage {
    /// String for storing encoded session data
    string: Arc<Mutex<String>>,

    /// Underlying session
    session: Arc<Mutex<crate::types::Session>>
}

/// Options for [`StringSessionStorage::new`]
pub struct StringSessionStorageOptions {
    /// Optional storage string
    ///
    /// If not provided, an empty string will be created
    pub string: Option<Arc<Mutex<String>>>,

    /// Optional initial session
    ///
    /// If not provided, an empty session with default values will be created
    pub session: Option<Arc<Mutex<crate::types::Session>>>
}

impl StringSessionStorage {
    pub fn new(options: StringSessionStorageOptions) -> Self {
        Self {
            string: if let Some(string) = options.string {
                string
            } else {
                Arc::new(Mutex::new(String::new()))
            },
            session: if let Some(session) = options.session {
                session
            } else {
                Arc::new(
                    Mutex::new(
                        crate::types::Session {
                            dcs: vec![],
                            user: None,
                            state: None,
                        }
                    )
                )
            }
        }
    }

    /// Returns a clone of Arc<Mutex<String>>
    pub fn get_string(&self) -> Arc<Mutex<String>> {
        self.string.clone()
    }

    /// Returns a clone of Arc<Mutex<Session>>
    pub fn get_session(&self) -> Arc<Mutex<crate::types::Session>> {
        self.session.clone()
    }

}

impl SessionProvider for StringSessionStorage {
    fn save(&self) -> std::result::Result<(), SessionProviderError> {
        let session_ref = self.session.clone();
        let session = session_ref.lock().unwrap();

        let string_ref = self.string.clone();
        let mut string = string_ref.lock().unwrap();

        (&mut *string).truncate(0);
        base64::prelude::BASE64_STANDARD.encode_string(&*session.to_bytes(), &mut *string);

        Ok(())

    }

    fn load(&self) -> std::result::Result<(), SessionProviderError> {
        let session_ref = self.session.clone();
        let mut session = session_ref.lock().unwrap();

        let string_ref = self.string.clone();
        let string = string_ref.lock().unwrap();

        let mut output: Vec<u8> = vec![];
        base64::prelude::BASE64_STANDARD.decode_vec(&*string, &mut output).context(DecodeStringSnafu{
            string: (*string).clone()
        })?;

        let new_session = crate::types::Session::from_bytes(&output).context(InvalidFormatSnafu)?;
        *session = new_session;

        Ok(())
    }
}

type Result<T, E = SessionProviderError> = std::result::Result<T, E>;