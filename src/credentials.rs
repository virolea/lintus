//! Where `lintus auth login` keeps the Jev API key, and how a run finds one.
//!
//! The key comes from, in order: `--api-key`, the `JEV_API_KEY` environment
//! variable, then the credentials file, which is
//! `$XDG_CONFIG_HOME/lintus/credentials.json` when that variable is set, and
//! otherwise `~/.config/lintus/credentials.json` (`%APPDATA%\lintus\credentials.json`
//! on Windows). The file is readable by its owner only.

use std::path::{Path, PathBuf};

use serde_json::json;

use crate::error::{Error, Result};

pub const ENV_VAR: &str = "JEV_API_KEY";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    Flag,
    Env,
    File(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiKey {
    pub key: String,
    pub source: Source,
}

impl std::fmt::Display for Source {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Source::Flag => write!(f, "--api-key"),
            Source::Env => write!(f, "{ENV_VAR}"),
            Source::File(path) => write!(f, "{}", path.display()),
        }
    }
}

/// The key a run should use: the flag, else the environment, else the saved one.
pub fn resolve(flag: Option<&str>) -> Result<Option<ApiKey>> {
    if let Some(key) = flag {
        return Ok(Some(ApiKey { key: key.to_string(), source: Source::Flag }));
    }
    if let Some(key) = std::env::var(ENV_VAR).ok().filter(|key| !key.is_empty()) {
        return Ok(Some(ApiKey { key, source: Source::Env }));
    }
    Ok(load()?.map(|(key, path)| ApiKey { key, source: Source::File(path) }))
}

/// The credentials file's location, whether or not it exists.
pub fn path() -> Option<PathBuf> {
    let from_env = |name: &str| std::env::var_os(name).map(PathBuf::from).filter(|dir| dir.is_absolute());
    let config_dir = from_env("XDG_CONFIG_HOME").or_else(|| {
        if cfg!(windows) { from_env("APPDATA") } else { from_env("HOME").map(|home| home.join(".config")) }
    })?;
    Some(config_dir.join("lintus").join("credentials.json"))
}

/// The saved key and the file it was read from, if there is one.
pub fn load() -> Result<Option<(String, PathBuf)>> {
    let Some(path) = path() else { return Ok(None) };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::new(format!("could not read {}: {e}", path.display()))),
    };
    let data: serde_json::Value = serde_json::from_str(&text).map_err(|e| unreadable(&path, &e.to_string()))?;
    match data.get("jev_api_key").and_then(|key| key.as_str()) {
        Some(key) if !key.is_empty() => Ok(Some((key.to_string(), path))),
        _ => Err(unreadable(&path, "no `jev_api_key` in it")),
    }
}

fn unreadable(path: &Path, reason: &str) -> Error {
    Error::new(format!(
        "could not read the saved API key in {}: {reason}. Run `lintus auth login` to save it again",
        path.display()
    ))
}

/// Saves the key, readable by the current user only, and returns where.
pub fn save(key: &str) -> Result<PathBuf> {
    let path = path().ok_or_else(|| Error::new("could not find your home directory to save the API key in"))?;
    let fail = |e: std::io::Error| Error::new(format!("could not save the API key to {}: {e}", path.display()));

    let dir = path.parent().expect("the credentials file is in a directory");
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir).map_err(fail)?;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut file = options.open(&path).map_err(fail)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600)).map_err(fail)?;
    }

    let content = serde_json::to_string_pretty(&json!({ "jev_api_key": key })).expect("a string serializes") + "\n";
    std::io::Write::write_all(&mut file, content.as_bytes()).map_err(fail)?;
    Ok(path)
}

/// Deletes the saved key. Returns the file it was in, or `None` if there was none.
pub fn delete() -> Result<Option<PathBuf>> {
    let Some(path) = path() else { return Ok(None) };
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(Some(path)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::new(format!("could not delete {}: {e}", path.display()))),
    }
}

/// Enough of a key to recognise it, never enough to use it.
pub fn mask(key: &str) -> String {
    let chars: Vec<char> = key.chars().collect();
    if chars.len() < 12 {
        return "*".repeat(chars.len().min(8));
    }
    format!("{}…{}", chars[..3].iter().collect::<String>(), chars[chars.len() - 4..].iter().collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mask_shows_only_the_ends_of_long_keys() {
        assert_eq!(mask("jev_1234567890abcdef"), "jev…cdef");
        assert_eq!(mask("short"), "*****");
        assert_eq!(mask("elevenchars"), "********");
    }
}
