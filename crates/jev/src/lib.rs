//! A typed client for the [Typesafe](https://typesafe.ai) Jev model API.
//!
//! A [`Query`] is a piece of text (the *state*) and a set of named questions
//! about it. Each question has a type, which fixes the shape of its answer:
//!
//! - [`Noul`]: a true/false judgement, answered with a probability.
//! - [`Choice`]: one option out of a set, with a probability for each.
//! - [`Score`]: a level on a scale of 2 to 10.
//!
//! ```no_run
//! use jev::{Choice, Client, Noul, Query};
//!
//! # fn main() -> Result<(), jev::Error> {
//! let client = Client::new(std::env::var("JEV_API_KEY").unwrap_or_default())?;
//!
//! let mut query = Query::new("Help! My payouts have been failing for 3 days.");
//! query.ask("urgent", Noul::new("Does this message need an answer today?").with_threshold(0.7))?;
//! query.ask("topic", Choice::new("What is this message about?", [
//!     ("billing", "Invoices, charges or refunds"),
//!     ("payouts", "Money sent to the customer"),
//! ])?)?;
//!
//! let response = client.perform(&query)?;
//! if response.noul("urgent")?.result() {
//!     println!("urgent, about {}", response.choice("topic")?.choice);
//! }
//! # Ok(())
//! # }
//! ```

mod client;
mod error;
mod query;
mod question;
mod response;

pub use client::{API_URL, Client, MODEL};
pub use error::{ApiError, ApiErrorKind, Error, Result};
pub use query::Query;
pub use question::{Choice, Noul, Question, Score};
pub use response::{Answer, ChoiceAnswer, NoulAnswer, Response, ScoreAnswer, Usage};
