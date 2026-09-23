//! The error that stops a run, and how it reaches the user.

use std::fmt;

/// An error that stops the run before or instead of linting: a bad argument,
/// config file, git command or credentials file. It is printed as
/// `lintus: <message>` and the command exits with status 2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(String);

pub type Result<T, E = Error> = std::result::Result<T, E>;

impl Error {
    pub fn new(message: impl Into<String>) -> Error {
        Error(message.into())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<jev::Error> for Error {
    fn from(error: jev::Error) -> Error {
        Error(error.to_string())
    }
}
