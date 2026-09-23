//! A thin wrapper around the git commands file discovery needs.

use std::path::PathBuf;
use std::process::Command;

use crate::error::{Error, Result};

/// Lists added, copied, modified and renamed files, but not deleted ones.
const CHANGE_FILTER: &str = "--diff-filter=ACMR";

pub struct Git {
    root: PathBuf,
}

impl Git {
    pub fn new(root: PathBuf) -> Git {
        Git { root }
    }

    pub fn is_repository(&self) -> bool {
        self.run(&["rev-parse", "--is-inside-work-tree"]).is_ok_and(|out| out.trim_end() == "true")
    }

    /// Tracked and untracked files, honouring .gitignore, under the given paths.
    pub fn files(&self, pathspecs: &[&str]) -> Result<Vec<String>> {
        let mut args = vec!["ls-files", "-z", "--cached", "--others", "--exclude-standard", "--"];
        args.extend_from_slice(pathspecs);
        self.lines(&args)
    }

    pub fn staged_files(&self) -> Result<Vec<String>> {
        self.lines(&["diff", "-z", "--cached", "--name-only", CHANGE_FILTER])
    }

    /// Files that differ between the working tree and the merge base of
    /// `reference` and HEAD, plus untracked files. With `reference` at HEAD
    /// this is "what I have not committed yet".
    pub fn changed_files_since(&self, reference: &str) -> Result<Vec<String>> {
        let base = self
            .run(&["merge-base", reference, "HEAD"])
            .map(|out| out.trim_end().to_string())
            .unwrap_or_else(|_| reference.to_string());
        let mut files = self.lines(&["diff", "-z", "--name-only", CHANGE_FILTER, &base])?;
        files.extend(self.lines(&["ls-files", "-z", "--others", "--exclude-standard"])?);
        Ok(files)
    }

    /// The content of a file as staged in the index.
    pub fn staged_content(&self, path: &str) -> Result<Vec<u8>> {
        self.run_bytes(&["show", &format!(":{path}")])
    }

    fn lines(&self, args: &[&str]) -> Result<Vec<String>> {
        let out = self.run_bytes(args)?;
        Ok(out
            .split(|&b| b == 0)
            .filter(|path| !path.is_empty())
            .map(|path| String::from_utf8_lossy(path).into_owned())
            .collect())
    }

    fn run(&self, args: &[&str]) -> Result<String> {
        self.run_bytes(args).map(|out| String::from_utf8_lossy(&out).into_owned())
    }

    fn run_bytes(&self, args: &[&str]) -> Result<Vec<u8>> {
        let output = Command::new("git")
            .args(args)
            .current_dir(&self.root)
            .output()
            .map_err(|e| Error::new(format!("could not run git: {e}")))?;
        if !output.status.success() {
            return Err(Error::new(format!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }
        Ok(output.stdout)
    }
}
