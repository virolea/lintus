//! Matches repository-relative paths against the globs used in the config file.
//!
//! Patterns follow Ruby's `File.fnmatch` with `FNM_PATHNAME | FNM_EXTGLOB |
//! FNM_DOTMATCH`: `*`, `?` and `[...]` never cross a `/`, `**/` spans any
//! number of directories, `{a,b}` is alternation, and wildcards match dotfiles.
//! On top of that, a bare directory (`vendor` or `vendor/`) and a trailing `**`
//! (`vendor/**`) match everything beneath it.
//!
//! The matcher is a port of the one in Ruby's dir.c rather than a glob crate,
//! so that a config file matches exactly the files it matched under Ruby.

#[derive(Debug, Clone, PartialEq)]
pub struct Glob {
    /// Every pattern this glob stands for, after the directory conveniences
    /// and brace expansion. A path matches if it matches any of them.
    alternatives: Vec<Vec<char>>,
}

impl Glob {
    pub fn new(pattern: &str) -> Glob {
        let mut alternatives = Vec::new();
        for expanded in expand(pattern) {
            brace_expand(&expanded.chars().collect::<Vec<_>>(), &mut alternatives);
        }
        Glob { alternatives }
    }

    pub fn matches(&self, path: &str) -> bool {
        let path: Vec<char> = path.chars().collect();
        self.alternatives.iter().any(|pattern| fnmatch(pattern, &path))
    }
}

pub fn matches_any(globs: &[Glob], path: &str) -> bool {
    globs.iter().any(|glob| glob.matches(path))
}

/// The directory conveniences: "./vendor/" is "vendor", which also matches
/// "vendor/**/*"; "vendor/**" also matches "vendor/**/*".
fn expand(pattern: &str) -> Vec<String> {
    let pattern = pattern.strip_prefix("./").unwrap_or(pattern);
    let pattern = pattern.strip_suffix('/').unwrap_or(pattern);

    if pattern.ends_with("**") {
        vec![pattern.to_string(), format!("{pattern}/*")]
    } else if pattern.contains(['*', '?', '[', '{']) {
        vec![pattern.to_string()]
    } else {
        vec![pattern.to_string(), format!("{pattern}/**/*")]
    }
}

/// Expands the first top-level `{...}` group into one pattern per
/// alternative, recursively. A `{` without its `}` yields nothing at all,
/// as in Ruby, so such a pattern never matches.
fn brace_expand(pattern: &[char], out: &mut Vec<Vec<char>>) {
    let (mut lbrace, mut rbrace) = (None, None);
    let mut nest = 0;
    let mut i = 0;
    while i < pattern.len() {
        let c = pattern[i];
        if c == '{' {
            if nest == 0 {
                lbrace = Some(i);
            }
            nest += 1;
        }
        if c == '}' && lbrace.is_some() {
            nest -= 1;
            if nest == 0 {
                rbrace = Some(i);
                break;
            }
        }
        if c == '\\' {
            i += 1;
            if i >= pattern.len() {
                break;
            }
        }
        i += 1;
    }

    match (lbrace, rbrace) {
        (Some(left), Some(right)) => {
            let mut p = left;
            while p < right {
                p += 1;
                let start = p;
                let mut nest = 0;
                while p < right && !(pattern[p] == ',' && nest == 0) {
                    match pattern[p] {
                        '{' => nest += 1,
                        '}' => nest -= 1,
                        '\\' => {
                            p += 1;
                            if p == right {
                                break;
                            }
                        }
                        _ => {}
                    }
                    p += 1;
                }
                let mut alternative = pattern[..left].to_vec();
                alternative.extend_from_slice(&pattern[start..p]);
                alternative.extend_from_slice(&pattern[right + 1..]);
                brace_expand(&alternative, out);
            }
        }
        (None, None) => out.push(pattern.to_vec()),
        _ => {}
    }
}

/// Ruby's `fnmatch` in pathname mode: matches segment by segment, and on a
/// `**/` retries the rest of the pattern at each deeper directory.
fn fnmatch(pattern: &[char], path: &[char]) -> bool {
    let (mut p, mut s) = (0, 0);
    let mut star_star: Option<(usize, usize)> = None;

    loop {
        if pattern[p..].starts_with(&['*', '*', '/']) {
            while pattern[p..].starts_with(&['*', '*', '/']) {
                p += 3;
            }
            star_star = Some((p, s));
        }

        let (matched, next_p, mut next_s) = match_segment(pattern, p, path, s);
        if matched {
            while next_s < path.len() && path[next_s] != '/' {
                next_s += 1;
            }
            if next_p < pattern.len() && next_s < path.len() {
                p = next_p + 1;
                s = next_s + 1;
                continue;
            }
            if next_p >= pattern.len() && next_s >= path.len() {
                return true;
            }
        }

        // Let the last `**/` swallow one more directory, and try again.
        if let Some((retry_p, mut retry_s)) = star_star {
            while retry_s < path.len() && path[retry_s] != '/' {
                retry_s += 1;
            }
            if retry_s < path.len() {
                retry_s += 1;
                p = retry_p;
                s = retry_s;
                star_star = Some((retry_p, retry_s));
                continue;
            }
        }
        return false;
    }
}

/// Ruby's `fnmatch_helper`: matches one path segment against one pattern
/// segment, both ending at a `/` or at their end. Returns whether they
/// matched and where each one stopped.
fn match_segment(pattern: &[char], mut p: usize, path: &[char], mut s: usize) -> (bool, usize, usize) {
    let p_end = |i: usize| i >= pattern.len() || pattern[i] == '/';
    let s_end = |i: usize| i >= path.len() || path[i] == '/';
    let unescape = |i: usize| if pattern.get(i) == Some(&'\\') { i + 1 } else { i };
    let mut star: Option<(usize, usize)> = None;

    loop {
        let mut failed = false;
        match pattern.get(p) {
            Some('*') => {
                while pattern.get(p) == Some(&'*') {
                    p += 1;
                }
                if p_end(unescape(p)) {
                    return (true, unescape(p), s);
                }
                if s_end(s) {
                    return (false, p, s);
                }
                star = Some((p, s));
                continue;
            }
            Some('?') => {
                if s_end(s) {
                    return (false, p, s);
                }
                p += 1;
                s += 1;
                continue;
            }
            Some('[') => {
                if s_end(s) {
                    return (false, p, s);
                }
                match bracket(pattern, p + 1, path[s]) {
                    Some(after) => {
                        p = after;
                        s += 1;
                        continue;
                    }
                    None => failed = true,
                }
            }
            _ => {}
        }

        if !failed {
            p = unescape(p);
            if s_end(s) {
                return (p_end(p), p, s);
            }
            if !p_end(p) && pattern[p] == path[s] {
                p += 1;
                s += 1;
                continue;
            }
        }

        // Let the last `*` swallow one more character, and try again.
        match star {
            Some((star_p, star_s)) => {
                p = star_p;
                s = star_s + 1;
                star = Some((star_p, s));
            }
            None => return (false, p, s),
        }
    }
}

/// Ruby's `bracket`: matches `c` against the class starting right after a
/// `[`, and returns the position after its `]`. `None` when `c` is not in the
/// class, or the class is not closed.
fn bracket(pattern: &[char], mut p: usize, c: char) -> Option<usize> {
    let end = pattern.len();
    if p >= end {
        return None;
    }
    let negated = pattern[p] == '!' || pattern[p] == '^';
    if negated {
        p += 1;
    }

    let mut ok = false;
    while pattern.get(p) != Some(&']') {
        let mut first = p;
        if pattern.get(first) == Some(&'\\') {
            first += 1;
        }
        if first >= end {
            return None;
        }
        p = first + 1;
        if p >= end {
            return None;
        }

        if pattern[p] == '-' && pattern.get(p + 1) != Some(&']') {
            let mut last = p + 1;
            if pattern.get(last) == Some(&'\\') {
                last += 1;
            }
            if last >= end {
                return None;
            }
            p = last + 1;
            if ok {
                continue;
            }
            if pattern[first] == c || pattern[last] == c {
                ok = true;
                continue;
            }
            if c < pattern[first] || c > pattern[last] {
                continue;
            }
        } else {
            if ok {
                continue;
            }
            if pattern[first] == c {
                ok = true;
            }
            continue;
        }
        ok = true;
    }

    if ok == negated { None } else { Some(p + 1) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(pattern: &str, path: &str) -> bool {
        Glob::new(pattern).matches(path)
    }

    #[test]
    fn double_star_spans_directories() {
        assert!(matches("app/**/*.rb", "app/jobs/foo_job.rb"));
        assert!(matches("app/**/*.rb", "app/foo.rb"));
        assert!(!matches("app/**/*.rb", "lib/foo.rb"));
    }

    #[test]
    fn bare_directory_matches_everything_beneath_it() {
        assert!(matches("vendor", "vendor/gem/lib/x.rb"));
        assert!(matches("vendor/", "vendor/x.rb"));
        assert!(matches("./vendor", "vendor/x.rb"));
        assert!(!matches("vendor", "vendored/x.rb"));
    }

    #[test]
    fn trailing_double_star_matches_nested_files() {
        assert!(matches("vendor/**", "vendor/a/b/c.rb"));
        assert!(matches("vendor/**", "vendor/c.rb"));
    }

    #[test]
    fn exact_file() {
        assert!(matches("Gemfile", "Gemfile"));
        assert!(!matches("Gemfile", "sub/Gemfile"));
    }

    #[test]
    fn braces_and_dotfiles() {
        assert!(matches("**/*.{yml,yaml}", "config/a.yaml"));
        assert!(matches("**/*.yml", ".github/workflows/ci.yml"));
        assert!(!matches("{a,b", "a"));
    }

    #[test]
    fn single_star_does_not_cross_directories() {
        assert!(!matches("app/*.rb", "app/jobs/foo.rb"));
    }

    /// Every (pattern, path) pair of a grid, and whether Ruby's
    /// `Lintus::Glob.match?` matched it, as generated by
    /// `tests/fixtures/generate_glob_cases.rb`.
    #[test]
    fn agrees_with_ruby_on_every_generated_case() {
        let cases = include_str!("../tests/fixtures/glob_cases.tsv");
        let mut checked = 0;
        for line in cases.lines().filter(|line| !line.is_empty()) {
            let mut fields = line.split('\t');
            let (pattern, path, expected) = (fields.next().unwrap(), fields.next().unwrap(), fields.next().unwrap());
            assert_eq!(matches(pattern, path), expected == "1", "pattern {pattern:?} against {path:?}");
            checked += 1;
        }
        assert!(checked > 1000, "only {checked} cases");
    }
}
