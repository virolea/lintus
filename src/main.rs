//! Lintus: a linter whose rules are plain-language questions, answered by the
//! Jev model. See the README for the command line and the config file.

mod cli;
mod config;
mod credentials;
mod error;
mod files;
mod format;
mod git;
mod glob;
mod paths;
mod progress;
mod report;
mod runner;
mod template;
mod yaml;

fn main() {
    let code = cli::run(std::env::args_os().collect());
    std::process::exit(code);
}
