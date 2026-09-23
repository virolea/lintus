//! What the conformance tests are built on: throwaway projects, a fake Jev API,
//! and a way to run the lintus binary under test.
//!
//! The binary is the one cargo builds, unless `LINTUS_BIN` names another one
//! (a path relative to the repository root works), so the same suite can check
//! any implementation of the command line.

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use serde_json::{Value, json};
use tempfile::TempDir;

pub const API_KEY: &str = "test-key";

pub fn lintus_bin() -> PathBuf {
    match std::env::var_os("LINTUS_BIN") {
        Some(bin) => Path::new(env!("CARGO_MANIFEST_DIR")).join(bin),
        None => PathBuf::from(env!("CARGO_BIN_EXE_lintus")),
    }
}

/// A temporary directory to run lintus in, with a separate one standing in for
/// the user's config directory so no real credentials or git config leak in.
pub struct Project {
    dir: TempDir,
    home: TempDir,
    root: PathBuf,
}

impl Project {
    pub fn new() -> Self {
        let dir = tempfile::Builder::new().prefix("lintus").tempdir().unwrap();
        let home = tempfile::Builder::new().prefix("lintus-home").tempdir().unwrap();
        let root = canonical(dir.path());
        Project { dir, home, root }
    }

    /// A project that is also a git repository, with no commits yet.
    pub fn git_repo() -> Self {
        let project = Project::new();
        project.git(&["init", "-q"]);
        project.git(&["config", "user.email", "test@example.com"]);
        project.git(&["config", "user.name", "Lintus Test"]);
        project.git(&["config", "commit.gpgsign", "false"]);
        project
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The stand-in for the user's config directory (`$XDG_CONFIG_HOME`).
    pub fn config_home(&self) -> &Path {
        self.home.path()
    }

    pub fn path(&self, relative: &str) -> PathBuf {
        self.root.join(relative)
    }

    pub fn write(&self, relative: &str, content: impl AsRef<[u8]>) -> &Self {
        let path = self.path(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
        self
    }

    pub fn read(&self, relative: &str) -> String {
        std::fs::read_to_string(self.path(relative)).unwrap()
    }

    pub fn remove(&self, relative: &str) {
        std::fs::remove_file(self.path(relative)).unwrap();
    }

    pub fn mkdir(&self, relative: &str) {
        std::fs::create_dir_all(self.path(relative)).unwrap();
    }

    pub fn git(&self, args: &[&str]) -> String {
        let output = isolate_git(process::Command::new("git")).args(args).current_dir(&self.root).output().unwrap();
        assert!(output.status.success(), "git {} failed: {}", args.join(" "), String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).unwrap()
    }

    pub fn commit_all(&self, message: &str) {
        self.git(&["add", "-A"]);
        self.git(&["commit", "-qm", message]);
    }

    pub fn lintus(&self, args: &[&str]) -> Lintus {
        Lintus {
            args: args.iter().map(|arg| arg.to_string()).collect(),
            cwd: self.root.clone(),
            env: vec![("XDG_CONFIG_HOME".into(), Some(self.home.path().display().to_string()))],
            stdin: None,
        }
    }

    // Keeps the TempDir alive as long as the project.
    fn _dir(&self) -> &TempDir {
        &self.dir
    }
}

/// The path with symlinks resolved (macOS's /var is /private/var), without
/// the `\\?\` prefix Windows adds, which git and humans do not expect.
fn canonical(path: &Path) -> PathBuf {
    let path = path.canonicalize().unwrap();
    match path.to_str().and_then(|p| p.strip_prefix(r"\\?\")) {
        Some(stripped) => PathBuf::from(stripped),
        None => path,
    }
}

fn isolate_git(mut command: process::Command) -> process::Command {
    let null = if cfg!(windows) { "NUL" } else { "/dev/null" };
    command.env("GIT_CONFIG_GLOBAL", null).env("GIT_CONFIG_NOSYSTEM", "1");
    command
}

/// One invocation of the binary under test, built up then `run`.
pub struct Lintus {
    args: Vec<String>,
    cwd: PathBuf,
    env: Vec<(String, Option<String>)>,
    stdin: Option<String>,
}

impl Lintus {
    /// Runs from a subdirectory of the project instead of its root.
    pub fn in_dir(mut self, relative: &str) -> Self {
        self.cwd = self.cwd.join(relative);
        self
    }

    pub fn env(mut self, key: &str, value: &str) -> Self {
        self.env.push((key.into(), Some(value.into())));
        self
    }

    pub fn stdin(mut self, input: &str) -> Self {
        self.stdin = Some(input.into());
        self
    }

    /// Talks to the given fake server with the test API key.
    pub fn api(self, server: &FakeJev) -> Self {
        self.env("JEV_API_URL", &server.url()).env("JEV_API_KEY", API_KEY)
    }

    /// Talks to the given fake server without an API key in the environment.
    pub fn api_without_key(self, server: &FakeJev) -> Self {
        self.env("JEV_API_URL", &server.url())
    }

    pub fn run(self) -> Output {
        let mut command = isolate_git(process::Command::new(lintus_bin()));
        command.args(&self.args).current_dir(&self.cwd);
        for key in ["JEV_API_KEY", "JEV_API_URL", "GITHUB_ACTIONS", "APPDATA"] {
            command.env_remove(key);
        }
        for (key, value) in &self.env {
            match value {
                Some(value) => command.env(key, value),
                None => command.env_remove(key),
            };
        }
        command.stdin(if self.stdin.is_some() { process::Stdio::piped() } else { process::Stdio::null() });
        command.stdout(process::Stdio::piped()).stderr(process::Stdio::piped());

        let mut child = command.spawn().unwrap_or_else(|e| panic!("could not run {}: {e}", lintus_bin().display()));
        if let Some(input) = &self.stdin {
            use std::io::Write;
            child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
        }
        let output = child.wait_with_output().unwrap();
        Output {
            code: output.status.code().expect("lintus was killed by a signal"),
            stdout: String::from_utf8(output.stdout).unwrap(),
            stderr: String::from_utf8(output.stderr).unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct Output {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// A request the fake server received.
#[derive(Debug, Clone)]
pub struct Request {
    pub method: String,
    pub url: String,
    pub authorization: Option<String>,
    pub content_type: Option<String>,
    pub body: Value,
}

impl Request {
    pub fn state(&self) -> &str {
        self.body["state"].as_str().unwrap()
    }

    /// The file path lintus put at the top of the state.
    pub fn file(&self) -> &str {
        self.state().strip_prefix("File: ").unwrap().split("\n\n").next().unwrap()
    }

    pub fn question_ids(&self) -> Vec<String> {
        self.body["questions"].as_object().unwrap().keys().cloned().collect()
    }

    pub fn question(&self, id: &str) -> &Value {
        &self.body["questions"][id]
    }
}

/// What the fake server answers to one request.
pub struct Reply {
    status: u16,
    body: String,
    delay: Duration,
}

impl Reply {
    /// A successful answer giving each listed question its noul, for the
    /// questions the request actually asked.
    pub fn nouls(request: &Request, nouls: &[(&str, f64)]) -> Reply {
        let asked = request.question_ids();
        let answers: serde_json::Map<String, Value> = nouls
            .iter()
            .filter(|(id, _)| asked.iter().any(|asked| asked == id))
            .map(|(id, noul)| (id.to_string(), json!({ "type": "noul", "noul": noul })))
            .collect();
        Reply::json(200, json!({ "model": "jev-1.13.0", "answers": answers }))
    }

    pub fn json(status: u16, body: Value) -> Reply {
        Reply::status(status, &body.to_string())
    }

    pub fn status(status: u16, body: &str) -> Reply {
        Reply { status, body: body.into(), delay: Duration::ZERO }
    }

    /// Holds the response back, so concurrent requests overlap.
    pub fn after(mut self, delay: Duration) -> Reply {
        self.delay = delay;
        self
    }
}

type Handler = dyn Fn(&Request, usize) -> Reply + Send + Sync;

/// A local stand-in for the Jev API. It records every request and answers with
/// a handler, which also gets the request's index (0 for the first).
pub struct FakeJev {
    server: Arc<tiny_http::Server>,
    requests: Arc<Mutex<Vec<Request>>>,
    max_in_flight: Arc<AtomicUsize>,
}

impl FakeJev {
    pub fn start(handler: impl Fn(&Request, usize) -> Reply + Send + Sync + 'static) -> FakeJev {
        let server = Arc::new(tiny_http::Server::http("127.0.0.1:0").unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let max_in_flight = Arc::new(AtomicUsize::new(0));
        let handler: Arc<Handler> = Arc::new(handler);

        let fake = FakeJev { server: server.clone(), requests: requests.clone(), max_in_flight: max_in_flight.clone() };
        thread::spawn(move || {
            for mut incoming in server.incoming_requests() {
                let (requests, in_flight, max_in_flight, handler) =
                    (requests.clone(), in_flight.clone(), max_in_flight.clone(), handler.clone());
                thread::spawn(move || {
                    let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                    max_in_flight.fetch_max(now, Ordering::SeqCst);

                    let mut body = String::new();
                    incoming.as_reader().read_to_string(&mut body).unwrap();
                    let header = |name: &str| {
                        incoming
                            .headers()
                            .iter()
                            .find(|h| h.field.to_string().eq_ignore_ascii_case(name))
                            .map(|h| h.value.as_str().to_string())
                    };
                    let request = Request {
                        method: incoming.method().as_str().to_string(),
                        url: incoming.url().to_string(),
                        authorization: header("Authorization"),
                        content_type: header("Content-Type"),
                        body: serde_json::from_str(&body).unwrap_or(Value::Null),
                    };
                    let index = {
                        let mut requests = requests.lock().unwrap();
                        requests.push(request.clone());
                        requests.len() - 1
                    };

                    let reply = handler(&request, index);
                    thread::sleep(reply.delay);
                    in_flight.fetch_sub(1, Ordering::SeqCst);
                    let response = tiny_http::Response::from_string(reply.body)
                        .with_status_code(reply.status)
                        .with_header("Content-Type: application/json".parse::<tiny_http::Header>().unwrap());
                    let _ = incoming.respond(response);
                });
            }
        });
        fake
    }

    /// Answers every request with the given nouls.
    pub fn nouls(nouls: &[(&'static str, f64)]) -> FakeJev {
        let nouls = nouls.to_vec();
        FakeJev::start(move |request, _| Reply::nouls(request, &nouls))
    }

    pub fn url(&self) -> String {
        format!("http://{}/v1/systemone", self.server.server_addr().to_ip().unwrap())
    }

    pub fn requests(&self) -> Vec<Request> {
        self.requests.lock().unwrap().clone()
    }

    /// Requests sorted by the file they were about, since workers finish in any order.
    pub fn requests_by_file(&self) -> Vec<Request> {
        let mut requests = self.requests();
        requests.sort_by(|a, b| a.file().cmp(b.file()));
        requests
    }

    /// The most requests the server was handling at the same time.
    pub fn max_in_flight(&self) -> usize {
        self.max_in_flight.load(Ordering::SeqCst)
    }
}

impl Drop for FakeJev {
    fn drop(&mut self) {
        self.server.unblock();
    }
}
