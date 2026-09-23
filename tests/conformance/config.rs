//! Finding, parsing and validating the config file.

use crate::CONFIG;
use crate::support::{FakeJev, Project};

#[test]
fn the_config_is_found_by_walking_up_and_its_directory_is_the_root() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/deep/b.rb", "class B; end\n");

    let out = project.lintus(&["--list"]).in_dir("lib/deep").run();

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout, "lib/deep/b.rb: documented\n1 file(s) would be checked\n");
}

#[test]
fn every_config_file_name_is_recognised_in_order() {
    let project = Project::new();
    project.write("a.rb", "class A; end\n");
    let names = ["lintus.yaml", ".lintus.yaml", "lintus.yml", ".lintus.yml"];

    for (i, name) in names.iter().enumerate() {
        project.write(name, format!("rules:\n  from_{i}:\n    question: q\n"));
        let out = project.lintus(&["--list", "a.rb"]).run();
        assert_eq!(out.stdout, format!("a.rb: from_{i}\n1 file(s) would be checked\n"), "{name}");
    }
}

#[test]
fn an_explicit_config_path_makes_its_directory_the_root() {
    let project = Project::new();
    project.write("elsewhere/custom.yml", CONFIG);
    project.write("elsewhere/lib/b.rb", "class B; end\n");
    project.write("lib/outside.rb", "class O; end\n");

    let out = project.lintus(&["--config", "elsewhere/custom.yml", "--list"]).run();

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout, "lib/b.rb: documented\n1 file(s) would be checked\n");
}

#[test]
fn a_missing_config_is_reported() {
    let project = Project::new();

    let out = project.lintus(&["--list"]).run();
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.starts_with(
            "lintus: No config file found: looked for .lintus.yml, lintus.yml, .lintus.yaml, lintus.yaml in "
        ),
        "{}",
        out.stderr
    );

    let out = project.lintus(&["--config", "nope.yml", "--list"]).run();
    assert_eq!(out.code, 2);
    assert_eq!(out.stderr, "lintus: Config file not found: nope.yml\n");
}

#[test]
fn invalid_yaml_is_reported() {
    let project = Project::new();
    project.write(".lintus.yml", "rules: [\n");

    let out = project.lintus(&["--list"]).run();

    assert_eq!(out.code, 2);
    let expected = format!("lintus: {} is not valid YAML: ", project.path(".lintus.yml").display());
    assert!(out.stderr.starts_with(&expected), "{}", out.stderr);
}

/// Each config and the exact error it produces.
#[test]
fn invalid_configs_are_rejected_with_a_precise_message() {
    let cases: &[(&str, &str)] = &[
        ("", "`rules` must be a map of rule id => attributes"),
        ("- a\n", "config must be a map, got Array"),
        ("just text\n", "config must be a map, got String"),
        ("rules: []\n", "`rules` must be a map of rule id => attributes"),
        ("rules: {}\n", "`rules` is empty: nothing to lint"),
        (
            "rule: {}\nrules:\n  r:\n    question: q\n",
            "config: unknown key(s) rule (expected rules, paths, exclude, max_file_size)",
        ),
        ("max_file_size: big\nrules:\n  r:\n    question: q\n", "`max_file_size` must be a positive integer of bytes"),
        ("max_file_size: 0\nrules:\n  r:\n    question: q\n", "`max_file_size` must be a positive integer of bytes"),
        ("max_file_size: 1.5\nrules:\n  r:\n    question: q\n", "`max_file_size` must be a positive integer of bytes"),
        (
            "rules:\n  No-Sleep:\n    question: q\n",
            "invalid rule id \"No-Sleep\": use snake_case (letters, digits, underscores)",
        ),
        (
            "rules:\n  1abc:\n    question: q\n",
            "invalid rule id \"1abc\": use snake_case (letters, digits, underscores)",
        ),
        ("rules:\n  r:\n    description: d\n", "rule r: `question` is required"),
        ("rules:\n  r:\n    question: '  '\n", "rule r: `question` is required"),
        ("rules:\n  r: just a string\n", "rule r: expected a map of attributes, got \"just a string\""),
        ("rules:\n  r:\n", "rule r: expected a map of attributes, got nil"),
        (
            "rules:\n  r:\n    question: q\n    severity: error\n",
            "rule r: unknown key(s) severity (expected question, description, criteria, threshold, paths, exclude, offense_when)",
        ),
        ("rules:\n  r:\n    question: q\n    threshold: 2\n", "rule r: `threshold` must be a number between 0 and 1"),
        (
            "rules:\n  r:\n    question: q\n    threshold: high\n",
            "rule r: `threshold` must be a number between 0 and 1",
        ),
        (
            "rules:\n  r:\n    question: q\n    criteria: x\n",
            "rule r: `criteria` must be a map with `true` and `false` keys",
        ),
        ("rules:\n  r:\n    question: q\n    offense_when: maybe\n", "rule r: `offense_when` must be true or false"),
    ];

    let project = Project::new();
    project.write("a.rb", "class A; end\n");
    for (yaml, message) in cases {
        project.write(".lintus.yml", yaml);
        let out = project.lintus(&["--list"]).run();
        assert_eq!(out.code, 2, "{yaml:?}: {out:?}");
        assert_eq!(out.stderr, format!("lintus: {message}\n"), "{yaml:?}");
    }
}

#[test]
fn optional_rule_attributes_have_defaults_and_accept_valid_values() {
    let project = Project::new();
    project.write("a.rb", "class A; end\n");
    project.write(
        ".lintus.yml",
        r#"
paths: ["*.rb"]
max_file_size: 5000
rules:
  bare:
    question: "  Is this padded?  "
  full:
    description: Full rule.
    question: Is this full?
    criteria:
      "true": It is.
      "false": It is not.
    threshold: 1
    paths: "*.rb"
    exclude: ["b.rb"]
    offense_when: false
"#,
    );

    let out = project.lintus(&["--list"]).run();

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout, "a.rb: bare, full\n1 file(s) would be checked\n");
}

#[test]
fn yaml_anchors_and_aliases_are_supported() {
    let project = Project::new();
    project.write("lib/a.rb", "class A; end\n");
    project.write("app/b.rb", "class B; end\n");
    project.write(
        ".lintus.yml",
        r#"
rules:
  one:
    question: q1
    paths: &ruby ["lib/**/*.rb"]
  two:
    question: q2
    paths: *ruby
"#,
    );

    let out = project.lintus(&["--list"]).run();

    assert_eq!(out.stdout, "lib/a.rb: one, two\n1 file(s) would be checked\n", "{out:?}");
}

#[test]
fn rules_are_asked_in_config_order_with_their_question_criteria_and_nothing_else() {
    let project = Project::new();
    project.write("a.rb", "class A; end\n");
    project.write(
        ".lintus.yml",
        r#"
rules:
  zeta:
    question: "  Is this the last letter?  "
    threshold: 0.9
  alpha:
    description: Described.
    question: Is this the first letter?
    criteria:
      "true": Yes it is.
      false: No it is not.
"#,
    );
    let server = FakeJev::nouls(&[("zeta", 0.1), ("alpha", 0.1)]);

    let out = project.lintus(&[]).api(&server).run();

    assert_eq!(out.code, 0, "{out:?}");
    let request = &server.requests()[0];
    assert_eq!(request.question_ids(), ["zeta", "alpha"]);
    assert_eq!(
        request.question("zeta"),
        &serde_json::json!({ "type": "noul", "instructions": "Is this the last letter?" })
    );
    assert_eq!(
        request.question("alpha"),
        &serde_json::json!({
            "type": "noul",
            "instructions": "Is this the first letter?",
            "criteria": { "true": "Yes it is.", "false": "No it is not." }
        })
    );
}

#[test]
fn threshold_and_offense_when_decide_the_offense() {
    let project = Project::new();
    project.write("a.rb", "class A; end\n");
    project.write(
        ".lintus.yml",
        r#"
paths: ["*.rb"]
rules:
  default_below:
    question: q
  default_at:
    question: q
  default_above:
    question: q
  strict_below:
    question: q
    threshold: 0.7
  strict_above:
    question: q
    threshold: 0.7
  negative_true:
    question: q
    offense_when: false
  negative_false:
    question: q
    offense_when: false
"#,
    );
    let server = FakeJev::nouls(&[
        ("default_below", 0.4),
        ("default_at", 0.5),
        ("default_above", 0.51),
        ("strict_below", 0.69),
        ("strict_above", 0.71),
        ("negative_true", 0.9),
        ("negative_false", 0.2),
    ]);

    let out = project.lintus(&[]).api(&server).run();

    assert_eq!(out.code, 1, "{out:?}");
    assert_eq!(
        out.stdout,
        "a.rb: [default_above] q (noul 0.51)\n\
         a.rb: [negative_false] q (noul 0.2)\n\
         a.rb: [strict_above] q (noul 0.71)\n\
         \n\
         1 file inspected, 3 offenses detected\n"
    );
}
