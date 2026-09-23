use indexmap::IndexMap;
use serde::Serialize;

use crate::{Error, Result};

/// A question of any type, as it is sent to the API.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    Noul(Noul),
    Choice(Choice),
    Score(Score),
}

impl Question {
    /// The type's name in the API: "noul", "choice" or "score".
    pub fn type_name(&self) -> &'static str {
        match self {
            Question::Noul(_) => "noul",
            Question::Choice(_) => "choice",
            Question::Score(_) => "score",
        }
    }

    pub fn instructions(&self) -> &str {
        match self {
            Question::Noul(q) => &q.instructions,
            Question::Choice(q) => &q.instructions,
            Question::Score(q) => &q.instructions,
        }
    }
}

/// A true/false question. The answer is the probability that it is true,
/// and its [result](crate::NoulAnswer::result) is whether that probability
/// is above the threshold (0.5 unless set).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Noul {
    instructions: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    criteria: Option<IndexMap<String, String>>,
    #[serde(skip)]
    threshold: f64,
}

impl Noul {
    pub const DEFAULT_THRESHOLD: f64 = 0.5;

    pub fn new(instructions: impl Into<String>) -> Noul {
        Noul { instructions: instructions.into(), criteria: None, threshold: Self::DEFAULT_THRESHOLD }
    }

    /// Spells out what a true and a false answer mean.
    pub fn with_criteria(self, when_true: impl Into<String>, when_false: impl Into<String>) -> Noul {
        self.with_criterion("true", when_true).with_criterion("false", when_false)
    }

    /// Adds one entry to the criteria, keyed by the answer it describes.
    pub fn with_criterion(mut self, answer: impl Into<String>, meaning: impl Into<String>) -> Noul {
        self.criteria.get_or_insert_with(IndexMap::new).insert(answer.into(), meaning.into());
        self
    }

    /// The probability above which the answer counts as true. It stays on
    /// this side: the API is never sent the threshold.
    pub fn with_threshold(mut self, threshold: f64) -> Noul {
        self.threshold = threshold;
        self
    }

    pub fn instructions(&self) -> &str {
        &self.instructions
    }

    pub fn criteria(&self) -> Option<&IndexMap<String, String>> {
        self.criteria.as_ref()
    }

    pub fn threshold(&self) -> f64 {
        self.threshold
    }
}

/// A question answered by picking one of a set of options, each described in words.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Choice {
    instructions: String,
    #[serde(rename = "criteria")]
    options: IndexMap<String, String>,
}

impl Choice {
    pub const MAX_OPTIONS: usize = 255;

    /// Takes the options as (name, description) pairs, 1 to 255 of them.
    pub fn new<K, V>(instructions: impl Into<String>, options: impl IntoIterator<Item = (K, V)>) -> Result<Choice>
    where
        K: Into<String>,
        V: Into<String>,
    {
        let options: IndexMap<String, String> = options.into_iter().map(|(k, v)| (k.into(), v.into())).collect();
        if options.is_empty() || options.len() > Self::MAX_OPTIONS {
            return Err(Error::InvalidQuestion(format!(
                "a choice question takes 1 to {} options, got {}",
                Self::MAX_OPTIONS,
                options.len()
            )));
        }
        Ok(Choice { instructions: instructions.into(), options })
    }

    pub fn instructions(&self) -> &str {
        &self.instructions
    }

    pub fn options(&self) -> &IndexMap<String, String> {
        &self.options
    }
}

/// A question answered with a level on a scale, each level described in words.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Score {
    instructions: String,
    #[serde(rename = "criteria")]
    levels: Vec<String>,
}

impl Score {
    pub const LEVELS: std::ops::RangeInclusive<usize> = 2..=10;

    /// Takes the levels from lowest to highest, 2 to 10 of them.
    pub fn new<S: Into<String>>(instructions: impl Into<String>, levels: impl IntoIterator<Item = S>) -> Result<Score> {
        let levels: Vec<String> = levels.into_iter().map(Into::into).collect();
        if !Self::LEVELS.contains(&levels.len()) {
            return Err(Error::InvalidQuestion(format!(
                "a score question takes {} to {} levels, got {}",
                Self::LEVELS.start(),
                Self::LEVELS.end(),
                levels.len()
            )));
        }
        Ok(Score { instructions: instructions.into(), levels })
    }

    pub fn instructions(&self) -> &str {
        &self.instructions
    }

    pub fn levels(&self) -> &[String] {
        &self.levels
    }
}

impl From<Noul> for Question {
    fn from(question: Noul) -> Question {
        Question::Noul(question)
    }
}

impl From<Choice> for Question {
    fn from(question: Choice) -> Question {
        Question::Choice(question)
    }
}

impl From<Score> for Question {
    fn from(question: Score) -> Question {
        Question::Score(question)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn to_json(question: impl Into<Question>) -> serde_json::Value {
        serde_json::to_value(question.into()).unwrap()
    }

    #[test]
    fn a_noul_is_sent_without_its_threshold_and_with_criteria_only_when_set() {
        assert_eq!(to_json(Noul::new("q?").with_threshold(0.8)), json!({ "type": "noul", "instructions": "q?" }));
        assert_eq!(
            to_json(Noul::new("q?").with_criteria("yes", "no")),
            json!({ "type": "noul", "instructions": "q?", "criteria": { "true": "yes", "false": "no" } })
        );
    }

    #[test]
    fn criteria_keep_their_order() {
        let noul = Noul::new("q").with_criterion("false", "f").with_criterion("true", "t");
        let json = serde_json::to_string(&Question::from(noul)).unwrap();
        assert_eq!(json, r#"{"type":"noul","instructions":"q","criteria":{"false":"f","true":"t"}}"#);
    }

    #[test]
    fn a_choice_sends_its_options_as_criteria() {
        let choice = Choice::new("which?", [("a", "first"), ("b", "second")]).unwrap();
        assert_eq!(
            to_json(choice),
            json!({ "type": "choice", "instructions": "which?", "criteria": { "a": "first", "b": "second" } })
        );
    }

    #[test]
    fn a_choice_takes_1_to_255_options() {
        let none: [(&str, &str); 0] = [];
        let error = Choice::new("q", none).unwrap_err();
        assert_eq!(error.to_string(), "a choice question takes 1 to 255 options, got 0");

        let many = (0..256).map(|i| (i.to_string(), "x"));
        assert!(Choice::new("q", many).is_err());
        assert!(Choice::new("q", (0..255).map(|i| (i.to_string(), "x"))).is_ok());
    }

    #[test]
    fn a_score_sends_its_levels_as_criteria() {
        let score = Score::new("how much?", ["low", "high"]).unwrap();
        assert_eq!(
            to_json(score),
            json!({ "type": "score", "instructions": "how much?", "criteria": ["low", "high"] })
        );
    }

    #[test]
    fn a_score_takes_2_to_10_levels() {
        assert_eq!(Score::new("q", ["one"]).unwrap_err().to_string(), "a score question takes 2 to 10 levels, got 1");
        assert!(Score::new("q", (0..11).map(|i| i.to_string())).is_err());
        assert!(Score::new("q", (0..10).map(|i| i.to_string())).is_ok());
    }
}
