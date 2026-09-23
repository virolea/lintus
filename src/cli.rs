//! The `lintus` command: parses the arguments, and runs a lint, `init` or `auth`.

use std::ffi::OsString;
use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use clap::{ArgAction, Parser, Subcommand};

use crate::config::{CONFIG_FILENAMES, Config};
use crate::credentials::{self, ApiKey, Source};
use crate::error::{Error, Result};
use crate::files::FileFinder;
use crate::format::Format;
use crate::progress::Progress;
use crate::runner::{self, Runner};
use crate::template;

const ABOUT: &str = "A linter whose rules are plain-language questions, answered by the Jev model.";

const AFTER_HELP: &str = "\
With no FILE, lints every file in the tree. FILE may be a file or a directory.

The API key comes from --api-key, else $JEV_API_KEY, else the key saved by
`lintus auth login`.

Exit status is 0 when clean, 1 when there are offenses, and 2 when a request
failed or the invocation was wrong.";

#[derive(Parser, Debug)]
#[command(
    name = "lintus",
    version,
    about = ABOUT,
    after_help = AFTER_HELP,
    override_usage = "lintus [OPTIONS] [FILE]...\n       lintus init\n       lintus auth <login|status|logout>",
    disable_version_flag = true,
    infer_long_args = true,
    args_conflicts_with_subcommands = true
)]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,

    /// Files or directories to lint
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,

    /// Only files changed since REF [default: HEAD, the uncommitted changes]
    #[arg(
        short,
        long,
        value_name = "REF",
        num_args = 0..=1,
        default_missing_value = "HEAD",
        help_heading = "Which files"
    )]
    diff: Option<String>,

    /// Only staged files, reading their staged content (for pre-commit hooks)
    #[arg(short, long, conflicts_with = "diff", help_heading = "Which files")]
    staged: bool,

    /// Config file [default: the nearest .lintus.yml from the current directory up]
    #[arg(short, long, value_name = "PATH", help_heading = "How to run")]
    config: Option<PathBuf>,

    /// Output format [default: text, or github under GitHub Actions]
    #[arg(short, long, value_name = "FORMAT", value_parser = Format::NAMES, help_heading = "How to run")]
    format: Option<String>,

    /// Concurrent requests to the Jev API
    #[arg(
        short,
        long,
        value_name = "N",
        default_value_t = 4,
        allow_negative_numbers = true,
        help_heading = "How to run"
    )]
    jobs: i64,

    /// Jev API key [default: $JEV_API_KEY, then the key saved by `lintus auth login`]
    #[arg(long, value_name = "KEY", help_heading = "How to run")]
    api_key: Option<String>,

    /// List the files and rules that would be checked, without calling the API
    #[arg(short, long, help_heading = "How to run")]
    list: bool,

    /// Print the version
    #[arg(short = 'v', long, action = ArgAction::Version)]
    version: Option<bool>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Write a starter .lintus.yml in the current directory
    Init,
    /// Save, show or delete the Jev API key lintus uses
    #[command(subcommand_required = true, arg_required_else_help = true)]
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
}

#[derive(Subcommand, Debug)]
enum AuthCommand {
    /// Save an API key, typed at a prompt or piped on stdin
    Login,
    /// Show which API key lintus would use, and where it comes from
    Status,
    /// Delete the saved API key
    Logout,
}

/// Runs the command and returns its exit status.
pub fn run(argv: Vec<OsString>) -> i32 {
    let code = match Args::try_parse_from(argv) {
        Ok(args) => {
            let result = match &args.command {
                Some(Command::Init) => init(),
                Some(Command::Auth { command }) => auth(command),
                None => lint(&args),
            };
            result.unwrap_or_else(|error| {
                eprintln!("lintus: {error}");
                2
            })
        }
        Err(error) => usage_error(error),
    };
    let _ = std::io::stdout().flush();
    code
}

/// Help and version go to stdout; anything else is a usage error on stderr.
fn usage_error(error: clap::Error) -> i32 {
    use clap::error::ErrorKind;
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            let _ = error.print();
            0
        }
        ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            eprint!("{}", error.render());
            2
        }
        _ => {
            let rendered = error.render().to_string();
            let message = rendered.split("\n\n").next().unwrap_or_default().trim_end();
            eprintln!("lintus: {}", message.strip_prefix("error: ").unwrap_or(message));
            2
        }
    }
}

fn lint(args: &Args) -> Result<i32> {
    if !args.files.is_empty() && (args.diff.is_some() || args.staged) {
        return Err(Error::new("FILE arguments cannot be combined with --diff or --staged"));
    }
    let cwd = current_dir()?;
    let format = match &args.format {
        Some(name) => Format::from_name(name).expect("clap only accepts known formats"),
        None if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") => Format::Github,
        None => Format::Text,
    };

    let config = Config::load(args.config.as_deref(), &cwd)?;
    let finder = FileFinder::new(config.root.clone());
    let files = if args.staged {
        finder.staged()?
    } else if let Some(reference) = &args.diff {
        finder.changed_since(reference)?
    } else if !args.files.is_empty() {
        finder.explicit(&args.files, &cwd)?
    } else {
        finder.all()?
    };
    let tasks = runner::plan(&config, files);

    if args.list {
        let mut out = String::new();
        for task in &tasks {
            let rules: Vec<&str> = task.rules.iter().map(|rule| rule.id.as_str()).collect();
            out += &format!("{}: {}\n", task.file.path, rules.join(", "));
        }
        out += &format!("{} file(s) would be checked\n", tasks.len());
        print(&out);
        return Ok(0);
    }

    let api_key =
        credentials::resolve(args.api_key.as_deref())?.filter(|api_key| !api_key.key.is_empty()).ok_or_else(|| {
            Error::new(format!(
                "no API key: set {}, pass --api-key, or run `lintus auth login` to save one",
                credentials::ENV_VAR
            ))
        })?;
    let mut client = jev::Client::new(api_key.key)?;
    if let Some(url) = std::env::var("JEV_API_URL").ok().filter(|url| !url.is_empty()) {
        client = client.with_api_url(url);
    }

    let stderr = std::io::stderr();
    let tty = stderr.is_terminal();
    let progress = Mutex::new(Progress::new(stderr, tasks.len(), tty));
    progress.lock().unwrap().start();
    let report = Runner::new(&config, client, args.jobs).run(&tasks, |done| progress.lock().unwrap().update(done));
    progress.lock().unwrap().finish();

    print(&format.render(&report));
    Ok(report.exit_status())
}

fn init() -> Result<i32> {
    let path = current_dir()?.join(CONFIG_FILENAMES[0]);
    if path.exists() {
        return Err(Error::new(format!("{} already exists", path.display())));
    }
    std::fs::write(&path, template::CONFIG)
        .map_err(|e| Error::new(format!("could not write {}: {e}", path.display())))?;
    print(&format!("Wrote {}\n", path.display()));
    Ok(0)
}

fn auth(command: &AuthCommand) -> Result<i32> {
    match command {
        AuthCommand::Login => login(),
        AuthCommand::Status => status(),
        AuthCommand::Logout => logout(),
    }
}

fn login() -> Result<i32> {
    let key = if std::io::stdin().is_terminal() {
        rpassword::prompt_password("Paste your Jev API key (it will not be shown): ")
            .map_err(|e| Error::new(format!("could not read the API key: {e}")))?
    } else {
        let mut input = String::new();
        std::io::stdin()
            .read_to_string(&mut input)
            .map_err(|e| Error::new(format!("could not read the API key from stdin: {e}")))?;
        input.lines().next().unwrap_or_default().to_string()
    };
    let key = key.trim();
    if key.is_empty() {
        return Err(Error::new(
            "no API key given: paste it at the prompt, or pipe it in with `lintus auth login < key.txt`",
        ));
    }

    let path = credentials::save(key)?;
    print(&format!("Saved the API key ({}) to {}\n", credentials::mask(key), path.display()));
    if env_key_is_set() {
        eprintln!("Note: {} is set in this environment, and is used instead of the saved key.", credentials::ENV_VAR);
    }
    Ok(0)
}

fn status() -> Result<i32> {
    let Some(ApiKey { key, source }) = credentials::resolve(None)? else {
        eprintln!("No API key: run `lintus auth login`, or set {}.", credentials::ENV_VAR);
        return Ok(1);
    };
    match &source {
        Source::File(path) => {
            print(&format!("Using the API key saved in {} ({}).\n", path.display(), credentials::mask(&key)))
        }
        other => {
            print(&format!("Using the API key from {other} ({}).\n", credentials::mask(&key)));
            if let Ok(Some((_, path))) = credentials::load() {
                print(&format!("The key saved in {} is not used while {other} is set.\n", path.display()));
            }
        }
    }
    Ok(0)
}

fn logout() -> Result<i32> {
    match credentials::delete()? {
        Some(path) => print(&format!("Deleted the API key saved in {}\n", path.display())),
        None => print("No API key was saved.\n"),
    }
    if env_key_is_set() {
        eprintln!("Note: {} is still set in this environment.", credentials::ENV_VAR);
    }
    Ok(0)
}

fn env_key_is_set() -> bool {
    std::env::var(credentials::ENV_VAR).is_ok_and(|key| !key.is_empty())
}

fn current_dir() -> Result<PathBuf> {
    std::env::current_dir().map_err(|e| Error::new(format!("could not read the current directory: {e}")))
}

/// Writes to stdout, ignoring a closed pipe (`lintus | head`).
fn print(text: &str) {
    let _ = std::io::stdout().lock().write_all(text.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use std::path::Path;

    #[test]
    fn the_argument_definitions_are_consistent() {
        Args::command().debug_assert();
    }

    fn parse(args: &[&str]) -> Args {
        Args::try_parse_from(std::iter::once("lintus").chain(args.iter().copied())).unwrap()
    }

    #[test]
    fn diff_takes_an_optional_ref() {
        assert_eq!(parse(&["--diff"]).diff.as_deref(), Some("HEAD"));
        assert_eq!(parse(&["--diff", "main"]).diff.as_deref(), Some("main"));
        assert_eq!(parse(&["--diff", "--list"]).diff.as_deref(), Some("HEAD"));
        assert_eq!(parse(&["-d"]).diff.as_deref(), Some("HEAD"));
        assert_eq!(parse(&[]).diff, None);
    }

    #[test]
    fn long_options_can_be_abbreviated() {
        let args = parse(&["--stag", "--form", "json"]);
        assert!(args.staged);
        assert_eq!(args.format.as_deref(), Some("json"));
    }

    #[test]
    fn subcommands_and_files() {
        assert!(matches!(parse(&["init"]).command, Some(Command::Init)));
        assert!(matches!(parse(&["auth", "login"]).command, Some(Command::Auth { command: AuthCommand::Login })));
        assert_eq!(parse(&["lib", "--", "init"]).files, [Path::new("lib"), Path::new("init")]);
    }
}
