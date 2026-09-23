//! Arguments, subcommands and exit statuses.

use crate::CONFIG;
use crate::support::{FakeJev, Project};

#[test]
fn version() {
    let out = Project::new().lintus(&["--version"]).run();

    assert_eq!(out.code, 0);
    let version = out.stdout.strip_prefix("lintus ").unwrap().trim_end();
    assert!(version.split('.').count() == 3 && version.split('.').all(|n| n.parse::<u32>().is_ok()), "{out:?}");
    assert!(out.stdout.ends_with('\n'));
}

#[test]
fn help_lists_every_option() {
    let out = Project::new().lintus(&["--help"]).run();

    assert_eq!(out.code, 0);
    assert!(out.stdout.contains("Usage: lintus"), "{}", out.stdout);
    for flag in ["--diff", "--staged", "--config", "--format", "--jobs", "--api-key", "--list", "--version", "--help"] {
        assert!(out.stdout.contains(flag), "help does not mention {flag}:\n{}", out.stdout);
    }
}

#[test]
fn init_writes_a_working_config_and_refuses_to_overwrite() {
    let project = Project::new();

    let out = project.lintus(&["init"]).run();
    assert_eq!(out.code, 0, "{out:?}");
    let path = project.path(".lintus.yml");
    assert_eq!(out.stdout, format!("Wrote {}\n", path.display()));

    project.write("lib/a.rb", "class A; end\n");
    project.write("vendor/b.rb", "class B; end\n");
    let out = project.lintus(&["--list"]).run();
    assert_eq!(
        out.stdout,
        "lib/a.rb: no_debugging_leftovers, no_hardcoded_secrets, classes_are_documented\n1 file(s) would be checked\n"
    );

    let out = project.lintus(&["init"]).run();
    assert_eq!(out.code, 2);
    assert_eq!(out.stderr, format!("lintus: {} already exists\n", path.display()));
}

#[test]
fn list_shows_files_and_rules_without_an_api_key() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("app/jobs/a_job.rb", "class AJob; end\n");
    project.write("lib/b.rb", "class B; end\n");
    project.write("README.md", "hi\n");

    let out = project.lintus(&["--list"]).run();

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout, "app/jobs/a_job.rb: no_sleep, documented\nlib/b.rb: documented\n2 file(s) would be checked\n");
    assert_eq!(out.stderr, "");
}

#[test]
fn a_missing_api_key_is_a_usage_error() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/b.rb", "class B; end\n");
    let server = FakeJev::nouls(&[]);

    let out = project.lintus(&[]).api_without_key(&server).run();

    assert_eq!(out.code, 2);
    assert!(out.stderr.starts_with("lintus: "), "{}", out.stderr);
    assert!(out.stderr.contains("JEV_API_KEY"), "{}", out.stderr);
    assert!(server.requests().is_empty());
}

#[test]
fn an_empty_api_key_counts_as_missing() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/b.rb", "class B; end\n");
    let server = FakeJev::nouls(&[]);

    let out = project.lintus(&[]).api_without_key(&server).env("JEV_API_KEY", "").run();

    assert_eq!(out.code, 2);
    assert!(out.stderr.contains("JEV_API_KEY"), "{}", out.stderr);
}

#[test]
fn a_full_run_reports_offenses_and_exits_1() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("app/jobs/a_job.rb", "sleep 1\n");
    let server = FakeJev::nouls(&[("no_sleep", 0.95), ("documented", 0.9)]);

    let out = project.lintus(&[]).api(&server).run();

    assert_eq!(out.code, 1, "{out:?}");
    assert_eq!(out.stdout, "app/jobs/a_job.rb: [no_sleep] Jobs must not sleep. (noul 0.95)\n\n1 file inspected, 1 offense detected\n");
    assert_eq!(out.stderr, "Checking 1 file...\n");
}

#[test]
fn the_api_key_flag_wins_over_the_environment() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/b.rb", "class B; end\n");
    let server = FakeJev::nouls(&[("documented", 0.9)]);

    let out = project.lintus(&["--api-key", "from-flag"]).api(&server).run();
    assert_eq!(out.code, 0, "{out:?}");
    let out = project.lintus(&[]).api(&server).run();
    assert_eq!(out.code, 0, "{out:?}");

    let auth: Vec<_> = server.requests().into_iter().map(|r| r.authorization.unwrap()).collect();
    assert_eq!(auth, ["Bearer from-flag", "Bearer test-key"]);
}

#[test]
fn a_clean_run_exits_0() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/b.rb", "# B\nclass B; end\n");
    let server = FakeJev::nouls(&[("documented", 0.9)]);

    let out = project.lintus(&[]).api(&server).run();

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout, "1 file inspected, 0 offenses detected\n");
}

#[test]
fn nothing_to_check_sends_nothing() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("README.md", "hi\n");
    let server = FakeJev::nouls(&[]);

    let out = project.lintus(&[]).api(&server).run();

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout, "0 files inspected, 0 offenses detected\n");
    assert_eq!(out.stderr, "");
    assert!(server.requests().is_empty());
}

#[test]
fn file_arguments_cannot_be_combined_with_diff_or_staged() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);

    for flag in ["--staged", "--diff"] {
        let out = project.lintus(&[flag, "--list", "--", "lib/a.rb"]).run();
        assert_eq!(out.code, 2, "{flag}: {out:?}");
        assert!(out.stderr.contains("cannot be combined"), "{flag}: {}", out.stderr);
    }
}

#[test]
fn invalid_arguments_exit_2_with_a_message() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);

    for args in [
        &["--format", "xml"][..],
        &["--jobs", "many"],
        &["--no-such-flag"],
        &["init", "extra"],
        &["--config"],
    ] {
        let out = project.lintus(args).run();
        assert_eq!(out.code, 2, "{args:?}: {out:?}");
        assert!(out.stderr.starts_with("lintus: "), "{args:?}: {}", out.stderr);
        assert_eq!(out.stdout, "", "{args:?}");
    }
}

#[test]
fn format_takes_text_github_or_json() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/b.rb", "class B; end\n");
    let server = FakeJev::nouls(&[("documented", 0.9)]);

    for format in ["text", "github", "json"] {
        let out = project.lintus(&["--format", format]).api(&server).run();
        assert_eq!(out.code, 0, "{format}: {out:?}");
    }
    let out = project.lintus(&["-f", "json"]).api(&server).run();
    assert!(out.stdout.starts_with('{'), "{out:?}");
}

#[test]
fn the_github_format_is_the_default_under_github_actions() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/b.rb", "class B; end\n");
    let server = FakeJev::nouls(&[("documented", 0.1)]);

    let out = project.lintus(&[]).api(&server).env("GITHUB_ACTIONS", "true").run();

    assert_eq!(out.code, 1);
    assert_eq!(
        out.stdout,
        "::error file=lib/b.rb,title=lintus%3A documented::Is every class documented?\n1 file inspected, 1 offense detected\n"
    );

    let out = project.lintus(&["--format", "text"]).api(&server).env("GITHUB_ACTIONS", "true").run();
    assert!(out.stdout.starts_with("lib/b.rb: [documented]"), "{out:?}");
}

#[test]
fn short_flags() {
    let project = Project::git_repo();
    project.write("conf/custom.yml", CONFIG);
    project.write("conf/lib/b.rb", "class B; end\n");
    project.write("conf/lib/c.rb", "class C; end\n");
    project.git(&["add", "conf/lib/b.rb"]);
    let server = FakeJev::nouls(&[("documented", 0.9)]);

    let out = project.lintus(&["-c", "conf/custom.yml", "-l"]).run();
    assert_eq!(out.stdout, "lib/b.rb: documented\nlib/c.rb: documented\n2 file(s) would be checked\n");

    let out = project.lintus(&["-c", "conf/custom.yml", "-j", "2", "-f", "json"]).api(&server).run();
    assert_eq!(out.code, 0, "{out:?}");
}
