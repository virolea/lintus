use indexmap::IndexMap;
use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::{Error, Query, Question, Result};

/// The API's answers to a [`Query`], each typed after the question it answers.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    model: Option<String>,
    usage: Option<Usage>,
    answers: IndexMap<String, Answer>,
    asked: IndexMap<String, &'static str>,
}

/// Tokens the request consumed, when the API reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
}

/// The answer to one question.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    Noul(NoulAnswer),
    Choice(ChoiceAnswer),
    Score(ScoreAnswer),
}

/// The answer to a [`Noul`](crate::Noul): the probability that it is true.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoulAnswer {
    pub noul: f64,
    /// The threshold the question was asked with.
    pub threshold: f64,
}

impl NoulAnswer {
    /// Whether the probability is above the question's threshold.
    pub fn result(&self) -> bool {
        self.noul > self.threshold
    }
}

/// The answer to a [`Choice`](crate::Choice): the option picked, and the
/// probability the model gave each option.
#[derive(Debug, Clone, PartialEq)]
pub struct ChoiceAnswer {
    pub choice: String,
    pub probabilities: IndexMap<String, f64>,
    pub confidence: Option<f64>,
}

impl ChoiceAnswer {
    pub fn result(&self) -> &str {
        &self.choice
    }
}

/// The answer to a [`Score`](crate::Score).
///
/// `score` is the probability-weighted level, good for ranking and averaging.
/// For a decision, [`level`](ScoreAnswer::level) and [`label`](ScoreAnswer::label)
/// give the single most probable level instead.
#[derive(Debug, Clone, PartialEq)]
pub struct ScoreAnswer {
    pub score: f64,
    /// The label of each level, keyed by the level's index ("0", "1", ...).
    pub legend: IndexMap<String, String>,
    /// The probability of each level, keyed by the level's index.
    pub probabilities: IndexMap<String, f64>,
    pub confidence: Option<f64>,
}

impl ScoreAnswer {
    pub fn result(&self) -> f64 {
        self.score
    }

    /// The most probable level; ties go to the lower level.
    pub fn level(&self) -> Option<usize> {
        let mut best: Option<(usize, f64)> = None;
        for (level, &probability) in &self.probabilities {
            let Ok(level) = level.parse::<usize>() else { continue };
            let better = match best {
                None => true,
                Some((best_level, best_probability)) => {
                    probability > best_probability || (probability == best_probability && level < best_level)
                }
            };
            if better {
                best = Some((level, probability));
            }
        }
        best.map(|(level, _)| level)
    }

    /// The label of the most probable level, from the legend.
    pub fn label(&self) -> Option<&str> {
        self.legend.get(&self.level()?.to_string()).map(String::as_str)
    }
}

impl Response {
    /// Reads a successful response body, pairing each answer with the question it answers.
    /// Answers to questions the query did not ask are ignored.
    pub(crate) fn parse(query: &Query, body: &str) -> Result<Response> {
        #[derive(Deserialize)]
        struct Payload {
            model: Option<String>,
            usage: Option<Usage>,
            answers: Option<IndexMap<String, serde_json::Value>>,
        }

        let payload: Payload = serde_json::from_str(body).map_err(|e| Error::InvalidResponse(e.to_string()))?;
        let mut raw_answers = payload.answers.unwrap_or_default();

        let mut answers = IndexMap::new();
        for (id, question) in query.question_map() {
            let Some(raw) = raw_answers.shift_remove(id) else { continue };
            if let Some(answer) = decode(id, question, raw)? {
                answers.insert(id.clone(), answer);
            }
        }

        let asked = query.questions().map(|(id, question)| (id.to_string(), question.type_name())).collect();
        Ok(Response { model: payload.model, usage: payload.usage, answers, asked })
    }

    /// The model version that answered: requests ask for the latest one.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    pub fn usage(&self) -> Option<&Usage> {
        self.usage.as_ref()
    }

    /// The answers, in the order the questions were asked.
    pub fn answers(&self) -> impl Iterator<Item = (&str, &Answer)> {
        self.answers.iter().map(|(id, answer)| (id.as_str(), answer))
    }

    pub fn answer(&self, id: &str) -> Option<&Answer> {
        self.answers.get(id)
    }

    pub fn noul(&self, id: &str) -> Result<&NoulAnswer> {
        match self.answers.get(id) {
            Some(Answer::Noul(answer)) => Ok(answer),
            _ => Err(self.missing(id, "noul")),
        }
    }

    pub fn choice(&self, id: &str) -> Result<&ChoiceAnswer> {
        match self.answers.get(id) {
            Some(Answer::Choice(answer)) => Ok(answer),
            _ => Err(self.missing(id, "choice")),
        }
    }

    pub fn score(&self, id: &str) -> Result<&ScoreAnswer> {
        match self.answers.get(id) {
            Some(Answer::Score(answer)) => Ok(answer),
            _ => Err(self.missing(id, "score")),
        }
    }

    fn missing(&self, id: &str, requested: &'static str) -> Error {
        match self.asked.get(id) {
            Some(&asked) if asked != requested => Error::WrongType { id: id.to_string(), asked, requested },
            _ => Error::Unanswered(id.to_string()),
        }
    }
}

/// Decodes one raw answer. An answer missing its value counts as no answer.
fn decode(id: &str, question: &Question, raw: serde_json::Value) -> Result<Option<Answer>> {
    #[derive(Deserialize)]
    struct RawNoul {
        noul: Option<f64>,
    }

    #[derive(Deserialize)]
    struct RawChoice {
        choice: Option<String>,
        #[serde(default)]
        probabilities: IndexMap<String, f64>,
        confidence: Option<f64>,
    }

    #[derive(Deserialize)]
    struct RawScore {
        score: Option<f64>,
        #[serde(default)]
        legend: IndexMap<String, String>,
        #[serde(default)]
        probabilities: IndexMap<String, f64>,
        confidence: Option<f64>,
    }

    fn read<T: DeserializeOwned>(id: &str, raw: serde_json::Value) -> Result<T> {
        serde_json::from_value(raw).map_err(|e| Error::InvalidResponse(format!("answer to {id}: {e}")))
    }

    Ok(match question {
        Question::Noul(noul) => read::<RawNoul>(id, raw)?
            .noul
            .map(|value| Answer::Noul(NoulAnswer { noul: value, threshold: noul.threshold() })),
        Question::Choice(_) => {
            let raw: RawChoice = read(id, raw)?;
            raw.choice.map(|choice| {
                Answer::Choice(ChoiceAnswer { choice, probabilities: raw.probabilities, confidence: raw.confidence })
            })
        }
        Question::Score(_) => {
            let raw: RawScore = read(id, raw)?;
            raw.score.map(|score| {
                Answer::Score(ScoreAnswer {
                    score,
                    legend: raw.legend,
                    probabilities: raw.probabilities,
                    confidence: raw.confidence,
                })
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Choice, Noul, Score};

    fn query() -> Query {
        let mut query = Query::new("state");
        query
            .ask("urgent", Noul::new("urgent?").with_threshold(0.8))
            .unwrap()
            .ask("team", Choice::new("which team?", [("billing", "b"), ("tech", "t")]).unwrap())
            .unwrap()
            .ask("mood", Score::new("how angry?", ["calm", "angry", "furious"]).unwrap())
            .unwrap();
        query
    }

    const BODY: &str = r#"{
        "model": "jev-1.13.0",
        "usage": { "input_tokens": 318, "output_tokens": 34 },
        "answers": {
            "mood": { "type": "score", "score": 1.05, "legend": { "0": "calm", "1": "angry", "2": "furious" },
                      "probabilities": { "0": 0.0, "1": 0.95, "2": 0.05 }, "confidence": 0.9 },
            "urgent": { "type": "noul", "noul": 0.75 },
            "team": { "type": "choice", "choice": "billing", "probabilities": { "billing": 0.88, "tech": 0.12 },
                      "confidence": 0.81 },
            "unasked": { "type": "noul", "noul": 1.0 }
        }
    }"#;

    #[test]
    fn answers_are_typed_by_their_question_and_kept_in_asking_order() {
        let response = Response::parse(&query(), BODY).unwrap();

        assert_eq!(response.model(), Some("jev-1.13.0"));
        assert_eq!(response.usage().unwrap().input_tokens, Some(318));
        assert_eq!(response.answers().map(|(id, _)| id).collect::<Vec<_>>(), ["urgent", "team", "mood"]);

        let urgent = response.noul("urgent").unwrap();
        assert_eq!(urgent.noul, 0.75);
        assert!(!urgent.result(), "0.75 is under the 0.8 threshold");

        let team = response.choice("team").unwrap();
        assert_eq!(team.result(), "billing");
        assert_eq!(team.probabilities["tech"], 0.12);
        assert_eq!(team.confidence, Some(0.81));

        let mood = response.score("mood").unwrap();
        assert_eq!(mood.result(), 1.05);
        assert_eq!(mood.level(), Some(1));
        assert_eq!(mood.label(), Some("angry"));
    }

    #[test]
    fn a_missing_answer_and_a_wrong_type_are_errors() {
        let response = Response::parse(&query(), r#"{ "answers": { "urgent": { "noul": null } } }"#).unwrap();

        assert_eq!(response.noul("urgent").unwrap_err().to_string(), "urgent has not been answered");
        assert_eq!(response.noul("never_asked").unwrap_err().to_string(), "never_asked has not been answered");
        assert_eq!(
            response.noul("team").unwrap_err().to_string(),
            "team was asked as a choice question, not a noul one"
        );
        assert!(response.model().is_none());
    }

    #[test]
    fn a_malformed_body_or_answer_is_an_invalid_response() {
        assert!(matches!(Response::parse(&query(), "not json"), Err(Error::InvalidResponse(_))));

        let error = Response::parse(&query(), r#"{ "answers": { "urgent": { "noul": "high" } } }"#).unwrap_err();
        assert!(error.to_string().starts_with("unexpected response from the Jev API: answer to urgent:"), "{error}");
    }

    #[test]
    fn score_level_ties_go_to_the_lower_level() {
        let answer = ScoreAnswer {
            score: 1.0,
            legend: IndexMap::new(),
            probabilities: [("2", 0.4), ("0", 0.4), ("1", 0.2)].into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            confidence: None,
        };
        assert_eq!(answer.level(), Some(0));
        assert_eq!(answer.label(), None);
    }
}
