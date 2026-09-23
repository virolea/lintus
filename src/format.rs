//! Renders a report as text, GitHub Actions workflow commands, or JSON.

use serde_json::json;

use crate::report::Report;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// One line per offense, for people.
    Text,
    /// Workflow commands, so each offense becomes an annotation on the pull request.
    Github,
    /// For other tools; also lists the probability every rule gave every file.
    Json,
}

impl Format {
    pub const NAMES: [&str; 3] = ["text", "github", "json"];

    pub fn from_name(name: &str) -> Option<Format> {
        match name {
            "text" => Some(Format::Text),
            "github" => Some(Format::Github),
            "json" => Some(Format::Json),
            _ => None,
        }
    }

    /// Renders a sorted report, ending with a newline.
    pub fn render(self, report: &Report) -> String {
        match self {
            Format::Text => text(report),
            Format::Github => github(report),
            Format::Json => json(report),
        }
    }
}

fn text(report: &Report) -> String {
    let mut out = String::new();
    for offense in &report.offenses {
        out += &format!(
            "{}: [{}] {} (noul {})\n",
            offense.path,
            offense.rule,
            offense.message,
            ruby_float(ruby_round(offense.noul, 2))
        );
    }
    for skipped in &report.skipped {
        out += &format!("{}: skipped, {}\n", skipped.path, skipped.reason);
    }
    for failure in &report.failures {
        out += &format!("{}: failed, {}\n", failure.path, failure.error);
    }
    if !report.offenses.is_empty() || !report.skipped.is_empty() || !report.failures.is_empty() {
        out.push('\n');
    }
    out + &summary(report) + "\n"
}

fn github(report: &Report) -> String {
    let mut out = String::new();
    for offense in &report.offenses {
        let title = format!("lintus: {}", offense.rule);
        out += &command("error", &offense.message, &offense.path, &title);
    }
    for skipped in &report.skipped {
        out += &command("notice", &format!("Skipped: {}", skipped.reason), &skipped.path, "lintus");
    }
    for failure in &report.failures {
        out += &command("error", &failure.error, &failure.path, "lintus: request failed");
    }
    out + &summary(report) + "\n"
}

fn command(level: &str, message: &str, file: &str, title: &str) -> String {
    format!("::{level} file={},title={}::{}\n", escape_property(file), escape_property(title), escape_data(message))
}

fn escape_data(value: &str) -> String {
    value.replace('%', "%25").replace('\r', "%0D").replace('\n', "%0A")
}

fn escape_property(value: &str) -> String {
    escape_data(value).replace(':', "%3A").replace(',', "%2C")
}

fn json(report: &Report) -> String {
    let round = |noul: f64| json!(ruby_round(noul, 3));
    let value = json!({
        "summary": {
            "files_inspected": report.checked.len(),
            "offenses": report.offenses.len(),
            "skipped": report.skipped.len(),
            "failures": report.failures.len(),
        },
        "offenses": report.offenses.iter().map(|o| json!({
            "path": o.path, "rule": o.rule, "message": o.message, "noul": round(o.noul),
        })).collect::<Vec<_>>(),
        "files": report.checked.iter().map(|c| json!({
            "path": c.path,
            "answers": c.answers.iter().map(|(rule, noul)| (rule.clone(), round(*noul))).collect::<serde_json::Map<_, _>>(),
        })).collect::<Vec<_>>(),
        "skipped": report.skipped.iter().map(|s| json!({ "path": s.path, "reason": s.reason })).collect::<Vec<_>>(),
        "failures": report.failures.iter().map(|f| json!({ "path": f.path, "error": f.error })).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&value).expect("a report always serializes") + "\n"
}

fn summary(report: &Report) -> String {
    let mut parts = vec![
        format!("{} inspected", pluralize(report.checked.len(), "file")),
        format!("{} detected", pluralize(report.offenses.len(), "offense")),
    ];
    if !report.skipped.is_empty() {
        parts.push(format!("{} skipped", pluralize(report.skipped.len(), "file")));
    }
    if !report.failures.is_empty() {
        parts.push(format!("{} failed", pluralize(report.failures.len(), "file")));
    }
    parts.join(", ")
}

fn pluralize(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}

/// Ruby's `Float#round(digits)` for positive `digits`: rounds half away from
/// zero, correcting for the error of scaling by a power of ten.
pub fn ruby_round(x: f64, digits: i32) -> f64 {
    if x == 0.0 || !x.is_finite() {
        return x;
    }
    let binexp = frexp_exponent(x);
    // float_round_overflow: the value already has fewer digits than asked for.
    if digits >= (f64::DIGITS as i32 + 2) - if binexp > 0 { binexp / 4 } else { binexp / 3 - 1 } {
        return x;
    }
    // float_round_underflow: the value is too small to show up at all.
    if x > 0.0 && digits < -(if binexp > 0 { binexp / 3 + 1 } else { binexp / 4 }) {
        return 0.0;
    }
    let scale = 10f64.powi(digits);
    let mut rounded = (x * scale).round();
    if x > 0.0 {
        if (rounded + 0.5) / scale <= x {
            rounded += 1.0;
        }
    } else if (rounded - 0.5) / scale >= x {
        rounded -= 1.0;
    }
    rounded / scale
}

/// The exponent C's `frexp` gives: `x = m * 2^e` with `0.5 <= |m| < 1`.
fn frexp_exponent(x: f64) -> i32 {
    let bits = x.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    if exponent == 0 {
        // Subnormal: scale into the normal range first.
        return frexp_exponent(x * 2f64.powi(64)) - 64;
    }
    exponent - 1022
}

/// Ruby's `Float#to_s`: the shortest representation that reads back as the
/// same float, with at least one decimal, in exponent form outside 1e-4..1e16.
pub fn ruby_float(x: f64) -> String {
    if x.is_nan() {
        return "NaN".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.into();
    }
    let debug = format!("{x:?}");
    let Some((mantissa, exponent)) = debug.split_once('e') else { return debug };
    let mantissa = if mantissa.contains('.') { mantissa.to_string() } else { format!("{mantissa}.0") };
    let (sign, digits) = match exponent.strip_prefix('-') {
        Some(digits) => ('-', digits),
        None => ('+', exponent),
    };
    format!("{mantissa}e{sign}{digits:0>2}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::{Checked, Failure, Offense, Skipped};

    fn report() -> Report {
        let mut report = Report {
            checked: vec![
                Checked {
                    path: "lib/ok.rb".into(),
                    answers: vec![("no_sleep".into(), 0.0123), ("documented".into(), 0.9)],
                },
                Checked { path: "app/jobs/a_job.rb".into(), answers: vec![] },
            ],
            offenses: vec![
                Offense {
                    path: "app/jobs/a_job.rb".into(),
                    rule: "no_sleep".into(),
                    message: "Jobs must not sleep.".into(),
                    noul: 0.912,
                },
                Offense {
                    path: "app/jobs/a_job.rb".into(),
                    rule: "documented".into(),
                    message: "Classes carry a comment.".into(),
                    noul: 0.3,
                },
            ],
            skipped: vec![Skipped { path: "lib/blob.rb".into(), reason: "binary file".into() }],
            failures: vec![Failure { path: "lib/bad.rb".into(), error: "Jev API error 500: boom".into() }],
        };
        report.sort();
        report
    }

    #[test]
    fn text() {
        assert_eq!(
            Format::Text.render(&report()),
            "\
app/jobs/a_job.rb: [documented] Classes carry a comment. (noul 0.3)
app/jobs/a_job.rb: [no_sleep] Jobs must not sleep. (noul 0.91)
lib/blob.rb: skipped, binary file
lib/bad.rb: failed, Jev API error 500: boom

2 files inspected, 2 offenses detected, 1 file skipped, 1 file failed
"
        );
        assert_eq!(Format::Text.render(&Report::default()), "0 files inspected, 0 offenses detected\n");
    }

    #[test]
    fn github_emits_workflow_commands() {
        let output = Format::Github.render(&report());
        let lines: Vec<_> = output.lines().collect();

        assert_eq!(lines[0], "::error file=app/jobs/a_job.rb,title=lintus%3A documented::Classes carry a comment.");
        assert_eq!(lines[1], "::error file=app/jobs/a_job.rb,title=lintus%3A no_sleep::Jobs must not sleep.");
        assert_eq!(lines[2], "::notice file=lib/blob.rb,title=lintus::Skipped: binary file");
        assert_eq!(lines[3], "::error file=lib/bad.rb,title=lintus%3A request failed::Jev API error 500: boom");
        assert_eq!(lines[4], "2 files inspected, 2 offenses detected, 1 file skipped, 1 file failed");
    }

    #[test]
    fn github_escapes_newlines_and_percent_in_messages() {
        let report = Report {
            checked: vec![Checked { path: "a.rb".into(), answers: vec![] }],
            offenses: vec![Offense { path: "a.rb".into(), rule: "r".into(), message: "100%\nsure".into(), noul: 1.0 }],
            ..Report::default()
        };
        assert_eq!(
            Format::Github.render(&report),
            "::error file=a.rb,title=lintus%3A r::100%25%0Asure\n1 file inspected, 1 offense detected\n"
        );
    }

    #[test]
    fn json() {
        let data: serde_json::Value = serde_json::from_str(&Format::Json.render(&report())).unwrap();

        assert_eq!(data["summary"], json!({ "files_inspected": 2, "offenses": 2, "skipped": 1, "failures": 1 }));
        assert_eq!(
            data["offenses"][1],
            json!({ "path": "app/jobs/a_job.rb", "rule": "no_sleep", "message": "Jobs must not sleep.", "noul": 0.912 })
        );
        assert_eq!(
            data["files"],
            json!([
                { "path": "app/jobs/a_job.rb", "answers": {} },
                { "path": "lib/ok.rb", "answers": { "no_sleep": 0.012, "documented": 0.9 } }
            ])
        );
        assert_eq!(data["skipped"], json!([{ "path": "lib/blob.rb", "reason": "binary file" }]));
        assert_eq!(data["failures"][0]["error"], "Jev API error 500: boom");
    }

    #[test]
    fn ruby_float_prints_like_ruby() {
        assert_eq!(ruby_float(1.0), "1.0");
        assert_eq!(ruby_float(0.0), "0.0");
        assert_eq!(ruby_float(0.95), "0.95");
        assert_eq!(ruby_float(0.0001), "0.0001");
        assert_eq!(ruby_float(0.00001), "1.0e-05");
        assert_eq!(ruby_float(1.5e16), "1.5e+16");
        assert_eq!(ruby_float(1e15), "1000000000000000.0");
    }

    /// Every value Ruby rounded and printed, as generated by
    /// `tests/fixtures/generate_round_cases.rb`.
    #[test]
    fn rounds_and_prints_like_ruby_on_every_generated_case() {
        let cases = include_str!("../tests/fixtures/round_cases.tsv");
        let mut checked = 0;
        for line in cases.lines().filter(|line| !line.is_empty()) {
            let fields: Vec<_> = line.split('\t').collect();
            let x = f64::from_bits(u64::from_str_radix(fields[0], 16).unwrap());
            assert_eq!(ruby_float(ruby_round(x, 2)), fields[1], "{x:?}.round(2)");
            assert_eq!(ruby_float(ruby_round(x, 3)), fields[2], "{x:?}.round(3)");
            checked += 1;
        }
        assert!(checked > 5000, "only {checked} cases");
    }
}
