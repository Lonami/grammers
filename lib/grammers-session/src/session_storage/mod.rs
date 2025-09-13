pub mod file_storage;
pub mod string_storage;

use std::io;
use base64::DecodeError;
use snafu::{Snafu, prelude::*};

pub trait SessionProvider {
    /// Save session into the underlying storage
    fn save(&self) -> Result<(), SessionProviderError>;

    /// Load session from the underlying storage
    fn load(&self) -> Result<(), SessionProviderError>;
}

#[derive(Snafu,Debug)]
#[snafu(visibility(pub(crate)))]
#[snafu(module(error))]
pub enum SessionProviderError {
    #[snafu(display("Specified path \"{path}\" not found"))]
    NotFound { path: String },

    #[snafu(display("Specified path \"{path}\" already exists"))]
    AlreadyExists { path: String },

    #[snafu(display("Error while converting bytes into session occurred"))]
    InvalidFormat {
        source: grammers_tl_types::deserialize::Error,
    },

    #[snafu(display("Unexpected IO error occurred"))]
    UnexpectedIoError {
        source: io::Error
    },

    #[snafu(display("Error while converting base64 string {string} into bytes"))]
    DecodeStringError{
        source: DecodeError,
        string: String
    },

    #[snafu(display("Strange argument has been passed : {}", if let Some(explanation) = explanation { explanation.to_owned() } else { "no explanation".to_string() }))]
    BadArgument { explanation: Option<String> },
}