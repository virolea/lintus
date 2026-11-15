//! The hunks of a diff without context lines, and the versions of a file made
//! by applying only some of them to its base. Locating an offense asks the
//! rule about those versions.

/// One run of changed lines: lines removed from the base and the lines that
/// replace them, with where each side starts.
#[derive(Debug, Clone, PartialEq)]
pub struct Hunk {
    /// First removed base line (1-based); with nothing removed, the line the
    /// additions go after (0 for the top of the file).
    old_start: usize,
    old_count: usize,
    /// First added line in the new file (1-based); with nothing added, the
    /// line the removal happened after.
    new_start: usize,
    pub added: Vec<String>,
}

/// Lines of the new file, both ends included and 1-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lines {
    pub start: usize,
    pub end: usize,
}

/// The hunks of a `git diff -U0` of a single file.
pub fn parse(diff: &str) -> Vec<Hunk> {
    let mut hunks: Vec<Hunk> = Vec::new();
    for line in diff.lines() {
        if let Some(header) = line.strip_prefix("@@ ") {
            if let Some(hunk) = parse_header(header) {
                hunks.push(hunk);
            }
        } else if let (Some(hunk), Some(added)) = (hunks.last_mut(), line.strip_prefix('+')) {
            hunk.added.push(added.to_string());
        }
    }
    hunks
}

/// `-a,b +c,d @@ …`, where a missing count means 1.
fn parse_header(header: &str) -> Option<Hunk> {
    let mut ranges = header.split_whitespace();
    let (old_start, old_count) = parse_range(ranges.next()?.strip_prefix('-')?)?;
    let (new_start, _) = parse_range(ranges.next()?.strip_prefix('+')?)?;
    Some(Hunk { old_start, old_count, new_start, added: Vec::new() })
}

fn parse_range(range: &str) -> Option<(usize, usize)> {
    match range.split_once(',') {
        Some((start, count)) => Some((start.parse().ok()?, count.parse().ok()?)),
        None => Some((range.parse().ok()?, 1)),
    }
}

impl Hunk {
    /// Where the hunk is in the new file. A pure removal points at the line
    /// before it, or the first line when it was at the top.
    pub fn lines(&self) -> Lines {
        if self.added.is_empty() {
            let line = self.new_start.max(1);
            return Lines { start: line, end: line };
        }
        Lines { start: self.new_start, end: self.new_start + self.added.len() - 1 }
    }

    /// The new-file line of the `index`th added line.
    pub fn line(&self, index: usize) -> Lines {
        let line = self.new_start + index;
        Lines { start: line, end: line }
    }

    /// The base with this hunk applied, keeping only the added lines `keep`
    /// selects. `base` is the base content split into lines, ends included.
    pub fn apply(&self, base: &[&str], keep: impl Fn(usize) -> bool) -> String {
        // Removals start at old_start; insertions alone go after it.
        let from = if self.old_count == 0 { self.old_start } else { self.old_start - 1 }.min(base.len());
        let to = (from + self.old_count).min(base.len());
        let mut out: String = base[..from].concat();
        for (index, line) in self.added.iter().enumerate() {
            if keep(index) {
                out.push_str(line);
                out.push('\n');
            }
        }
        out + &base[to..].concat()
    }
}

/// Merges sorted line ranges that touch or overlap.
pub fn merge(mut lines: Vec<Lines>) -> Vec<Lines> {
    lines.sort();
    let mut merged: Vec<Lines> = Vec::new();
    for range in lines {
        match merged.last_mut() {
            Some(last) if range.start <= last.end + 1 => last.end = last.end.max(range.end),
            _ => merged.push(range),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIFF: &str = "\
diff --git a/a.rb b/a.rb
index 1111111..2222222 100644
--- a/a.rb
+++ b/a.rb
@@ -0,0 +1,2 @@
+# top
+# comment
@@ -2 +4 @@ class A
-  old
+  new
@@ -4,2 +5,0 @@ class A
-  gone
-  gone too
@@ -6,0 +7,3 @@ end
+one
+two
+three
";

    const BASE: &str = "class A\n  old\n  kept\n  gone\n  gone too\nend\n";

    fn base() -> Vec<&'static str> {
        BASE.split_inclusive('\n').collect()
    }

    #[test]
    fn parses_every_kind_of_hunk() {
        let hunks = parse(DIFF);

        assert_eq!(hunks.len(), 4);
        assert_eq!(
            hunks[0],
            Hunk { old_start: 0, old_count: 0, new_start: 1, added: vec!["# top".into(), "# comment".into()] }
        );
        assert_eq!(hunks[1], Hunk { old_start: 2, old_count: 1, new_start: 4, added: vec!["  new".into()] });
        assert_eq!(hunks[2], Hunk { old_start: 4, old_count: 2, new_start: 5, added: vec![] });
        assert_eq!(hunks[3].added, ["one", "two", "three"]);
    }

    #[test]
    fn locates_hunks_in_the_new_file() {
        let hunks = parse(DIFF);

        assert_eq!(hunks[0].lines(), Lines { start: 1, end: 2 });
        assert_eq!(hunks[1].lines(), Lines { start: 4, end: 4 });
        assert_eq!(hunks[2].lines(), Lines { start: 5, end: 5 });
        assert_eq!(hunks[3].lines(), Lines { start: 7, end: 9 });
        assert_eq!(hunks[3].line(1), Lines { start: 8, end: 8 });
    }

    #[test]
    fn applies_one_hunk_to_the_base() {
        let hunks = parse(DIFF);
        let all = |_| true;

        assert_eq!(hunks[0].apply(&base(), all), format!("# top\n# comment\n{BASE}"));
        assert_eq!(hunks[1].apply(&base(), all), "class A\n  new\n  kept\n  gone\n  gone too\nend\n");
        assert_eq!(hunks[2].apply(&base(), all), "class A\n  old\n  kept\nend\n");
        assert_eq!(hunks[3].apply(&base(), all), format!("{BASE}one\ntwo\nthree\n"));
        assert_eq!(hunks[3].apply(&base(), |index| index == 1), format!("{BASE}two\n"));
    }

    #[test]
    fn ignores_the_no_newline_marker() {
        let hunks = parse("@@ -1 +1 @@\n-a\n\\ No newline at end of file\n+b\n\\ No newline at end of file\n");
        assert_eq!(hunks[0].added, ["b"]);
    }

    #[test]
    fn merges_touching_ranges() {
        let ranges = vec![
            Lines { start: 7, end: 7 },
            Lines { start: 1, end: 2 },
            Lines { start: 3, end: 3 },
            Lines { start: 5, end: 5 },
        ];
        assert_eq!(merge(ranges), [Lines { start: 1, end: 3 }, Lines { start: 5, end: 5 }, Lines { start: 7, end: 7 }]);
    }
}
