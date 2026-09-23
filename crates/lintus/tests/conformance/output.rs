//! The text, github and json formats, byte for byte.

use crate::CONFIG;
use crate::support::{FakeJev, Project, Reply};

/// A project with one file of each outcome: offenses, clean, skipped, failed.
fn mixed_run(format: &str) -> crate::support::Output {
    let project = Project::new();
    project.write(".lintus.yml", CONFIG);
    project.write("app/jobs/a_job.rb", "sleep 1\n");
    project.write("lib/ok.rb", "# Ok\nclass Ok; end\n");
    project.write("lib/blob.rb", b"\0\0");
    project.write("lib/bad.rb", "class Bad; end\n");
    let server = FakeJev::start(|request, _| match request.file() {
        "lib/bad.rb" => Reply::status(500, "boom"),
        "lib/ok.rb" => Reply::nouls(request, &[("documented", 0.9)]),
        _ => Reply::nouls(request, &[("no_sleep", 0.91234), ("documented", 0.3)]),
    });
    project.lintus(&["--format", format]).api(&server).run()
}

#[test]
fn text() {
    let out = mixed_run("text");

    assert_eq!(out.code, 2);
    assert_eq!(
        out.stdout,
        "\
app/jobs/a_job.rb: [documented] Is every class documented? (noul 0.3)
app/jobs/a_job.rb: [no_sleep] Jobs must not sleep. (noul 0.91)
lib/blob.rb: skipped, binary file
lib/bad.rb: failed, Jev API error 500: boom

2 files inspected, 2 offenses detected, 1 file skipped, 1 file failed
"
    );
}

#[test]
fn github() {
    let out = mixed_run("github");

    assert_eq!(out.code, 2);
    assert_eq!(
        out.stdout,
        "\
::error file=app/jobs/a_job.rb,title=lintus%3A documented::Is every class documented?
::error file=app/jobs/a_job.rb,title=lintus%3A no_sleep::Jobs must not sleep.
::notice file=lib/blob.rb,title=lintus::Skipped: binary file
::error file=lib/bad.rb,title=lintus%3A request failed::Jev API error 500: boom
2 files inspected, 2 offenses detected, 1 file skipped, 1 file failed
"
    );
}

#[test]
fn json() {
    let out = mixed_run("json");

    assert_eq!(out.code, 2);
    assert_eq!(
        out.stdout,
        r#"{
  "summary": {
    "files_inspected": 2,
    "offenses": 2,
    "skipped": 1,
    "failures": 1
  },
  "offenses": [
    {
      "path": "app/jobs/a_job.rb",
      "rule": "documented",
      "message": "Is every class documented?",
      "noul": 0.3
    },
    {
      "path": "app/jobs/a_job.rb",
      "rule": "no_sleep",
      "message": "Jobs must not sleep.",
      "noul": 0.912
    }
  ],
  "files": [
    {
      "path": "app/jobs/a_job.rb",
      "answers": {
        "no_sleep": 0.912,
        "documented": 0.3
      }
    },
    {
      "path": "lib/ok.rb",
      "answers": {
        "documented": 0.9
      }
    }
  ],
  "skipped": [
    {
      "path": "lib/blob.rb",
      "reason": "binary file"
    }
  ],
  "failures": [
    {
      "path": "lib/bad.rb",
      "error": "Jev API error 500: boom"
    }
  ]
}
"#
    );
}

#[test]
fn json_when_clean() {
    let project = Project::new();
    project.write(".lintus.yml", CONFIG);
    let server = FakeJev::nouls(&[]);

    let out = project.lintus(&["--format", "json"]).api(&server).run();

    assert_eq!(out.code, 0);
    assert_eq!(
        out.stdout,
        r#"{
  "summary": {
    "files_inspected": 0,
    "offenses": 0,
    "skipped": 0,
    "failures": 0
  },
  "offenses": [],
  "files": [],
  "skipped": [],
  "failures": []
}
"#
    );
}

#[test]
fn github_escapes_messages_and_properties() {
    let project = Project::new();
    project.write(
        ".lintus.yml",
        "paths: [lib/**]\nrules:\n  r:\n    question: q\n    description: \"100%\\nsure, really: yes\\r\"\n",
    );
    project.write("lib/a,b:c.rb", "x\n");
    project.write("lib/fail.rb", "x\n");
    let server = FakeJev::start(|request, _| match request.file() {
        "lib/fail.rb" => Reply::status(500, "line 1\nline 2: 50%"),
        _ => Reply::nouls(request, &[("r", 1.0)]),
    });

    let out = project.lintus(&["--format", "github"]).api(&server).run();

    assert_eq!(
        out.stdout,
        "\
::error file=lib/a%2Cb%3Ac.rb,title=lintus%3A r::100%25%0Asure, really: yes%0D
::error file=lib/fail.rb,title=lintus%3A request failed::Jev API error 500: line 1%0Aline 2: 50%25
1 file inspected, 1 offense detected, 1 file failed
"
    );
}

#[test]
fn probabilities_are_rounded_like_ruby_floats() {
    let cases: &[(f64, &str, &str)] = &[
        (1.0, "1.0", "1.0"),
        (0.0, "0.0", "0.0"),
        (0.999, "1.0", "0.999"),
        (0.9996, "1.0", "1.0"),
        (0.125, "0.13", "0.125"),
        (0.1, "0.1", "0.1"),
        (0.51, "0.51", "0.51"),
        (0.5049, "0.5", "0.505"),
        (0.0001, "0.0", "0.0"),
        (0.7, "0.7", "0.7"),
    ];
    let project = Project::new();
    project.write(
        ".lintus.yml",
        "paths: [\"*.rb\"]\nrules:\n  r:\n    question: q\n    threshold: 0\n    offense_when: true\n",
    );
    project.write("a.rb", "x\n");

    for &(noul, text, json) in cases {
        let server = FakeJev::start(move |request, _| Reply::nouls(request, &[("r", noul)]));
        // A threshold of 0 makes everything above 0 an offense; 0.0 itself is clean.
        let out = project.lintus(&[]).api(&server).run();
        if noul > 0.0 {
            assert_eq!(out.stdout.lines().next().unwrap(), format!("a.rb: [r] q (noul {text})"), "{noul}");
        }
        let out = project.lintus(&["--format", "json"]).api(&server).run();
        let files = out.stdout.split("\"files\": [").nth(1).unwrap();
        assert!(files.contains(&format!("\"r\": {json}\n")), "{noul}: {}", out.stdout);
    }
}
