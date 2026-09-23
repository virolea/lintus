//! Black-box tests of the `lintus` command line: they run the binary against
//! temporary projects and a fake Jev API, and check what it prints, the exit
//! status, and what it sent. See support.rs for how to point them at another
//! implementation.

mod support;

mod cli;
mod config;
mod files;
mod output;
mod runner;

/// The config most tests use: one rule scoped to jobs, one negative rule for every Ruby file.
pub const CONFIG: &str = r#"paths: ["**/*.rb"]
rules:
  no_sleep:
    description: Jobs must not sleep.
    question: Does this file call sleep?
    paths: ["app/jobs/**/*.rb"]
  documented:
    question: Is every class documented?
    offense_when: false
"#;
