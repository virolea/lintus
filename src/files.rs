//! Resolves which files a run looks at, always as paths relative to the config
//! root with forward slashes, in a stable order.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::git::Git;
use crate::paths;

/// A file to lint: a root-relative path, and where to read its content from.
/// Reading from the index is what lets `--staged` lint what is being committed
/// rather than the working tree.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceFile {
    pub path: String,
    from_index: bool,
}

impl SourceFile {
    #[cfg(test)]
    pub fn in_working_tree(path: &str) -> SourceFile {
        SourceFile { path: path.to_string(), from_index: false }
    }

    /// Reads the content afresh on every call, so a run holds only the files in flight.
    pub fn content(&self, root: &Path) -> Result<Vec<u8>, String> {
        let read = if self.from_index {
            Git::new(root.to_path_buf()).staged_content(&self.path).map_err(|e| e.to_string())
        } else {
            std::fs::read(root.join(&self.path)).map_err(|e| e.to_string())
        };
        read.map_err(|e| format!("could not read {}: {e}", self.path))
    }
}

pub struct FileFinder {
    root: PathBuf,
    git: Git,
}

impl FileFinder {
    pub fn new(root: PathBuf) -> FileFinder {
        FileFinder { git: Git::new(root.clone()), root }
    }

    /// Every file in the tree: git's view when inside a repository, else a walk.
    pub fn all(&self) -> Result<Vec<SourceFile>> {
        Ok(self.sources(self.files_under(&self.root)?, false))
    }

    pub fn changed_since(&self, reference: &str) -> Result<Vec<SourceFile>> {
        self.require_repository()?;
        Ok(self.sources(self.git.changed_files_since(reference)?, false))
    }

    pub fn staged(&self) -> Result<Vec<SourceFile>> {
        self.require_repository()?;
        Ok(self.sources(self.git.staged_files()?, true))
    }

    /// Paths given on the command line, resolved against `from`.
    pub fn explicit(&self, paths: &[PathBuf], from: &Path) -> Result<Vec<SourceFile>> {
        let mut files = Vec::new();
        for path in paths {
            files.extend(self.files_under(&paths::normalize(&from.join(path)))?);
        }
        Ok(self.sources(files, false))
    }

    fn require_repository(&self) -> Result<()> {
        if self.git.is_repository() {
            return Ok(());
        }
        Err(Error::new(format!("{} is not a git repository: --diff and --staged need git", self.root.display())))
    }

    /// Root-relative paths of the files at or beneath an absolute path.
    fn files_under(&self, absolute: &Path) -> Result<Vec<String>> {
        let relative = paths::relative(absolute, &self.root)
            .filter(|relative| relative != ".." && !relative.starts_with("../"))
            .ok_or_else(|| {
                Error::new(format!("{} is outside the config root {}", absolute.display(), self.root.display()))
            })?;
        if !absolute.is_dir() {
            return Ok(vec![relative]);
        }
        if self.git.is_repository() {
            return self.git.files(&[&relative]);
        }

        let mut found = Vec::new();
        walk(absolute, "", &mut found);
        let prefix = if relative == "." { String::new() } else { format!("{relative}/") };
        Ok(found
            .into_iter()
            .filter(|path| path != ".git" && !path.starts_with(".git/"))
            .map(|path| format!("{prefix}{path}"))
            .collect())
    }

    /// Sorted and deduplicated, keeping only files that exist in the working
    /// tree unless they are read from the index.
    fn sources(&self, mut paths: Vec<String>, from_index: bool) -> Vec<SourceFile> {
        paths.sort();
        paths.dedup();
        paths
            .into_iter()
            .filter(|path| from_index || self.root.join(path).is_file())
            .map(|path| SourceFile { path, from_index })
            .collect()
    }
}

/// Everything beneath `dir`, dotfiles included, as `/`-separated paths
/// relative to it. Symlinked directories are listed but not entered.
fn walk(dir: &Path, prefix: &str, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let path = format!("{prefix}{name}");
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            walk(&entry.path(), &format!("{path}/"), found);
        }
        found.push(path);
    }
}
