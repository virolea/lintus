//! Errors from building a query and from talking to the API.

use std::fmt;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Everything that can go wrong building a query or talking to the API.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// The client was given an empty API key.
    MissingApiKey,
    /// The API answered with an error status.
    Api(ApiError),
    /// The API could not be reached, or the connection broke.
    Transport(String),
    /// The API answered successfully with something that is not a Jev response.
    InvalidResponse(String),
    /// A question was badly formed, such as a choice with no options.
    InvalidQuestion(String),
    /// Two questions of one query share an identifier.
    DuplicateQuestion(String),
    /// The response has no answer for this question.
    Unanswered(String),
    /// The question was asked as another type than the one requested.
    WrongType { id: String, asked: &'static str, requested: &'static str },
}

impl Error {
    /// The API error behind this error, if it is one.
    pub fn api(&self) -> Option<&ApiError> {
        match self {
            Error::Api(error) => Some(error),
            _ => None,
        }
    }

    /// Whether the same request may succeed if sent again later: the API was
    /// rate limiting or overloaded.
    pub fn is_retryable(&self) -> bool {
        self.api().is_some_and(|error| matches!(error.kind(), ApiErrorKind::RateLimit | ApiErrorKind::Overloaded))
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::MissingApiKey => write!(f, "missing API key: pass the key of your Typesafe account to Client::new"),
            Error::Api(error) => error.fmt(f),
            Error::Transport(message) => write!(f, "could not reach the Jev API: {message}"),
            Error::InvalidResponse(message) => write!(f, "unexpected response from the Jev API: {message}"),
            Error::InvalidQuestion(message) => write!(f, "{message}"),
            Error::DuplicateQuestion(id) => write!(f, "{id} has already been asked"),
            Error::Unanswered(id) => write!(f, "{id} has not been answered"),
            Error::WrongType { id, asked, requested } => {
                write!(f, "{id} was asked as a {asked} question, not a {requested} one")
            }
        }
    }
}

impl std::error::Error for Error {}

/// An error status from the API, with the body it came with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiError {
    pub status: u16,
    pub body: String,
}

/// What an [`ApiError`] status means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiErrorKind {
    /// 401: the API key was refused.
    Authentication,
    /// 422: the query was refused.
    Validation,
    /// 429: too many requests.
    RateLimit,
    /// 529: the model is overloaded.
    Overloaded,
    /// Any other error status.
    Other,
}

impl ApiError {
    pub fn kind(&self) -> ApiErrorKind {
        match self.status {
            401 => ApiErrorKind::Authentication,
            422 => ApiErrorKind::Validation,
            429 => ApiErrorKind::RateLimit,
            529 => ApiErrorKind::Overloaded,
            _ => ApiErrorKind::Other,
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Jev API error {}: {}", self.status, self.body)
    }
}

impl std::error::Error for ApiError {}
