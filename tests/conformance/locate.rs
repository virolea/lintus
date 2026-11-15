//! `--locate`: pinning an offense to the changed lines that cause it.

use crate::CONFIG;
use crate::support::{FakeJev, Project, Reply};

const BASE: &str = "class AJob\n  def perform\n    work\n  end\nend\n";

/// A model that sees a sleep wherever the file it is shown calls one.
fn sleep_detector() -> FakeJev {
    FakeJev::start(|request, _| {
        let file = request.state().split_once("\n\n").unwrap().1;
        let no_sleep = if file.contains("sleep") { 0.95 } else { 0.05 };
        Reply::nouls(request, &[("no_sleep", no_sleep), ("documented", 0.9)])
    })
}

/// The versions of the file the requests were about, after the first one,
/// which is the whole file. A file's requests are sent one after the other.
fn versions(server: &FakeJev) -> Vec<String> {
    server.requests().iter().skip(1).map(|r| r.state().split_once("\n\n").unwrap().1.to_string()).collect()
}

fn job_with_base() -> Project {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("app/jobs/a_job.rb", BASE);
    project.commit_all("init");
    project
}

#[test]
fn an_offense_is_pinned_to_the_line_that_causes_it() {
    let project = job_with_base();
    project.write(
        "app/jobs/a_job.rb",
        "# Does the work.\nclass AJob\n  def perform\n    log\n    sleep 1\n    work\n  end\nend\n",
    );
    let server = sleep_detector();

    let out = project.lintus(&["--diff", "--locate"]).api(&server).run();

    assert_eq!(out.code, 1, "{out:?}");
    assert_eq!(
        out.stdout,
        "app/jobs/a_job.rb:5: [no_sleep] Jobs must not sleep. (noul 0.95)\n\n1 file inspected, 1 offense detected\n"
    );
    let requests = server.requests();
    assert_eq!(requests[0].question_ids(), ["no_sleep", "documented"]);
    assert!(requests[1..].iter().all(|r| r.question_ids() == ["no_sleep"]), "only the broken rule is asked again");
    assert_eq!(
        versions(&server),
        [
            // The base, then each hunk alone, then each line of the blamed hunk alone.
            BASE,
            "# Does the work.\nclass AJob\n  def perform\n    work\n  end\nend\n",
            "class AJob\n  def perform\n    log\n    sleep 1\n    work\n  end\nend\n",
            "class AJob\n  def perform\n    log\n    work\n  end\nend\n",
            "class AJob\n  def perform\n    sleep 1\n    work\n  end\nend\n",
        ]
    );
}

#[test]
fn every_culprit_is_reported_and_adjacent_lines_merge() {
    let project = job_with_base();
    project.write(
        "app/jobs/a_job.rb",
        "class AJob\n  def perform\n    sleep 1\n    sleep 2\n    log\n    work\n  end\nend\nsleep 3\n",
    );
    let server = sleep_detector();

    let out = project.lintus(&["--diff", "--locate", "--format", "github"]).api(&server).run();

    assert_eq!(
        out.stdout,
        "\
::error file=app/jobs/a_job.rb,line=3,endLine=4,title=lintus%3A no_sleep::Jobs must not sleep.
::error file=app/jobs/a_job.rb,line=9,endLine=9,title=lintus%3A no_sleep::Jobs must not sleep.
1 file inspected, 2 offenses detected
"
    );
}

#[test]
fn an_offense_already_in_the_base_stays_on_the_file() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("app/jobs/a_job.rb", "sleep 1\n");
    project.commit_all("init");
    project.write("app/jobs/a_job.rb", "# Naps.\nsleep 1\n");
    let server = sleep_detector();

    let out = project.lintus(&["--diff", "--locate"]).api(&server).run();

    assert_eq!(
        out.stdout,
        "app/jobs/a_job.rb: [no_sleep] Jobs must not sleep. (noul 0.95)\n\n1 file inspected, 1 offense detected\n"
    );
    assert_eq!(server.requests().len(), 2, "the whole file, then the base");
}

#[test]
fn a_new_file_has_nothing_to_compare_against() {
    let project = job_with_base();
    project.write("app/jobs/b_job.rb", "sleep 1\n");
    let server = sleep_detector();

    let out = project.lintus(&["--diff", "--locate"]).api(&server).run();

    assert_eq!(
        out.stdout,
        "app/jobs/b_job.rb: [no_sleep] Jobs must not sleep. (noul 0.95)\n\n1 file inspected, 1 offense detected\n"
    );
    assert_eq!(server.requests().len(), 1);
}

#[test]
fn staged_changes_are_located_in_the_index() {
    let project = job_with_base();
    project.write("app/jobs/a_job.rb", "class AJob\n  def perform\n    sleep 1\n    work\n  end\nend\n");
    project.git(&["add", "app/jobs/a_job.rb"]);
    project.write("app/jobs/a_job.rb", "# Unstaged.\nclass AJob\n  def perform\n    sleep 1\n    work\n  end\nend\n");
    let server = sleep_detector();

    let out = project.lintus(&["--staged", "--locate", "--format", "json"]).api(&server).run();

    let json: serde_json::Value = serde_json::from_str(&out.stdout).unwrap();
    assert_eq!(json["offenses"][0]["line"], 3);
    assert_eq!(json["offenses"][0]["end_line"], 3);
}

#[test]
fn a_failed_request_while_locating_keeps_the_offense_on_the_file() {
    let project = job_with_base();
    project.write("app/jobs/a_job.rb", "class AJob\n  def perform\n    sleep 1\n    work\n  end\nend\n");
    let server = FakeJev::start(|request, index| match index {
        0 => Reply::nouls(request, &[("no_sleep", 0.95), ("documented", 0.9)]),
        _ => Reply::status(400, "bad request"),
    });

    let out = project.lintus(&["--diff", "--locate"]).api(&server).run();

    assert_eq!(out.code, 1);
    assert_eq!(
        out.stdout,
        "app/jobs/a_job.rb: [no_sleep] Jobs must not sleep. (noul 0.95)\n\n1 file inspected, 1 offense detected\n"
    );
}

#[test]
fn locate_needs_diff_or_staged() {
    let project = job_with_base();

    let out = project.lintus(&["--locate"]).run();

    assert_eq!(out.code, 2);
    assert_eq!(out.stderr, "lintus: --locate needs --diff or --staged: it finds offenses among the changed lines\n");
}

#[test]
fn json_file_writes_the_json_report_next_to_the_chosen_format() {
    let project = job_with_base();
    project.write("app/jobs/a_job.rb", "sleep 1\n");
    let server = sleep_detector();

    let out = project.lintus(&["--diff", "--format", "text", "--json-file", "report.json"]).api(&server).run();

    assert!(out.stdout.starts_with("app/jobs/a_job.rb: [no_sleep]"), "{out:?}");
    let json: serde_json::Value = serde_json::from_str(&project.read("report.json")).unwrap();
    assert_eq!(json["offenses"][0]["rule"], "no_sleep");
}
