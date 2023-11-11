#![allow(clippy::module_name_repetitions)]
use std::{error, fmt};

#[derive(Debug)]
pub struct Error {
    pub msg: String,
    pub kind: ErrorKind,
}

#[derive(Debug, PartialEq)]
pub enum ErrorKind {
    /// The requested file is not found.
    FileNotFound,
    /// Store folder is nonexistent or corrupted.
    StoreFolder,
    /// Thumbnail folder is nonexistent or corrupted.
    ThumbnailFolder,
    /// Errors emitted by libmagic.
    Magic,
    /// Generic IO errors.
    IO,
    /// Database errors.
    DB,
    /// The file is not yet supported by vorg.
    Unsupported,
    /// The item to import exists already in the repo.
    Duplicate,
    /// Wrong arguments to the commandline util.
    WrongArguments,
}

impl error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.msg)
    }
}

impl From<sqlx::Error> for Error {
    fn from(value: sqlx::Error) -> Self {
        Error {
            msg: value.to_string(),
            kind: ErrorKind::DB,
        }
    }
}

impl From<magic::MagicError> for Error {
    fn from(value: magic::MagicError) -> Self {
        Error {
            msg: value.to_string(),
            kind: ErrorKind::Magic,
        }
    }
}

/// This trait should only be used for generic IO errors that does not fall into any of the other
/// error categories.
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error {
            msg: value.to_string(),
            kind: ErrorKind::IO,
        }
    }
}
