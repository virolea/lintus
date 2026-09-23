# jev-api

A typed Rust client for the [Typesafe](https://typesafe.ai) Jev model API. It is the Rust
counterpart of the [`jev` gem](https://github.com/virolea/jev), and what
[Lintus](https://github.com/virolea/lintus) talks to the API with.

```toml
[dependencies]
jev-api = "0.1"   # imported as `jev`
```

```rust
use jev::{Client, Noul, Query, Score};

let client = Client::new(std::env::var("JEV_API_KEY").unwrap_or_default())?;

let mut query = Query::new("Help! My payouts have been failing for 3 days.");
query.ask("is_urgent", Noul::new("Does this convey urgency?").with_threshold(0.8))?;
query.ask("frustration", Score::new("How frustrated is the customer?", ["Calm", "Frustrated", "Very angry"])?)?;

let response = client.perform(&query)?;
response.noul("is_urgent")?.result();        // true: 0.95 is above 0.8
response.noul("is_urgent")?.noul;            // 0.95
response.score("frustration")?.label();      // Some("Frustrated")
```

## Questions

| Type                | Answer                                                     | Built with |
| ------------------- | ---------------------------------------------------------- | ---------- |
| `Noul`              | A probability that the statement is true, and `result()` against a threshold (0.5 by default). | `Noul::new(text)`, then `.with_criteria(when_true, when_false)` and `.with_threshold(t)` |
| `Choice`            | The option picked, with a probability for each.            | `Choice::new(text, [(name, description), ...])`, 1 to 255 options |
| `Score`             | The probability-weighted level (`result()`), and the most probable `level()` and `label()`. | `Score::new(text, [lowest, ..., highest])`, 2 to 10 levels |

Every question of a query is answered in one request, so several narrow questions cost far
less than several calls.

Asking for an answer that is not there is an error, never a default: `Error::Unanswered` when
the API gave none, `Error::WrongType` when the question was asked as another type.

## Errors

API error statuses come back as `Error::Api`, whose `kind()` is `Authentication` (401),
`Validation` (422), `RateLimit` (429), `Overloaded` (529) or `Other`. Rate limits are not
retried for you; `error.is_retryable()` tells you when backing off and retrying makes sense.

`Client::with_api_url` points the client at a proxy or a test server.
