use indexmap::IndexMap;

use crate::{Error, Question, Result};

/// A state (the text the questions are about) and the questions to ask of it,
/// each under an identifier the answers come back keyed by.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    state: String,
    questions: IndexMap<String, Question>,
}

impl Query {
    pub fn new(state: impl Into<String>) -> Query {
        Query { state: state.into(), questions: IndexMap::new() }
    }

    /// Adds a question. Identifiers must be unique within a query.
    pub fn ask(&mut self, id: impl Into<String>, question: impl Into<Question>) -> Result<&mut Query> {
        let id = id.into();
        if self.questions.contains_key(&id) {
            return Err(Error::DuplicateQuestion(id));
        }
        self.questions.insert(id, question.into());
        Ok(self)
    }

    pub fn state(&self) -> &str {
        &self.state
    }

    pub fn question(&self, id: &str) -> Option<&Question> {
        self.questions.get(id)
    }

    /// The questions in the order they were asked.
    pub fn questions(&self) -> impl Iterator<Item = (&str, &Question)> {
        self.questions.iter().map(|(id, question)| (id.as_str(), question))
    }

    pub(crate) fn question_map(&self) -> &IndexMap<String, Question> {
        &self.questions
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Noul;

    #[test]
    fn questions_keep_the_order_they_were_asked_in() {
        let mut query = Query::new("state");
        query.ask("b", Noul::new("q1")).unwrap().ask("a", Noul::new("q2")).unwrap();

        let ids: Vec<_> = query.questions().map(|(id, _)| id).collect();
        assert_eq!(ids, ["b", "a"]);
        assert_eq!(query.question("a").unwrap().instructions(), "q2");
    }

    #[test]
    fn an_identifier_can_only_be_asked_once() {
        let mut query = Query::new("state");
        query.ask("a", Noul::new("q")).unwrap();

        let error = query.ask("a", Noul::new("again")).unwrap_err();
        assert_eq!(error.to_string(), "a has already been asked");
        assert_eq!(query.question("a").unwrap().instructions(), "q");
    }
}
