use std::fs::File;
use std::io::{self, BufRead, BufReader, Seek, Write};
use std::net::SocketAddr;
use std::path::Path;

use crate::Session;

const CURRENT_VERSION: u32 = 1;

/// A basic session implementation, backed by a text file.
pub struct TextSession {
    file: File,
    user_dc: Option<(i32, SocketAddr)>,
    auth_key_data: Option<[u8; 256]>,
}

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

impl TextSession {
    /// Create a new session instance.
    pub fn create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        Ok(Self {
            file: File::create(path)?,
            user_dc: None,
            auth_key_data: None,
        })
    }

    /// Load a previous session instance.
    pub fn load<P: AsRef<Path>>(path: &P) -> io::Result<Self> {
        let mut lines = BufReader::new(File::open(path)?).lines();

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

        // auth_key_data
        let auth_key_data = if let Some(Ok(line)) = lines.next() {
            key_from_hex(&line)
        } else {
            None
        };

        drop(lines);
        Ok(Self {
            file: File::open(path)?,
            user_dc,
            auth_key_data,
        })
    }
}

impl Session for TextSession {
    fn set_user_datacenter(&mut self, dc_id: i32, dc_addr: &SocketAddr) {
        self.user_dc = Some((dc_id, dc_addr.clone()));
    }

    fn set_auth_key_data(&mut self, _dc_id: i32, data: &[u8; 256]) {
        self.auth_key_data = Some(data.clone());
    }

    fn get_user_datacenter(&self) -> Option<(i32, SocketAddr)> {
        self.user_dc.clone()
    }

    fn get_auth_key_data(&self, _dc_id: i32) -> Option<[u8; 256]> {
        self.auth_key_data.clone()
    }

    fn save(&mut self) -> io::Result<()> {
        self.file.seek(io::SeekFrom::Start(0))?;
        writeln!(self.file, "{}", CURRENT_VERSION)?;

        if let Some((dc_id, dc_addr)) = self.user_dc {
            writeln!(self.file, "{}", dc_id)?;
            writeln!(self.file, "{}", dc_addr)?;
        } else {
            writeln!(self.file)?;
            writeln!(self.file)?;
        }

        if let Some(data) = self.auth_key_data {
            writeln!(self.file, "{}", hex_from_key(&data))?;
        } else {
            writeln!(self.file)?;
        }
        self.file.sync_data()?;
        Ok(())
    }
}
