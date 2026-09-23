//! Tells the person waiting that something is happening. On a terminal it is
//! a counter updated in place and erased when done; elsewhere (CI logs, pipes)
//! it is a single line announcing the run, so nothing garbles captured output.

use std::io::Write;

pub struct Progress<W: Write> {
    out: W,
    total: usize,
    tty: bool,
    width: usize,
}

impl<W: Write> Progress<W> {
    pub fn new(out: W, total: usize, tty: bool) -> Progress<W> {
        Progress { out, total, tty, width: 0 }
    }

    pub fn start(&mut self) {
        if self.total == 0 {
            return;
        }
        if self.tty {
            self.draw(0);
        } else {
            let noun = if self.total == 1 { "file" } else { "files" };
            let _ = writeln!(self.out, "Checking {} {noun}...", self.total);
        }
    }

    pub fn update(&mut self, done: usize) {
        if self.tty {
            self.draw(done);
        }
    }

    pub fn finish(&mut self) {
        if !self.tty || self.width == 0 {
            return;
        }
        let _ = write!(self.out, "\r{}\r", " ".repeat(self.width));
        let _ = self.out.flush();
    }

    fn draw(&mut self, done: usize) {
        let line = format!("Checking {done}/{} files", self.total);
        self.width = self.width.max(line.chars().count());
        let _ = write!(self.out, "\r{line:<width$}", width = self.width);
        let _ = self.out.flush();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn on_a_terminal_redraws_in_place_and_clears_when_done() {
        let mut out = Vec::new();
        let mut progress = Progress::new(&mut out, 12, true);

        progress.start();
        progress.update(1);
        progress.update(12);
        progress.finish();

        let out = String::from_utf8(out).unwrap();
        let frames: Vec<_> = out.split('\r').filter(|frame| !frame.is_empty()).collect();
        assert_eq!(frames, ["Checking 0/12 files", "Checking 1/12 files", "Checking 12/12 files", &" ".repeat(20)]);
        assert!(out.ends_with('\r'));
    }

    #[test]
    fn off_a_terminal_prints_one_line_and_never_redraws() {
        let mut out = Vec::new();
        let mut progress = Progress::new(&mut out, 3, false);

        progress.start();
        progress.update(1);
        progress.update(3);
        progress.finish();

        assert_eq!(String::from_utf8(out).unwrap(), "Checking 3 files...\n");
    }

    #[test]
    fn says_nothing_when_there_is_nothing_to_check() {
        let mut out = Vec::new();
        let mut progress = Progress::new(&mut out, 0, true);

        progress.start();
        progress.finish();

        assert!(out.is_empty());
    }
}
