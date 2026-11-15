//! Sends each file to Jev once, with every rule that applies to it as a
//! separate noul question, and turns the answers into offenses. With
//! `locate`, then asks the rules a file broke about versions of it holding
//! only part of its changes, to find the lines that cause each offense.

use std::hash::{BuildHasher, Hasher};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use crate::config::{Config, Rule};
use crate::files::SourceFile;
use crate::hunks::{self, Lines};
use crate::report::{Checked, Failure, Offense, Report, Skipped};

/// A file and the rules that apply to it.
pub struct Task<'c> {
    pub file: SourceFile,
    pub rules: Vec<&'c Rule>,
}

/// Pairs every file with the rules that apply to it, dropping files no rule covers.
pub fn plan(config: &Config, files: Vec<SourceFile>) -> Vec<Task<'_>> {
    files
        .into_iter()
        .filter_map(|file| {
            let rules = config.rules_for(&file.path);
            (!rules.is_empty()).then_some(Task { file, rules })
        })
        .collect()
}

pub struct Runner<'c> {
    config: &'c Config,
    client: jev::Client,
    jobs: usize,
    retries: u32,
    locate: bool,
    sleep: Box<dyn Fn(Duration) + Sync + 'c>,
}

enum Outcome {
    Checked(Checked, Vec<Offense>),
    Skipped(Skipped),
    Failed(Failure),
}

impl<'c> Runner<'c> {
    pub const RETRIES: u32 = 3;
    /// Files with more hunks than this are not located: one request per hunk adds up.
    pub const MAX_HUNKS: usize = 50;
    /// Hunks with more added lines than this are located as a whole, not line by line.
    pub const MAX_LINES: usize = 20;

    /// `jobs` is how many requests may be in flight at once; anything below 1 means 1.
    pub fn new(config: &'c Config, client: jev::Client, jobs: i64) -> Runner<'c> {
        Runner {
            config,
            client,
            jobs: jobs.clamp(1, 1024) as usize,
            retries: Self::RETRIES,
            locate: false,
            sleep: Box::new(std::thread::sleep),
        }
    }

    /// Whether to find the changed lines behind each offense. Only files with
    /// a base to compare against (under `--diff` and `--staged`) are located.
    pub fn with_locate(mut self, locate: bool) -> Runner<'c> {
        self.locate = locate;
        self
    }

    #[cfg(test)]
    fn with_retries(mut self, retries: u32, sleep: impl Fn(Duration) + Sync + 'c) -> Runner<'c> {
        self.retries = retries;
        self.sleep = Box::new(sleep);
        self
    }

    /// Checks the tasks concurrently. `on_done` is called with the number of
    /// files done after each one finishes, one call at a time.
    pub fn run(&self, tasks: &[Task], on_done: impl Fn(usize) + Sync) -> Report {
        let next = AtomicUsize::new(0);
        let report = Mutex::new(Report::default());
        let done = Mutex::new(0);

        std::thread::scope(|scope| {
            for _ in 0..self.jobs.min(tasks.len()) {
                scope.spawn(|| {
                    while let Some(task) = tasks.get(next.fetch_add(1, Ordering::SeqCst)) {
                        let outcome = self.check(task);
                        {
                            let mut report = report.lock().unwrap();
                            match outcome {
                                Outcome::Checked(checked, offenses) => {
                                    report.checked.push(checked);
                                    report.offenses.extend(offenses);
                                }
                                Outcome::Skipped(skipped) => report.skipped.push(skipped),
                                Outcome::Failed(failure) => report.failures.push(failure),
                            }
                        }
                        let mut done = done.lock().unwrap();
                        *done += 1;
                        on_done(*done);
                    }
                });
            }
        });

        let mut report = report.into_inner().unwrap();
        report.sort();
        report
    }

    fn check(&self, task: &Task) -> Outcome {
        let path = &task.file.path;
        let failed = |error: String| Outcome::Failed(Failure { path: path.clone(), error });
        let skipped = |reason: String| Outcome::Skipped(Skipped { path: path.clone(), reason });

        let content = match task.file.content(&self.config.root) {
            Ok(content) => content,
            Err(error) => return failed(error),
        };
        if content.contains(&0) {
            return skipped("binary file".into());
        }
        let Ok(content) = String::from_utf8(content) else { return skipped("binary file".into()) };
        if content.len() as u64 > self.config.max_file_size {
            return skipped(format!(
                "larger than max_file_size ({} > {} bytes)",
                content.len(),
                self.config.max_file_size
            ));
        }

        let answers = match self.ask(path, &content, &task.rules) {
            Ok(answers) => answers,
            Err(error) => return failed(error),
        };

        let mut offenses = Vec::new();
        for (rule, answer) in task.rules.iter().zip(&answers) {
            if rule.is_offense(answer) {
                offenses.push(Offense {
                    path: path.clone(),
                    rule: rule.id.clone(),
                    message: rule.description.clone(),
                    noul: answer.noul,
                    lines: None,
                });
            }
        }
        if self.locate && !offenses.is_empty() {
            offenses = self.locate(task, offenses);
        }
        let answers = task.rules.iter().zip(&answers).map(|(rule, answer)| (rule.id.clone(), answer.noul)).collect();
        Outcome::Checked(Checked { path: path.clone(), answers }, offenses)
    }

    /// Asks every rule about a version of the file, answers in rule order.
    fn ask(&self, path: &str, content: &str, rules: &[&Rule]) -> Result<Vec<jev::NoulAnswer>, String> {
        let mut query = jev::Query::new(format!("File: {path}\n\n{content}"));
        for rule in rules {
            query.ask(&rule.id, rule.to_noul()).map_err(|error| error.to_string())?;
        }
        let response = self.retrying(|| self.client.perform(&query)).map_err(|error| error.to_string())?;
        rules.iter().map(|rule| response.noul(&rule.id).cloned().map_err(|error| error.to_string())).collect()
    }

    /// Which of the rules a version of the file breaks, in rule order.
    fn breaks(&self, path: &str, content: &str, rules: &[&Rule]) -> Option<Vec<bool>> {
        let answers = self.ask(path, content, rules).ok()?;
        Some(rules.iter().zip(&answers).map(|(rule, answer)| rule.is_offense(answer)).collect())
    }

    /// Pins each offense to the changed lines that cause it. A rule the base
    /// already broke is left on the whole file, since the change did not
    /// introduce it. Otherwise each hunk is applied alone to the base, and the
    /// hunks that make the rule fire on their own are blamed; within one of
    /// those, so are the added lines that do it alone, or else the whole hunk.
    /// An offense only a combination of hunks causes stays on the whole file,
    /// and so does every offense when a request fails: locating is best effort.
    fn locate(&self, task: &Task, offenses: Vec<Offense>) -> Vec<Offense> {
        let path = &task.file.path;
        let Some((base, diff)) = task.file.changes(&self.config.root) else { return offenses };
        let Ok(base) = String::from_utf8(base) else { return offenses };
        let hunks = hunks::parse(&diff);
        if hunks.is_empty() || hunks.len() > Self::MAX_HUNKS {
            return offenses;
        }
        let base_lines: Vec<&str> = base.split_inclusive('\n').collect();
        let rules: Vec<&Rule> = offenses
            .iter()
            .filter_map(|offense| task.rules.iter().copied().find(|rule| rule.id == offense.rule))
            .collect();
        if rules.len() != offenses.len() {
            return offenses;
        }

        let Some(on_base) = self.breaks(path, &base, &rules) else { return offenses };
        let introduced: Vec<usize> = (0..rules.len()).filter(|&i| !on_base[i]).collect();
        let mut found: Vec<Vec<Lines>> = vec![Vec::new(); rules.len()];

        for hunk in &hunks {
            // With a single hunk, it is what turned the base into an offense.
            let blamed = if hunks.len() == 1 {
                introduced.clone()
            } else {
                match self.blame(path, &hunk.apply(&base_lines, |_| true), &rules, &introduced) {
                    Some(blamed) => blamed,
                    None => return offenses,
                }
            };
            if blamed.is_empty() {
                continue;
            }

            let mut by_line: Vec<Vec<Lines>> = vec![Vec::new(); rules.len()];
            if (2..=Self::MAX_LINES).contains(&hunk.added.len()) {
                for index in 0..hunk.added.len() {
                    let version = hunk.apply(&base_lines, |kept| kept == index);
                    match self.blame(path, &version, &rules, &blamed) {
                        Some(lines) => lines.into_iter().for_each(|i| by_line[i].push(hunk.line(index))),
                        None => return offenses,
                    }
                }
            }
            for i in blamed {
                if by_line[i].is_empty() {
                    found[i].push(hunk.lines());
                } else {
                    found[i].append(&mut by_line[i]);
                }
            }
        }

        offenses
            .into_iter()
            .zip(found)
            .flat_map(|(offense, found)| {
                if found.is_empty() {
                    return vec![offense];
                }
                hunks::merge(found).into_iter().map(|lines| Offense { lines: Some(lines), ..offense.clone() }).collect()
            })
            .collect()
    }

    /// Which of the `candidates` (indexes into `rules`) a version of the file breaks.
    fn blame(&self, path: &str, content: &str, rules: &[&Rule], candidates: &[usize]) -> Option<Vec<usize>> {
        let asked: Vec<&Rule> = candidates.iter().map(|&i| rules[i]).collect();
        let broken = self.breaks(path, content, &asked)?;
        Some(candidates.iter().zip(broken).filter(|(_, broken)| *broken).map(|(&i, _)| i).collect())
    }

    /// Retries rate-limited and overloaded requests with exponential backoff.
    fn retrying<T>(&self, mut request: impl FnMut() -> jev::Result<T>) -> jev::Result<T> {
        let mut attempt = 0;
        loop {
            match request() {
                Err(error) if error.is_retryable() && attempt < self.retries => {
                    attempt += 1;
                    (self.sleep)(backoff(attempt));
                }
                result => return result,
            }
        }
    }
}

/// 2^attempt seconds, give or take half, so parallel workers do not retry in lockstep.
fn backoff(attempt: u32) -> Duration {
    let jitter = 0.5 + random_fraction() / 2.0;
    Duration::from_secs_f64(2f64.powi(attempt as i32) * jitter)
}

/// A number in [0, 1), from the random keys std seeds its hash maps with.
fn random_fraction() -> f64 {
    let bits = std::collections::hash_map::RandomState::new().build_hasher().finish();
    (bits >> 11) as f64 / (1u64 << 53) as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::yaml;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;

    const CONFIG: &str = "paths: ['**/*.rb']\nrules:\n  documented:\n    question: Is every class documented?\n    offense_when: false\n";

    /// A server answering every request with `status` and `body`, counting them.
    fn server(status: u16, body: &'static str) -> (String, Arc<AtomicUsize>) {
        let server = tiny_http::Server::http("127.0.0.1:0").unwrap();
        let url = format!("http://{}/", server.server_addr().to_ip().unwrap());
        let count = Arc::new(AtomicUsize::new(0));
        let counter = count.clone();
        std::thread::spawn(move || {
            for request in server.incoming_requests() {
                counter.fetch_add(1, Ordering::SeqCst);
                let _ = request.respond(tiny_http::Response::from_string(body).with_status_code(status));
            }
        });
        (url, count)
    }

    fn project(files: &[&str]) -> (tempfile::TempDir, Config) {
        let dir = tempfile::tempdir().unwrap();
        for file in files {
            std::fs::write(dir.path().join(file), "class Foo; end\n").unwrap();
        }
        let config = Config::from_yaml(yaml::load(CONFIG).unwrap(), dir.path().to_path_buf()).unwrap();
        (dir, config)
    }

    fn sources(paths: &[&str]) -> Vec<SourceFile> {
        paths.iter().map(|path| SourceFile::in_working_tree(path)).collect()
    }

    fn client(url: &str) -> jev::Client {
        jev::Client::new("key").unwrap().with_api_url(url)
    }

    #[test]
    fn plan_drops_files_no_rule_covers() {
        let (_dir, config) = project(&[]);
        let tasks = plan(&config, sources(&["a.rb", "README.md", "lib/b.rb"]));

        let planned: Vec<_> = tasks.iter().map(|task| task.file.path.as_str()).collect();
        assert_eq!(planned, ["a.rb", "lib/b.rb"]);
    }

    #[test]
    fn gives_up_after_the_retries_with_growing_backoff() {
        let (_dir, config) = project(&["a.rb"]);
        let (url, requests) = server(529, "overloaded");
        let slept = Mutex::new(Vec::new());

        let runner = Runner::new(&config, client(&url), 1).with_retries(3, |d| slept.lock().unwrap().push(d));
        let report = runner.run(&plan(&config, sources(&["a.rb"])), |_| {});

        assert_eq!(report.failures, [Failure { path: "a.rb".into(), error: "Jev API error 529: overloaded".into() }]);
        assert_eq!(requests.load(Ordering::SeqCst), 4);
        let slept = slept.lock().unwrap().clone();
        assert_eq!(slept.len(), 3);
        for (attempt, duration) in slept.iter().enumerate() {
            let base = 2f64.powi(attempt as i32 + 1);
            assert!((base * 0.5..base).contains(&duration.as_secs_f64()), "attempt {attempt}: {duration:?}");
        }
    }

    #[test]
    fn other_errors_are_not_retried() {
        let (_dir, config) = project(&["a.rb"]);
        let (url, requests) = server(401, "bad key");

        let runner = Runner::new(&config, client(&url), 1).with_retries(3, |_| panic!("should not sleep"));
        let report = runner.run(&plan(&config, sources(&["a.rb"])), |_| {});

        assert_eq!(report.failures[0].error, "Jev API error 401: bad key");
        assert_eq!(requests.load(Ordering::SeqCst), 1);
        assert_eq!(report.exit_status(), 2);
    }

    #[test]
    fn an_unreachable_api_fails_each_file() {
        let (_dir, config) = project(&["a.rb"]);
        let port = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();

        let runner = Runner::new(&config, client(&format!("http://127.0.0.1:{port}/")), 1);
        let report = runner.run(&plan(&config, sources(&["a.rb"])), |_| {});

        assert!(report.failures[0].error.starts_with("could not reach the Jev API"), "{:?}", report.failures);
    }

    #[test]
    fn unreadable_files_fail_without_a_request() {
        let (_dir, config) = project(&[]);
        let (url, requests) = server(200, "{}");

        let report = Runner::new(&config, client(&url), 1).run(&plan(&config, sources(&["gone.rb"])), |_| {});

        assert!(report.failures[0].error.starts_with("could not read gone.rb: "), "{:?}", report.failures);
        assert_eq!(requests.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn reports_progress_after_each_file() {
        let (_dir, config) = project(&["a.rb", "b.rb", "c.rb", "d.rb"]);
        let (url, _) = server(200, r#"{"answers":{"documented":{"type":"noul","noul":0.9}}}"#);
        let seen = Mutex::new(Vec::new());

        let tasks = plan(&config, sources(&["a.rb", "b.rb", "c.rb", "d.rb"]));
        let report = Runner::new(&config, client(&url), 2).run(&tasks, |done| seen.lock().unwrap().push(done));

        assert_eq!(seen.into_inner().unwrap(), [1, 2, 3, 4]);
        assert_eq!(report.checked.len(), 4);
        assert_eq!(report.checked[0].answers, [("documented".to_string(), 0.9)]);
    }

    #[test]
    fn backoff_doubles_with_jitter() {
        for attempt in 1..=3 {
            let base = 2f64.powi(attempt as i32);
            let delay = backoff(attempt).as_secs_f64();
            assert!(delay >= base * 0.5 && delay < base, "{attempt}: {delay}");
        }
    }
}
