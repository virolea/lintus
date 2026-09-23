//! `lintus auth`, and how a run picks its API key. Unlike the conformance
//! suite, these have no Ruby counterpart.

#[path = "conformance/support.rs"]
mod support;

use support::{FakeJev, Project};

const CONFIG: &str =
    "paths: ['**/*.rb']\nrules:\n  documented:\n    question: Is every class documented?\n    offense_when: false\n";

fn project() -> Project {
    let project = Project::new();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/a.rb", "class A; end\n");
    project
}

fn credentials_file(project: &Project) -> std::path::PathBuf {
    project.config_home().join("lintus/credentials.json")
}

#[test]
fn login_saves_the_key_that_runs_then_use_and_logout_deletes_it() {
    let project = project();
    let server = FakeJev::nouls(&[("documented", 0.9)]);

    let out = project.lintus(&["auth", "login"]).stdin("jev_saved_key_1234\n").run();
    assert_eq!(out.code, 0, "{out:?}");
    let file = credentials_file(&project);
    assert_eq!(out.stdout, format!("Saved the API key (jev…1234) to {}\n", file.display()));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&std::fs::read_to_string(&file).unwrap()).unwrap(),
        serde_json::json!({ "jev_api_key": "jev_saved_key_1234" })
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&file).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(file.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
    }

    let out = project.lintus(&["auth", "status"]).run();
    assert_eq!(out.code, 0);
    assert_eq!(out.stdout, format!("Using the API key saved in {} (jev…1234).\n", file.display()));

    let out = project.lintus(&[]).api_without_key(&server).run();
    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(server.requests()[0].authorization.as_deref(), Some("Bearer jev_saved_key_1234"));

    let out = project.lintus(&["auth", "logout"]).run();
    assert_eq!(out.code, 0);
    assert_eq!(out.stdout, format!("Deleted the API key saved in {}\n", file.display()));
    assert!(!file.exists());

    let out = project.lintus(&["auth", "logout"]).run();
    assert_eq!(out.stdout, "No API key was saved.\n");

    let out = project.lintus(&["auth", "status"]).run();
    assert_eq!(out.code, 1);
    assert_eq!(out.stderr, "No API key: run `lintus auth login`, or set JEV_API_KEY.\n");
}

#[test]
fn logging_in_again_replaces_the_key() {
    let project = project();

    project.lintus(&["auth", "login"]).stdin("first_key_000000").run();
    project.lintus(&["auth", "login"]).stdin("second_key_11111").run();

    let saved = std::fs::read_to_string(credentials_file(&project)).unwrap();
    assert!(saved.contains("second_key_11111") && !saved.contains("first_key"), "{saved}");
}

#[test]
fn the_environment_and_the_flag_win_over_the_saved_key() {
    let project = project();
    let server = FakeJev::nouls(&[("documented", 0.9)]);
    project.lintus(&["auth", "login"]).stdin("jev_saved_key_1234").run();

    let out = project.lintus(&["auth", "status"]).env("JEV_API_KEY", "jev_from_env_5678").run();
    assert_eq!(
        out.stdout,
        format!(
            "Using the API key from JEV_API_KEY (jev…5678).\nThe key saved in {} is not used while JEV_API_KEY is set.\n",
            credentials_file(&project).display()
        )
    );

    project.lintus(&[]).api_without_key(&server).env("JEV_API_KEY", "jev_from_env_5678").run();
    project.lintus(&["--api-key", "from-flag"]).api_without_key(&server).env("JEV_API_KEY", "env").run();
    let used: Vec<_> = server.requests().into_iter().map(|r| r.authorization.unwrap()).collect();
    assert_eq!(used, ["Bearer jev_from_env_5678", "Bearer from-flag"]);

    let out = project.lintus(&["auth", "login"]).stdin("another_key_9999").env("JEV_API_KEY", "x").run();
    assert_eq!(out.code, 0);
    assert_eq!(out.stderr, "Note: JEV_API_KEY is set in this environment, and is used instead of the saved key.\n");
}

#[test]
fn an_empty_key_is_refused() {
    let project = project();

    let out = project.lintus(&["auth", "login"]).stdin("\n").run();

    assert_eq!(out.code, 2);
    assert!(out.stderr.starts_with("lintus: no API key given"), "{}", out.stderr);
    assert!(!credentials_file(&project).exists());
}

#[test]
fn without_any_key_the_error_says_how_to_get_one() {
    let project = project();
    let server = FakeJev::nouls(&[]);

    let out = project.lintus(&[]).api_without_key(&server).run();

    assert_eq!(out.code, 2);
    assert_eq!(
        out.stderr,
        "lintus: no API key: set JEV_API_KEY, pass --api-key, or run `lintus auth login` to save one\n"
    );
}

#[test]
fn a_damaged_credentials_file_is_reported_when_the_key_is_needed() {
    let project = project();
    let file = credentials_file(&project);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, "not json").unwrap();
    let server = FakeJev::nouls(&[]);

    let out = project.lintus(&["--list"]).run();
    assert_eq!(out.code, 0, "--list needs no key: {out:?}");

    let out = project.lintus(&[]).api_without_key(&server).run();
    assert_eq!(out.code, 2);
    assert!(
        out.stderr.starts_with(&format!("lintus: could not read the saved API key in {}: ", file.display())),
        "{}",
        out.stderr
    );
    assert!(out.stderr.contains("lintus auth login"), "{}", out.stderr);
}

#[test]
fn auth_without_a_subcommand_shows_its_help() {
    let out = Project::new().lintus(&["auth"]).run();

    assert_eq!(out.code, 2);
    for command in ["login", "status", "logout"] {
        assert!(out.stderr.contains(command), "{}", out.stderr);
    }
}
