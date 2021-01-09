// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
mod generated;

pub use generated::LAYER as VERSION;
use generated::{enums, types};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, Write};
use std::path::Path;

// Needed for auto-generated definitions.
use grammers_tl_types::{deserialize, serialize, Deserializable, Identifiable, Serializable};

pub struct Session {
    file: File,
    session: types::Session,
}

impl Session {
    /// Loads or creates a new session file.
    pub fn load_or_create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        if let Ok(instance) = Self::load(path.as_ref()) {
            Ok(instance)
        } else {
            Self::create(path)
        }
    }

    /// Create a new session instance.
    pub fn create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        Ok(Self {
            file: File::create(path)?,
            session: types::Session {
                dcs: Vec::new(),
                user: None,
                state: None,
            },
        })
    }

    /// Load a previous session instance.
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut data = Vec::new();
        File::open(path.as_ref())?.read_to_end(&mut data)?;
        let enums::Session::Session(session) = enums::Session::from_bytes(&data).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "malformed session or unsupported version",
            )
        })?;

        Ok(Self {
            file: OpenOptions::new().write(true).open(path.as_ref())?,
            session,
        })
    }

    /// Saves the session file.
    pub fn save(&mut self) -> io::Result<()> {
        self.file.seek(io::SeekFrom::Start(0))?;
        self.file.set_len(0)?;
        self.file
            .write_all(&enums::Session::Session(self.session.clone()).to_bytes())?;
        self.file.sync_data()?;
        Ok(())
    }
}
