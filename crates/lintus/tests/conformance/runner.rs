//! What is sent to the API, and how answers, skips, errors and retries turn into results.

use std::time::Duration;

use crate::CONFIG;
use crate::support::{FakeJev, Project, Reply};

#[test]
fn one_request_per_file_with_every_rule_that_applies() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("app/jobs/a_job.rb", "sleep 1\n");
    project.write("lib/b.rb", "class B; end\n");
    let server = FakeJev::nouls(&[("no_sleep", 0.1), ("documented", 0.9)]);

    let out = project.lintus(&[]).api(&server).run();

    assert_eq!(out.code, 0, "{out:?}");
    let requests = server.requests_by_file();
    assert_eq!(requests.len(), 2);

    let job = &requests[0];
    assert_eq!(job.method, "POST");
    assert_eq!(job.url, "/v1/systemone");
    assert_eq!(job.authorization.as_deref(), Some("Bearer test-key"));
    assert_eq!(job.content_type.as_deref(), Some("application/json"));
    assert_eq!(job.body["model"], "jev-latest");
    assert_eq!(job.state(), "File: app/jobs/a_job.rb\n\nsleep 1\n");
    assert_eq!(job.question_ids(), ["no_sleep", "documented"]);
    assert_eq!(job.question("no_sleep"), &serde_json::json!({ "type": "noul", "instructions": "Does this file call sleep?" }));
    assert_eq!(job.body.as_object().unwrap().keys().collect::<Vec<_>>(), ["model", "state", "questions"]);

    assert_eq!(requests[1].state(), "File: lib/b.rb\n\nclass B; end\n");
    assert_eq!(requests[1].question_ids(), ["documented"]);
}

#[test]
fn binary_and_oversized_files_are_skipped_without_a_request() {
    let project = Project::new();
    project.write(".lintus.yml", format!("max_file_size: 10\n{CONFIG}"));
    project.write("lib/bin.rb", b"ab\0cd");
    project.write("lib/latin1.rb", b"caf\xe9\n");
    project.write("lib/big.rb", "x".repeat(11));
    project.write("lib/limit.rb", "x".repeat(10));
    project.write("lib/utf8.rb", "café\n");
    let server = FakeJev::nouls(&[("documented", 0.9)]);

    let out = project.lintus(&["--jobs", "1"]).api(&server).run();

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(
        out.stdout,
        "\
lib/big.rb: skipped, larger than max_file_size (11 > 10 bytes)
lib/bin.rb: skipped, binary file
lib/latin1.rb: skipped, binary file

2 files inspected, 0 offenses detected, 3 files skipped
"
    );
    let files: Vec<_> = server.requests_by_file().iter().map(|r| r.file().to_string()).collect();
    assert_eq!(files, ["lib/limit.rb", "lib/utf8.rb"]);
}

#[test]
fn the_default_max_file_size_is_100000_bytes() {
    let project = Project::new();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/big.rb", "x".repeat(100_001));
    project.write("lib/ok.rb", "x".repeat(100_000));
    let server = FakeJev::nouls(&[("documented", 0.9)]);

    let out = project.lintus(&["-j", "1"]).api(&server).run();

    assert_eq!(out.stdout, "lib/big.rb: skipped, larger than max_file_size (100001 > 100000 bytes)\n\n1 file inspected, 0 offenses detected, 1 file skipped\n");
}

#[test]
fn rate_limits_are_retried_then_succeed() {
    let project = Project::new();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/b.rb", "class B; end\n");
    let server = FakeJev::start(|request, index| match index {
        0 => Reply::status(429, "slow down"),
        _ => Reply::nouls(request, &[("documented", 0.9)]),
    });

    let out = project.lintus(&[]).api(&server).run();

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout, "1 file inspected, 0 offenses detected\n");
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn overloaded_responses_are_retried_then_succeed() {
    let project = Project::new();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/b.rb", "class B; end\n");
    let server = FakeJev::start(|request, index| match index {
        0 => Reply::status(529, "overloaded"),
        _ => Reply::nouls(request, &[("documented", 0.1)]),
    });

    let out = project.lintus(&[]).api(&server).run();

    assert_eq!(out.code, 1, "{out:?}");
    assert_eq!(server.requests().len(), 2);
}

#[test]
fn other_api_errors_fail_the_file_without_retrying() {
    for (status, body) in [(401, "bad key"), (422, "{\"error\":\"invalid\"}"), (500, "boom")] {
        let project = Project::new();
        project.write(".lintus.yml", CONFIG);
        project.write("lib/b.rb", "class B; end\n");
        let server = FakeJev::start(move |_, _| Reply::status(status, body));

        let out = project.lintus(&[]).api(&server).run();

        assert_eq!(out.code, 2, "{status}: {out:?}");
        assert_eq!(
            out.stdout,
            format!("lib/b.rb: failed, Jev API error {status}: {body}\n\n0 files inspected, 0 offenses detected, 1 file failed\n")
        );
        assert_eq!(server.requests().len(), 1, "{status}");
    }
}

#[test]
fn a_failed_file_does_not_stop_the_others() {
    let project = Project::new();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/a.rb", "class A; end\n");
    project.write("lib/b.rb", "class B; end\n");
    project.write("lib/c.rb", "class C; end\n");
    let server = FakeJev::start(|request, _| match request.file() {
        "lib/b.rb" => Reply::status(500, "boom"),
        _ => Reply::nouls(request, &[("documented", 0.1)]),
    });

    let out = project.lintus(&[]).api(&server).run();

    assert_eq!(out.code, 2, "{out:?}");
    assert_eq!(
        out.stdout,
        "\
lib/a.rb: [documented] Is every class documented? (noul 0.1)
lib/c.rb: [documented] Is every class documented? (noul 0.1)
lib/b.rb: failed, Jev API error 500: boom

2 files inspected, 2 offenses detected, 1 file failed
"
    );
}

#[test]
fn a_rule_left_unanswered_fails_the_file() {
    let project = Project::new();
    project.write(".lintus.yml", CONFIG);
    project.write("app/jobs/a_job.rb", "class AJob; end\n");
    let server = FakeJev::nouls(&[("no_sleep", 0.9)]);

    let out = project.lintus(&[]).api(&server).run();

    assert_eq!(out.code, 2, "{out:?}");
    assert_eq!(
        out.stdout,
        "app/jobs/a_job.rb: failed, documented has not been answered\n\n0 files inspected, 0 offenses detected, 1 file failed\n"
    );
}

#[test]
fn files_are_checked_concurrently_up_to_jobs() {
    let project = Project::new();
    project.write(".lintus.yml", CONFIG);
    for i in 0..6 {
        project.write(&format!("lib/f{i}.rb"), "class F; end\n");
    }
    let slow = || FakeJev::start(|request, _| Reply::nouls(request, &[("documented", 0.9)]).after(Duration::from_millis(150)));

    let server = slow();
    let out = project.lintus(&["--jobs", "3"]).api(&server).run();
    assert_eq!(out.stdout, "6 files inspected, 0 offenses detected\n", "{out:?}");
    assert_eq!(server.requests().len(), 6);
    assert!((2..=3).contains(&server.max_in_flight()), "{} in flight", server.max_in_flight());

    let server = slow();
    project.lintus(&["--jobs", "1"]).api(&server).run();
    assert_eq!(server.max_in_flight(), 1);

    let server = slow();
    project.lintus(&["--jobs", "0"]).api(&server).run();
    assert_eq!(server.max_in_flight(), 1);
    assert_eq!(server.requests().len(), 6);
}

#[test]
fn progress_is_one_line_on_stderr_off_a_terminal() {
    let project = Project::new();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/a.rb", "class A; end\n");
    project.write("lib/b.rb", "class B; end\n");
    let server = FakeJev::nouls(&[("documented", 0.9)]);

    let out = project.lintus(&[]).api(&server).run();

    assert_eq!(out.stderr, "Checking 2 files...\n");
}
