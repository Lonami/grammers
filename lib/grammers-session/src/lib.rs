// Copyright 2020 - developers of the `grammers` project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.
use grammers_crypto::auth_key::AuthKey;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, Seek, Write};
use std::net::SocketAddr;
use std::path::Path;

const CURRENT_VERSION: u32 = 1;

fn parse_hex(byte: &str) -> Option<u8> {
    match u8::from_str_radix(byte, 16) {
        Ok(x) => Some(x),
        Err(_) => None,
    }
}

fn key_from_hex(hex: &str) -> Option<[u8; 256]> {
    let mut buffer = [0; 256];
    if hex.len() == buffer.len() * 2 {
        for (i, byte) in buffer.iter_mut().enumerate() {
            let i = i * 2;
            if let Some(value) = parse_hex(&hex[i..i + 2]) {
                *byte = value;
            } else {
                return None;
            }
        }

        Some(buffer)
    } else {
        None
    }
}

fn hex_from_key(key: &[u8; 256]) -> String {
    use std::fmt::Write;
    let mut buffer = String::with_capacity(key.len() * 2);
    for byte in key.iter() {
        write!(buffer, "{:02x}", byte).unwrap();
    }
    buffer
}

pub struct Session {
    file: File,
    pub user_dc: Option<(i32, SocketAddr)>,
    pub auth_key: Option<AuthKey>,
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
            user_dc: None,
            auth_key: None,
        })
    }

    /// Load a previous session instance.
    pub fn load<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut lines = BufReader::new(File::open(path.as_ref())?).lines();

        // Version
        let version: u32 = if let Some(Ok(line)) = lines.next() {
            line.parse()
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "malformed session"))?
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "malformed session",
            ));
        };
        if version != CURRENT_VERSION {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "unknown version",
            ));
        }

        // user_dc.0
        let user_dc_id = if let Some(Ok(line)) = lines.next() {
            match line.parse() {
                Ok(x) => Some(x),
                Err(_) => None,
            }
        } else {
            None
        };

        // user_dc.1
        let user_dc_addr = if let Some(Ok(line)) = lines.next() {
            match line.parse() {
                Ok(x) => Some(x),
                Err(_) => None,
            }
        } else {
            None
        };

        // user_dc
        let user_dc = if let Some(id) = user_dc_id {
            if let Some(addr) = user_dc_addr {
                Some((id, addr))
            } else {
                None
            }
        } else {
            None
        };

        // auth_key
        let auth_key = if let Some(Ok(line)) = lines.next() {
            key_from_hex(&line)
        } else {
            None
        };

        drop(lines);
        Ok(Self {
            file: OpenOptions::new().write(true).open(path.as_ref())?,
            user_dc,
            auth_key: auth_key.map(AuthKey::from_bytes),
        })
    }

    /// Saves the session file.
    pub fn save(&mut self) -> io::Result<()> {
        self.file.seek(io::SeekFrom::Start(0))?;
        writeln!(self.file, "{}", CURRENT_VERSION)?;

        if let Some((dc_id, dc_addr)) = self.user_dc {
            writeln!(self.file, "{}", dc_id)?;
            writeln!(self.file, "{}", dc_addr)?;
        } else {
            writeln!(self.file)?;
            writeln!(self.file)?;
        }

        if let Some(data) = &self.auth_key {
            writeln!(self.file, "{}", hex_from_key(&data.to_bytes()))?;
        } else {
            writeln!(self.file)?;
        }
        self.file.sync_data()?;
        Ok(())
    }
}
