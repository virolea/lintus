//! The parsed config file. Its directory is the root every path is relative to.

use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use crate::error::{Error, Result};
use crate::glob::{self, Glob};
use crate::paths;
use crate::yaml::{self, Value};

pub const CONFIG_FILENAMES: [&str; 4] = [".lintus.yml", "lintus.yml", ".lintus.yaml", "lintus.yaml"];
pub const DEFAULT_MAX_FILE_SIZE: u64 = 100_000;
const KEYS: [&str; 4] = ["rules", "paths", "exclude", "max_file_size"];

#[derive(Debug)]
pub struct Config {
    pub root: PathBuf,
    pub max_file_size: u64,
    pub rules: Vec<Rule>,
}

/// One entry of the `rules:` map.
///
/// A rule is a noul question, asked against every file whose path matches its
/// globs. The file is an offense when the answer equals `offense_when` (true by
/// default, so questions are phrased to describe the offense: "Does this file
/// call sleep?").
#[derive(Debug)]
pub struct Rule {
    pub id: String,
    pub description: String,
    pub question: String,
    pub criteria: Option<IndexMap<String, String>>,
    pub threshold: Option<f64>,
    pub offense_when: bool,
    paths: Vec<Glob>,
    exclude: Vec<Glob>,
}

impl Config {
    /// Reads the config at `path`, or else the nearest one found by walking up from `dir`.
    pub fn load(path: Option<&Path>, dir: &Path) -> Result<Config> {
        let path = match path {
            Some(path) => {
                if !dir.join(path).is_file() {
                    return Err(Error::new(format!("Config file not found: {}", path.display())));
                }
                paths::normalize(&dir.join(path))
            }
            None => Config::locate(dir).ok_or_else(|| {
                Error::new(format!(
                    "No config file found: looked for {} in {} and its parents",
                    CONFIG_FILENAMES.join(", "),
                    dir.display()
                ))
            })?,
        };

        let source = std::fs::read_to_string(&path)
            .map_err(|e| Error::new(format!("could not read {}: {e}", path.display())))?;
        let data = yaml::load(&source).map_err(|e| Error::new(format!("{} is not valid YAML: {e}", path.display())))?;
        let root = path.parent().expect("a config file is in a directory").to_path_buf();
        Config::from_yaml(data, root)
    }

    /// Walks up from `dir` and returns the first config file found, like git does for .git.
    pub fn locate(dir: &Path) -> Option<PathBuf> {
        dir.ancestors()
            .flat_map(|ancestor| CONFIG_FILENAMES.iter().map(move |name| ancestor.join(name)))
            .find(|candidate| candidate.is_file())
    }

    pub fn from_yaml(data: Value, root: PathBuf) -> Result<Config> {
        let data = match data {
            // Ruby reads the file with `YAML.safe_load_file(path) || {}`.
            Value::Null | Value::Bool(false) => Vec::new(),
            Value::Map(map) => string_keys(&map),
            other => return Err(Error::new(format!("config must be a map, got {}", other.class_name()))),
        };
        reject_unknown_keys(&data, &KEYS, "config")?;

        let paths = string_list(get(&data, "paths"));
        let exclude = string_list(get(&data, "exclude"));
        let max_file_size = match get(&data, "max_file_size") {
            None => DEFAULT_MAX_FILE_SIZE,
            Some(Value::Int(size)) if *size > 0 => *size as u64,
            Some(_) => return Err(Error::new("`max_file_size` must be a positive integer of bytes")),
        };

        let rules = match get(&data, "rules") {
            Some(Value::Map(rules)) if rules.is_empty() => return Err(Error::new("`rules` is empty: nothing to lint")),
            Some(Value::Map(rules)) => rules
                .iter()
                .map(|(id, attrs)| Rule::new(&id.to_s(), attrs, &paths, &exclude))
                .collect::<Result<Vec<_>>>()?,
            _ => return Err(Error::new("`rules` must be a map of rule id => attributes")),
        };

        Ok(Config { root, max_file_size, rules })
    }

    /// The rules that apply to a repository-relative path, in config order.
    pub fn rules_for(&self, path: &str) -> Vec<&Rule> {
        self.rules.iter().filter(|rule| rule.applies_to(path)).collect()
    }
}

impl Rule {
    const KEYS: [&str; 7] = ["question", "description", "criteria", "threshold", "paths", "exclude", "offense_when"];

    fn new(id: &str, attrs: &Value, default_paths: &[String], default_exclude: &[String]) -> Result<Rule> {
        let valid_id = id.starts_with(|c: char| c.is_ascii_lowercase())
            && id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !valid_id {
            return Err(Error::new(format!(
                "invalid rule id {}: use snake_case (letters, digits, underscores)",
                Value::Str(id.into()).inspect()
            )));
        }
        let fail = |message: &str| Error::new(format!("rule {id}: {message}"));

        let Value::Map(attrs) = attrs else {
            return Err(fail(&format!("expected a map of attributes, got {}", attrs.inspect())));
        };
        let attrs = string_keys(attrs);
        reject_unknown_keys(&attrs, &Rule::KEYS, &format!("rule {id}"))?;

        let question = get(&attrs, "question").map(|q| ruby_strip(&q.to_s()).to_string()).unwrap_or_default();
        if question.is_empty() {
            return Err(fail("`question` is required"));
        }
        let description = get(&attrs, "description").map_or_else(|| question.clone(), Value::to_s);

        let criteria = match get(&attrs, "criteria") {
            None | Some(Value::Null) => None,
            Some(Value::Map(criteria)) => Some(criteria.iter().map(|(k, v)| (k.to_s(), v.to_s())).collect()),
            Some(_) => return Err(fail("`criteria` must be a map with `true` and `false` keys")),
        };

        let threshold = match get(&attrs, "threshold") {
            None | Some(Value::Null) => None,
            Some(Value::Int(t)) if (0..=1).contains(t) => Some(*t as f64),
            Some(Value::Float(t)) if (0.0..=1.0).contains(t) => Some(*t),
            Some(_) => return Err(fail("`threshold` must be a number between 0 and 1")),
        };

        let paths = match get(&attrs, "paths") {
            Some(paths) => string_list(Some(paths)),
            None => default_paths.to_vec(),
        };
        let mut exclude = default_exclude.to_vec();
        exclude.extend(string_list(get(&attrs, "exclude")));

        let offense_when = match get(&attrs, "offense_when") {
            None => true,
            Some(Value::Bool(value)) => *value,
            Some(_) => return Err(fail("`offense_when` must be true or false")),
        };

        Ok(Rule {
            id: id.to_string(),
            description,
            question,
            criteria,
            threshold,
            offense_when,
            paths: paths.iter().map(|p| Glob::new(p)).collect(),
            exclude: exclude.iter().map(|p| Glob::new(p)).collect(),
        })
    }

    pub fn applies_to(&self, path: &str) -> bool {
        if glob::matches_any(&self.exclude, path) {
            return false;
        }
        self.paths.is_empty() || glob::matches_any(&self.paths, path)
    }

    /// The question as it is sent to the model.
    pub fn to_noul(&self) -> jev::Noul {
        let mut noul = jev::Noul::new(&self.question);
        if let Some(criteria) = &self.criteria {
            for (answer, meaning) in criteria {
                noul = noul.with_criterion(answer, meaning);
            }
        }
        match self.threshold {
            Some(threshold) => noul.with_threshold(threshold),
            None => noul,
        }
    }

    /// Whether an answer makes the file an offense.
    pub fn is_offense(&self, answer: &jev::NoulAnswer) -> bool {
        answer.result() == self.offense_when
    }
}

/// A map's entries with their keys as strings. Keys equal once stringified
/// collapse into one, keeping the first position and the last value.
fn string_keys(map: &yaml::Map) -> Vec<(String, Value)> {
    let mut entries: Vec<(String, Value)> = Vec::new();
    for (key, value) in map.iter() {
        let key = key.to_s();
        match entries.iter_mut().find(|(existing, _)| *existing == key) {
            Some(entry) => entry.1 = value.clone(),
            None => entries.push((key, value.clone())),
        }
    }
    entries
}

/// Ruby's `String#strip`, which only knows ASCII whitespace (and trailing NULs).
fn ruby_strip(text: &str) -> &str {
    let whitespace = |c: char| matches!(c, ' ' | '\t' | '\n' | '\x0B' | '\x0C' | '\r');
    text.trim_start_matches(whitespace).trim_end_matches(|c| whitespace(c) || c == '\0')
}

fn get<'a>(entries: &'a [(String, Value)], key: &str) -> Option<&'a Value> {
    entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

fn reject_unknown_keys(entries: &[(String, Value)], allowed: &[&str], context: &str) -> Result<()> {
    let unknown: Vec<&str> = entries.iter().map(|(k, _)| k.as_str()).filter(|k| !allowed.contains(k)).collect();
    if unknown.is_empty() {
        return Ok(());
    }
    Err(Error::new(format!("{context}: unknown key(s) {} (expected {})", unknown.join(", "), allowed.join(", "))))
}

/// Ruby's `Array(value).map(&:to_s)`: nothing, one value, or a list of them.
fn string_list(value: Option<&Value>) -> Vec<String> {
    match value {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Seq(items)) => items.iter().map(Value::to_s).collect(),
        Some(Value::Map(map)) => map.iter().map(|(k, v)| Value::Seq(vec![k.clone(), v.clone()]).to_s()).collect(),
        Some(other) => vec![other.to_s()],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: &str = r#"
paths: ["**/*.rb"]
exclude: ["vendor/**"]
rules:
  no_sleep:
    description: Jobs must not sleep.
    question: Does this file call sleep?
    paths: ["app/jobs/**/*.rb"]
  documented:
    question: Is every class documented?
    offense_when: false
    threshold: 0.7
"#;

    fn config(yaml: &str) -> Result<Config> {
        Config::from_yaml(yaml::load(yaml).unwrap(), PathBuf::from("/project"))
    }

    fn ids(rules: Vec<&Rule>) -> Vec<&str> {
        rules.into_iter().map(|rule| rule.id.as_str()).collect()
    }

    #[test]
    fn rules_for_applies_global_exclude_then_rule_globs() {
        let config = config(RULES).unwrap();

        assert_eq!(ids(config.rules_for("app/jobs/a_job.rb")), ["no_sleep", "documented"]);
        assert_eq!(ids(config.rules_for("lib/a.rb")), ["documented"]);
        assert!(config.rules_for("vendor/gem/a.rb").is_empty());
        assert!(config.rules_for("README.md").is_empty());
    }

    #[test]
    fn defaults() {
        let config = config("rules:\n  no_sleep:\n    question: Does this file call sleep?\n").unwrap();
        let rule = &config.rules[0];

        assert_eq!(config.max_file_size, DEFAULT_MAX_FILE_SIZE);
        assert_eq!(rule.description, "Does this file call sleep?");
        assert!(rule.offense_when);
        assert_eq!(rule.threshold, None);
        assert_eq!(rule.criteria, None);
        assert!(rule.applies_to("anything/at/all.txt"));
    }

    #[test]
    fn rule_paths_override_default_paths_and_exclude_adds_to_it() {
        let config = config(
            "paths: ['**/*']\nexclude: [tmp]\nrules:\n  r:\n    question: q\n    paths: app/**/*.rb\n    exclude: ['**/*_test.rb']\n",
        )
        .unwrap();
        let rule = &config.rules[0];

        assert!(rule.applies_to("app/x.rb"));
        assert!(!rule.applies_to("lib/x.rb"));
        assert!(!rule.applies_to("app/x_test.rb"));
        assert!(!rule.applies_to("tmp/app/x.rb"));
    }

    #[test]
    fn an_explicitly_empty_paths_applies_everywhere() {
        let config = config("paths: ['lib/**']\nrules:\n  r:\n    question: q\n    paths:\n").unwrap();
        assert!(config.rules[0].applies_to("app/x.rb"));
    }

    #[test]
    fn yaml_1_1_values_read_like_ruby() {
        let config = config("max_file_size: 100_000\nrules:\n  r:\n    question: q\n    offense_when: no\n").unwrap();
        assert_eq!(config.max_file_size, 100_000);
        assert!(!config.rules[0].offense_when);
    }

    #[test]
    fn to_noul_forwards_criteria_and_threshold() {
        let config = config(
            "rules:\n  r:\n    question: q?\n    criteria:\n      true: t\n      'false': f\n    threshold: 0.8\n",
        )
        .unwrap();
        let noul = config.rules[0].to_noul();

        assert_eq!(noul.instructions(), "q?");
        assert_eq!(
            noul.criteria().unwrap().iter().collect::<Vec<_>>(),
            [(&"true".to_string(), &"t".to_string()), (&"false".to_string(), &"f".to_string())]
        );
        assert_eq!(noul.threshold(), 0.8);
    }

    #[test]
    fn offense_follows_offense_when() {
        let config =
            config("rules:\n  positive:\n    question: q\n  negative:\n    question: q\n    offense_when: false\n")
                .unwrap();
        let yes = jev::NoulAnswer { noul: 0.9, threshold: 0.5 };

        assert!(config.rules[0].is_offense(&yes));
        assert!(!config.rules[1].is_offense(&yes));
    }

    #[test]
    fn locate_walks_up_and_prefers_names_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("lintus.yaml"), "").unwrap();
        std::fs::write(root.join(".lintus.yml"), "").unwrap();

        assert_eq!(Config::locate(&root.join("a/b")), Some(root.join(".lintus.yml")));
        assert_eq!(Config::locate(Path::new("/")), None);
    }
}
