//! Path arithmetic done on the path's text, the way Ruby's `File.expand_path`
//! and `Pathname#relative_path_from` do it: `..` removes the component before
//! it, and symlinks are never resolved.

use std::path::{Component, Path, PathBuf};

/// Removes `.` components and folds `..` into the component before it.
pub fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push(component);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// The path to `path` from `base`, both absolute and normalized, with `/`
/// between components: "." for `base` itself, and starting with ".." when
/// `path` is outside `base`. `None` when they share no root, such as two
/// Windows drives.
pub fn relative(path: &Path, base: &Path) -> Option<String> {
    let path: Vec<Component> = path.components().collect();
    let base: Vec<Component> = base.components().collect();
    if path.first() != base.first() {
        return None;
    }
    let common = path.iter().zip(&base).take_while(|(a, b)| a == b).count();

    let mut parts: Vec<String> = vec!["..".to_string(); base.len() - common];
    parts.extend(path[common..].iter().map(|c| c.as_os_str().to_string_lossy().into_owned()));
    Some(if parts.is_empty() { ".".to_string() } else { parts.join("/") })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_folds_dots_without_touching_the_filesystem() {
        assert_eq!(normalize(Path::new("/a/b/../c/./d")), PathBuf::from("/a/c/d"));
        assert_eq!(normalize(Path::new("/a/..")), PathBuf::from("/"));
    }

    #[test]
    fn relative_paths_use_forward_slashes() {
        assert_eq!(relative(Path::new("/root/app/x.rb"), Path::new("/root")).as_deref(), Some("app/x.rb"));
        assert_eq!(relative(Path::new("/root"), Path::new("/root")).as_deref(), Some("."));
        assert_eq!(relative(Path::new("/other/x.rb"), Path::new("/root")).as_deref(), Some("../other/x.rb"));
        assert_eq!(relative(Path::new("/rooted/x.rb"), Path::new("/root")).as_deref(), Some("../rooted/x.rb"));
    }
}
