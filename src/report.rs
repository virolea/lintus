//! Everything a run produced: what was checked, what was flagged, what was skipped, what broke.

/// A file the model answered about, with the probability each rule got,
/// offense or not, so a run can be read for confidence and not just verdicts.
#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    pub path: String,
    pub answers: Vec<(String, f64)>,
}

/// A rule the model flagged on a file, with the probability it gave.
#[derive(Debug, Clone, PartialEq)]
pub struct Offense {
    pub path: String,
    pub rule: String,
    pub message: String,
    pub noul: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Skipped {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub path: String,
    pub error: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Report {
    pub checked: Vec<Checked>,
    pub offenses: Vec<Offense>,
    pub skipped: Vec<Skipped>,
    pub failures: Vec<Failure>,
}

impl Report {
    /// Puts every list in a stable order, whatever order the files finished in:
    /// by path, and offenses by path then rule.
    pub fn sort(&mut self) {
        self.checked.sort_by(|a, b| a.path.cmp(&b.path));
        self.offenses.sort_by(|a, b| (&a.path, &a.rule).cmp(&(&b.path, &b.rule)));
        self.skipped.sort_by(|a, b| a.path.cmp(&b.path));
        self.failures.sort_by(|a, b| a.path.cmp(&b.path));
    }

    /// 2 when a file could not be checked, 1 when there are offenses, else 0.
    pub fn exit_status(&self) -> i32 {
        if !self.failures.is_empty() {
            2
        } else if !self.offenses.is_empty() {
            1
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offense(path: &str, rule: &str) -> Offense {
        Offense { path: path.into(), rule: rule.into(), message: String::new(), noul: 0.9 }
    }

    #[test]
    fn exit_status() {
        let mut report = Report::default();
        assert_eq!(report.exit_status(), 0);

        report.offenses.push(offense("a.rb", "r"));
        assert_eq!(report.exit_status(), 1);

        report.failures.push(Failure { path: "b.rb".into(), error: "boom".into() });
        assert_eq!(report.exit_status(), 2);
    }

    #[test]
    fn offenses_sort_by_path_then_rule() {
        let mut report = Report {
            offenses: vec![offense("b.rb", "no_sleep"), offense("a.rb", "no_sleep"), offense("a.rb", "documented")],
            ..Report::default()
        };
        report.sort();

        let order: Vec<_> = report.offenses.iter().map(|o| (o.path.as_str(), o.rule.as_str())).collect();
        assert_eq!(order, [("a.rb", "documented"), ("a.rb", "no_sleep"), ("b.rb", "no_sleep")]);
    }
}
