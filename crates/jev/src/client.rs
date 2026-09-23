use std::fmt;
use std::time::Duration;

use serde::Serialize;

use crate::{ApiError, Error, Query, Response, Result};

pub const API_URL: &str = "https://api.typesafe.ai/v1/systemone";
/// The model every request asks for; [`Response::model`] says which version answered.
pub const MODEL: &str = "jev-latest";

/// Sends queries to the API. Cheap to clone, and safe to share between threads.
///
/// Rate limits (429) and overload (529) are not retried: the error says so
/// through [`Error::is_retryable`], and the retry policy is the caller's.
#[derive(Clone)]
pub struct Client {
    api_key: String,
    api_url: String,
    agent: ureq::Agent,
}

impl Client {
    pub fn new(api_key: impl Into<String>) -> Result<Client> {
        let api_key = api_key.into();
        if api_key.is_empty() {
            return Err(Error::MissingApiKey);
        }

        let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
        let agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .timeout_connect(Some(Duration::from_secs(60)))
            .timeout_recv_response(Some(Duration::from_secs(120)))
            .user_agent(concat!("jev-rust/", env!("CARGO_PKG_VERSION")))
            .tls_config(tls)
            .build()
            .into();
        Ok(Client { api_key, api_url: API_URL.to_string(), agent })
    }

    /// Sends requests somewhere other than the public API, such as a proxy or a test server.
    pub fn with_api_url(mut self, api_url: impl Into<String>) -> Client {
        self.api_url = api_url.into();
        self
    }

    pub fn api_url(&self) -> &str {
        &self.api_url
    }

    /// Asks every question of the query in one request.
    pub fn perform(&self, query: &Query) -> Result<Response> {
        #[derive(Serialize)]
        struct Body<'a> {
            model: &'static str,
            state: &'a str,
            questions: &'a indexmap::IndexMap<String, crate::Question>,
        }

        let body = serde_json::to_vec(&Body { model: MODEL, state: query.state(), questions: query.question_map() })
            .map_err(|e| Error::InvalidQuestion(e.to_string()))?;

        let mut response = self
            .agent
            .post(&self.api_url)
            .header("Content-Type", "application/json")
            .header("Authorization", &format!("Bearer {}", self.api_key))
            .send(&body[..])
            .map_err(|e| Error::Transport(e.to_string()))?;

        let status = response.status().as_u16();
        let text = response.body_mut().read_to_string().map_err(|e| Error::Transport(e.to_string()))?;
        if !(200..300).contains(&status) {
            return Err(Error::Api(ApiError { status, body: text }));
        }
        Response::parse(query, &text)
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client").field("api_url", &self.api_url).field("api_key", &"[redacted]").finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_api_key_is_refused() {
        assert!(matches!(Client::new(""), Err(Error::MissingApiKey)));
    }

    #[test]
    fn debug_output_hides_the_api_key() {
        let client = Client::new("secret-key").unwrap();
        let debug = format!("{client:?}");
        assert!(!debug.contains("secret-key"), "{debug}");
        assert!(debug.contains(API_URL));
    }
}
