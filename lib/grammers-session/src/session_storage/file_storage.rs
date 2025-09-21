use super::error::*;
use crate::generated::types;
use crate::generated::types::Session;
use crate::session_storage::{SessionProvider, SessionProviderError};
use grammers_tl_types::{Deserializable, Serializable};
use std::cmp::PartialEq;
use std::io::{ErrorKind, Read, Seek, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use snafu::prelude::*;

pub struct FileSessionStorage {
    file: Arc<Mutex<std::fs::File>>,
    session: Arc<Mutex<Session>>,
}

pub enum OpenMode {
    /// Creates a new file. If the file already exists, an error will occur.
    /// This mode is used when you need to create a new session file.
    Create,

    /// Opens an existing file for reading/writing.
    /// If the file does not exist, an error will occur.
    Open,

    /// Attempts to create a file if it doesn't exist, or open it if it does.
    /// Combines the functionality of `Create` and `Open` modes.
    CreateOpen
}

/// Used in [`FileSessionStorage::new`] to configure storage
pub struct FileSessionStorageOptions {
    /// Path to the file for saving and loading session.
    /// The path can be either absolute or relative.
    pub path: Box<dyn AsRef<Path>>,

    /// Specifies the mode for file operations.
    pub mode: OpenMode,

    /// Holds the session data. When [`FileSessionStorage::load`] is called,
    /// the session from the file will be loaded into this field.
    /// When [`FileSessionStorage::save`] is called, the session stored here
    /// will be written to the file.
    pub session: Arc<Mutex<Session>>,
}

impl FileSessionStorage {
    /// Returns a clone of the [`Arc<Mutex<Session>>`] containing the underlying session data
    pub fn get_session(&self) -> Arc<Mutex<types::Session>> {
        self.session.clone()
    }

    /// Returns a clone of the [`Arc<Mutex<File>>`] representing the file used by the storage.
    pub fn get_file(&self) -> Arc<Mutex<std::fs::File>> {
        self.file.clone()
    }

    /// Creates a new storage instance with default session and specified path.
    /// ```ignore
    /// session: Session{
    ///     dcs: vec![],
    ///     user: None,
    ///     state: None,
    /// },
    /// path: path,
    /// mode: OpenMode::CreateOpen
    /// ```
    pub fn default_with_path<T: AsRef<Path> + 'static>(path: T) -> Result<Self> {
        Self::default_with_path_and_session(path, Arc::new(Mutex::new(types::Session{
            dcs: vec![],
            user: None,
            state: None,
        })))
    }

    /// Creates a new storage instance with specified path and session.
    /// ```ignore
    /// session: session,
    /// path: path,
    /// mode: OpenMode::CreateOpen
    /// ```
    pub fn default_with_path_and_session<T: AsRef<Path> + 'static>(path: T, session: Arc<Mutex<types::Session>>) -> Result<Self> {
        Self::new(
            FileSessionStorageOptions{
                path: Box::new(path),
                mode: OpenMode::CreateOpen,
                session
            }
        )
    }

    pub fn new(options: FileSessionStorageOptions) -> Result<Self> {
        let path = (*options.path).as_ref().display().to_string();
        let exists = std::fs::exists(&*options.path).context(UnexpectedIoSnafu)?;

        match options.mode {
            OpenMode::Create => {
                ensure!(!exists, AlreadyExistsSnafu { path });

                Ok(Self {
                    file: Arc::new(Mutex::new(
                        std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .read(true)
                            .open(&*options.path)
                            .context(UnexpectedIoSnafu)?
                    )),
                    session: options.session,
                })
            }
            OpenMode::Open => {
                ensure!(exists, NotFoundSnafu { path });

                Ok(Self {
                    file: Arc::new(Mutex::new(
                        std::fs::OpenOptions::new()
                            .write(true)
                            .read(true)
                            .open(&*options.path)
                            .context(UnexpectedIoSnafu)?,
                    )),
                    session: options.session,
                })
            }
            OpenMode::CreateOpen => {
                let file = match std::fs::OpenOptions::new()
                    .write(true)
                    .read(true)
                    .create(true)
                    .open(&*options.path)
                {
                    Ok(f) => f,
                    Err(e) => if e.kind() == ErrorKind::NotFound {

                        std::fs::OpenOptions::new()
                            .write(true)
                            .read(true)
                            .open(&*options.path)
                            .context(UnexpectedIoSnafu)?

                    } else {
                        Err(e).context(UnexpectedIoSnafu)?
                    }
                };

                Ok(Self{file: Arc::new(Mutex::new(file)), session: options.session})
            }
        }
    }
}

impl SessionProvider for FileSessionStorage {
    /// Saves the current session data to the underlying file.
    fn save(&self) -> Result<()> {
        let session = self.session.lock().unwrap();
        let mut file = self.file.lock().unwrap();
        let handle = &mut *file;

        handle.seek(std::io::SeekFrom::Start(0)).context(UnexpectedIoSnafu)?;
        handle.set_len(0).context(UnexpectedIoSnafu)?;
        handle.write_all(&session.to_bytes()).context(UnexpectedIoSnafu)?;
        handle.sync_data().context(UnexpectedIoSnafu)
    }

    /// Loads session data from the underlying file into the session.
    fn load(&self) -> Result<()> {
        let mut session = self.session.lock().unwrap();
        let mut file = self.file.lock().unwrap();
        let handle = &mut *file;

        let mut content: Vec<u8> = vec![];
        handle
            .read_to_end(&mut content)
            .context(UnexpectedIoSnafu)?;

        let deserialized_session = Session::from_bytes(&content).context(InvalidFormatSnafu)?;

        *session = deserialized_session;

        Ok(())
    }
}

impl PartialEq for OpenMode {
    fn eq(&self, other: &Self) -> bool {
        self == other
    }
}

type Result<T, E = SessionProviderError> = std::result::Result<T, E>;