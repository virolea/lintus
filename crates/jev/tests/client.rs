//! The client against a local HTTP server standing in for the API.

use std::sync::mpsc;
use std::thread;

use jev::{ApiErrorKind, Client, Error, Noul, Query};
use serde_json::{Value, json};

struct Received {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Value,
}

impl Received {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }
}

/// Serves one request with the given status and body, and hands back what it received.
fn serve_once(status: u16, body: &str) -> (String, mpsc::Receiver<Received>) {
    let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
    let url = format!("http://{}/v1/systemone", server.server_addr().to_ip().unwrap());
    let (sender, receiver) = mpsc::channel();
    let body = body.to_string();
    thread::spawn(move || {
        let mut request = server.recv().unwrap();
        let mut text = String::new();
        request.as_reader().read_to_string(&mut text).unwrap();
        sender
            .send(Received {
                method: request.method().to_string(),
                url: request.url().to_string(),
                headers: request.headers().iter().map(|h| (h.field.to_string(), h.value.to_string())).collect(),
                body: serde_json::from_str(&text).unwrap(),
            })
            .unwrap();
        request.respond(tiny_http::Response::from_string(body).with_status_code(status)).unwrap();
    });
    (url, receiver)
}

fn query() -> Query {
    let mut query = Query::new("Help! My payouts have been failing for 3 days.");
    query.ask("is_urgent", Noul::new("Does this convey urgency?").with_criteria("Time-sensitive", "Not")).unwrap();
    query
}

#[test]
fn sends_the_query_as_json_with_the_api_key() {
    let (url, received) = serve_once(200, r#"{"model":"jev-1.13.0","answers":{"is_urgent":{"type":"noul","noul":0.95}}}"#);

    let response = Client::new("secret").unwrap().with_api_url(&url).perform(&query()).unwrap();

    let request = received.recv().unwrap();
    assert_eq!(request.method, "POST");
    assert_eq!(request.url, "/v1/systemone");
    assert_eq!(request.header("Authorization"), Some("Bearer secret"));
    assert_eq!(request.header("Content-Type"), Some("application/json"));
    assert_eq!(
        request.body,
        json!({
            "model": "jev-latest",
            "state": "Help! My payouts have been failing for 3 days.",
            "questions": {
                "is_urgent": {
                    "type": "noul",
                    "instructions": "Does this convey urgency?",
                    "criteria": { "true": "Time-sensitive", "false": "Not" }
                }
            }
        })
    );

    assert_eq!(response.model(), Some("jev-1.13.0"));
    assert!(response.noul("is_urgent").unwrap().result());
}

#[test]
fn error_statuses_become_api_errors() {
    for (status, kind, retryable) in [
        (401, ApiErrorKind::Authentication, false),
        (422, ApiErrorKind::Validation, false),
        (429, ApiErrorKind::RateLimit, true),
        (529, ApiErrorKind::Overloaded, true),
        (500, ApiErrorKind::Other, false),
    ] {
        let (url, _received) = serve_once(status, "nope");

        let error = Client::new("key").unwrap().with_api_url(&url).perform(&query()).unwrap_err();

        let api = error.api().unwrap_or_else(|| panic!("{status}: {error:?}"));
        assert_eq!((api.status, api.body.as_str(), api.kind()), (status, "nope", kind));
        assert_eq!(error.is_retryable(), retryable, "{status}");
        assert_eq!(error.to_string(), format!("Jev API error {status}: nope"));
    }
}

#[test]
fn an_unreachable_api_is_a_transport_error() {
    let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();

    let error = Client::new("key").unwrap().with_api_url(format!("http://127.0.0.1:{port}/")).perform(&query()).unwrap_err();

    assert!(matches!(error, Error::Transport(_)), "{error:?}");
    assert!(!error.is_retryable());
}

#[test]
fn a_success_that_is_not_json_is_an_invalid_response() {
    let (url, _received) = serve_once(200, "<html>maintenance</html>");

    let error = Client::new("key").unwrap().with_api_url(&url).perform(&query()).unwrap_err();

    assert!(matches!(error, Error::InvalidResponse(_)), "{error:?}");
}
