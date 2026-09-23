//! Which files a run looks at: the whole tree, --diff, --staged, explicit paths, and globs.

use crate::CONFIG;
use crate::support::{FakeJev, Project};

const ALL_FILES: &str = "rules:\n  r:\n    question: q\n";

fn listed(out: &crate::support::Output) -> Vec<&str> {
    assert_eq!(out.code, 0, "{out:?}");
    let mut lines: Vec<&str> = out.stdout.lines().collect();
    lines.pop(); // the "N file(s) would be checked" summary
    lines.into_iter().map(|line| line.rsplit_once(": ").unwrap().0).collect()
}

#[test]
fn the_whole_tree_is_tracked_and_untracked_files_minus_ignored_ones() {
    let project = Project::git_repo();
    project.write(".lintus.yml", ALL_FILES);
    project.write("lib/a.rb", "a\n");
    project.write(".gitignore", "ignored.rb\n");
    project.commit_all("init");
    project.write("untracked.rb", "u\n");
    project.write("ignored.rb", "i\n");

    let out = project.lintus(&["--list"]).run();

    assert_eq!(listed(&out), [".gitignore", ".lintus.yml", "lib/a.rb", "untracked.rb"]);
}

#[test]
fn deleted_tracked_files_are_skipped() {
    let project = Project::git_repo();
    project.write(".lintus.yml", ALL_FILES);
    project.write("lib/a.rb", "a\n");
    project.write("lib/b.rb", "b\n");
    project.commit_all("init");
    project.remove("lib/a.rb");

    assert_eq!(listed(&project.lintus(&["--list"]).run()), [".lintus.yml", "lib/b.rb"]);
    assert_eq!(listed(&project.lintus(&["--list", "--diff"]).run()), Vec::<&str>::new());
}

#[test]
fn outside_git_the_tree_is_walked_including_dotfiles() {
    let project = Project::new();
    project.write(".lintus.yml", ALL_FILES);
    project.write("a.rb", "a\n");
    project.write("sub/.hidden", "h\n");
    project.write("sub/deeper/c.rb", "c\n");
    project.mkdir("empty/dir");

    assert_eq!(listed(&project.lintus(&["--list"]).run()), [".lintus.yml", "a.rb", "sub/.hidden", "sub/deeper/c.rb"]);
}

#[test]
fn diff_uses_the_merge_base_plus_uncommitted_and_untracked_changes() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/a.rb", "a\n");
    project.write("lib/b.rb", "b\n");
    project.commit_all("init");
    let main = project.git(&["rev-parse", "--abbrev-ref", "HEAD"]);
    let main = main.trim();
    project.git(&["checkout", "-qb", "feature"]);
    project.write("lib/a.rb", "changed on feature\n");
    project.commit_all("change a");
    project.git(&["checkout", "-q", main]);
    project.write("lib/b.rb", "changed on main\n");
    project.commit_all("change b on main");
    project.git(&["checkout", "-q", "feature"]);
    project.write("lib/c.rb", "staged\n");
    project.write("lib/d.rb", "untracked\n");
    project.git(&["add", "lib/c.rb"]);

    assert_eq!(listed(&project.lintus(&["--list", "--diff", main]).run()), ["lib/a.rb", "lib/c.rb", "lib/d.rb"]);
    assert_eq!(listed(&project.lintus(&["--list", &format!("--diff={main}")]).run()), ["lib/a.rb", "lib/c.rb", "lib/d.rb"]);
    assert_eq!(listed(&project.lintus(&["--list", "--diff"]).run()), ["lib/c.rb", "lib/d.rb"]);
    assert_eq!(listed(&project.lintus(&["--list", "-d"]).run()), ["lib/c.rb", "lib/d.rb"]);
    assert_eq!(listed(&project.lintus(&["--diff", "HEAD", "--list"]).run()), ["lib/c.rb", "lib/d.rb"]);
}

#[test]
fn diff_against_an_unknown_ref_fails() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/a.rb", "a\n");
    project.commit_all("init");

    let out = project.lintus(&["--list", "--diff", "no-such-ref"]).run();

    assert_eq!(out.code, 2);
    assert!(out.stderr.starts_with("lintus: git diff"), "{}", out.stderr);
    assert!(out.stderr.contains("failed"), "{}", out.stderr);
}

#[test]
fn staged_lints_the_index_not_the_working_tree() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/a.rb", "staged\n");
    project.git(&["add", "lib/a.rb"]);
    project.write("lib/a.rb", "working tree\n");
    project.write("lib/unstaged.rb", "u\n");
    let server = FakeJev::nouls(&[("documented", 0.9)]);

    assert_eq!(listed(&project.lintus(&["--list", "--staged"]).run()), ["lib/a.rb"]);
    assert_eq!(listed(&project.lintus(&["--list", "-s"]).run()), ["lib/a.rb"]);

    let out = project.lintus(&["--staged"]).api(&server).run();
    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(out.stdout, "1 file inspected, 0 offenses detected\n");
    assert_eq!(server.requests()[0].state(), "File: lib/a.rb\n\nstaged\n");
}

#[test]
fn staged_lints_a_file_deleted_from_the_working_tree_but_not_a_deleted_one() {
    let project = Project::git_repo();
    project.write(".lintus.yml", CONFIG);
    project.write("lib/gone.rb", "committed\n");
    project.commit_all("init");
    project.write("lib/new.rb", "new\n");
    project.git(&["add", "lib/new.rb"]);
    project.remove("lib/new.rb");
    project.git(&["rm", "-q", "lib/gone.rb"]);

    assert_eq!(listed(&project.lintus(&["--list", "--staged"]).run()), ["lib/new.rb"]);
}

#[test]
fn diff_and_staged_need_a_git_repository() {
    let project = Project::new();
    project.write(".lintus.yml", CONFIG);

    for flag in ["--staged", "--diff"] {
        let out = project.lintus(&["--list", flag]).run();
        assert_eq!(out.code, 2);
        assert_eq!(
            out.stderr,
            format!("lintus: {} is not a git repository: --diff and --staged need git\n", project.root().display())
        );
    }
}

#[test]
fn explicit_paths_are_resolved_from_the_current_directory() {
    let project = Project::new();
    project.write(".lintus.yml", CONFIG);
    project.write("app/jobs/a.rb", "a\n");
    project.write("app/b.rb", "b\n");
    project.write("lib/c.rb", "c\n");

    let out = project.lintus(&["--list", "jobs", "../lib/c.rb", "missing.rb", "jobs/a.rb"]).in_dir("app").run();

    assert_eq!(listed(&out), ["app/jobs/a.rb", "lib/c.rb"]);
}

#[test]
fn explicit_paths_outside_the_root_are_rejected() {
    let project = Project::new();
    project.write("inner/.lintus.yml", CONFIG);
    project.write("outer.rb", "o\n");

    let out = project.lintus(&["--list", "../outer.rb"]).in_dir("inner").run();

    assert_eq!(out.code, 2);
    assert_eq!(
        out.stderr,
        format!("lintus: {} is outside the config root {}\n", project.path("outer.rb").display(), project.path("inner").display())
    );
}

#[test]
fn explicit_directories_honour_gitignore_inside_a_repository() {
    let project = Project::git_repo();
    project.write(".lintus.yml", ALL_FILES);
    project.write("app/a.rb", "a\n");
    project.write("app/ignored.rb", "i\n");
    project.write(".gitignore", "ignored.rb\n");
    project.commit_all("init");
    project.write("app/untracked.rb", "u\n");

    let out = project.lintus(&["--list", "app", "."]).run();

    assert_eq!(listed(&out), [".gitignore", ".lintus.yml", "app/a.rb", "app/untracked.rb"]);
}

#[test]
fn globs_follow_the_documented_rules() {
    let project = Project::new();
    project.write(
        ".lintus.yml",
        r#"
exclude: ["tmp"]
rules:
  double_star:
    question: q
    paths: ["app/**/*.rb"]
  single_star:
    question: q
    paths: ["app/*.rb"]
  bare_dir:
    question: q
    paths: ["./vendor/"]
  trailing_double_star:
    question: q
    paths: ["config/**"]
  braces:
    question: q
    paths: ["**/*.{yml,yaml}"]
  exact:
    question: q
    paths: ["Gemfile"]
  classes:
    question: q
    paths: ["lib/[a-c]?.rb", "lib/[!a-c]x.rb"]
  excluded:
    question: q
    paths: ["**/*"]
    exclude: ["**/*_test.rb", "vendor", "config/**", "*.{yml,yaml}", ".*/**", "lib", "app", "Gemfile"]
"#,
    );
    for file in [
        "app/top.rb",
        "app/jobs/nested.rb",
        "vendor/gem/lib/v.rb",
        "vendored/x.rb",
        "config/a.yaml",
        "config/deep/b.yml",
        ".github/workflows/ci.yml",
        "Gemfile",
        "sub/Gemfile",
        "lib/ab.rb",
        "lib/dx.rb",
        "lib/ax.rb",
        "unit_test.rb",
        "tmp/cache.rb",
        "other.txt",
    ] {
        project.write(file, "x\n");
    }

    let out = project.lintus(&["--list"]).run();

    assert_eq!(out.code, 0, "{out:?}");
    assert_eq!(
        out.stdout,
        "\
.github/workflows/ci.yml: braces
.lintus.yml: braces
Gemfile: exact
app/jobs/nested.rb: double_star
app/top.rb: double_star, single_star
config/a.yaml: trailing_double_star, braces
config/deep/b.yml: trailing_double_star, braces
lib/ab.rb: classes
lib/ax.rb: classes
lib/dx.rb: classes
other.txt: excluded
sub/Gemfile: excluded
vendor/gem/lib/v.rb: bare_dir
vendored/x.rb: excluded
14 file(s) would be checked
"
    );
}
