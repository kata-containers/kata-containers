// Copyright (c) Kata Containers Community
//
// SPDX-License-Identifier: Apache-2.0
//
// Description:
// A byte meter, for transfers slow enough that the caller needs telling they
// are still moving.

use std::{
    io::{self, IsTerminal, Write},
    time::{Duration, Instant},
};

/// How often the meter may redraw: often enough to look continuous, rarely
/// enough that drawing it costs nothing next to what it is measuring.
const TICK: Duration = Duration::from_millis(100);

const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

/// Whether a meter is worth drawing. Redrawing a line a few times a second
/// only reads as progress on a terminal; anywhere else it is noise in
/// somebody's log. The test harness owns the terminal while the suite runs,
/// so it does not count as one either.
fn interactive() -> bool {
    !cfg!(test) && io::stderr().is_terminal()
}

/// Bytes moved so far, drawn over a single line of the terminal and wiped off
/// it once the transfer ends.
pub struct Progress {
    label: String,
    total: Option<u64>,
    done: u64,
    started: Instant,
    drawn: Instant,
    /// Width of the line last drawn, so a shorter one wipes what it replaces.
    width: usize,
    live: bool,
}

impl Progress {
    /// Start metering. `total` is what the transfer is expected to come to,
    /// where that is knowable; without it the meter counts but cannot say out
    /// of how much.
    pub fn new(label: impl Into<String>, total: Option<u64>) -> Self {
        let now = Instant::now();
        let mut progress = Progress {
            label: label.into(),
            total,
            done: 0,
            started: now,
            drawn: now,
            width: 0,
            live: interactive(),
        };

        // Draw once before anything has moved: the far end can be slow to
        // produce a first byte, and until it does this is the only sign that
        // something is happening.
        progress.draw(false);

        progress
    }

    pub fn add(&mut self, n: u64) {
        self.done += n;

        if self.drawn.elapsed() >= TICK {
            self.drawn = Instant::now();
            self.draw(false);
        }
    }

    fn line(&self) -> String {
        let rate = format!("{}/s", human(self.rate()));

        match self.total {
            Some(total) if total > 0 => {
                // An estimate can be beaten, and a meter reading 103% looks
                // like a bug rather than a rounding error.
                let pct = std::cmp::min(100, self.done.saturating_mul(100) / total);

                format!(
                    "{}  {} / {}  {pct:>3}%  {rate}",
                    self.label,
                    human(self.done),
                    human(total)
                )
            }
            _ => format!("{}  {}  {rate}", self.label, human(self.done)),
        }
    }

    fn rate(&self) -> u64 {
        let secs = self.started.elapsed().as_secs_f64();
        if secs <= 0.0 {
            return 0;
        }

        (self.done as f64 / secs) as u64
    }

    /// The next reading, back at the start of the line and padded out to
    /// whatever the last one occupied, so no tail of it shows past the end of
    /// this one.
    fn frame(&mut self) -> String {
        let line = self.line();
        let width = line.chars().count();
        let pad = self.width.saturating_sub(width);
        self.width = width;

        format!("\r{line}{blank:pad$}", blank = "")
    }

    fn draw(&mut self, last: bool) {
        if !self.live {
            return;
        }

        let frame = self.frame();
        let mut err = io::stderr().lock();

        let _ = err.write_all(frame.as_bytes());
        if last {
            let _ = err.write_all(b"\n");
        }
        let _ = err.flush();
    }
}

impl Drop for Progress {
    /// Whether the transfer finished or gave up partway, the last thing drawn
    /// should be where it got to, on a line of its own so that what comes next
    /// - a shell prompt, or the error that ended it - starts on a clean one.
    fn drop(&mut self) {
        self.draw(true);
    }
}

/// A writer that meters what passes through it on its way to another.
pub struct Meter<W: Write> {
    inner: W,
    progress: Progress,
}

impl<W: Write> Meter<W> {
    pub fn new(inner: W, progress: Progress) -> Self {
        Meter { inner, progress }
    }
}

impl<W: Write> Write for Meter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.progress.add(n as u64);

        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// Bytes as a size someone can take in at a glance.
fn human(bytes: u64) -> String {
    let mut size = bytes as f64;
    let mut unit = 0;

    while size >= 1024.0 && unit + 1 < UNITS.len() {
        size /= 1024.0;
        unit += 1;
    }

    match unit {
        0 => format!("{bytes} B"),
        _ => format!("{size:.1} {}", UNITS[unit]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_human() {
        assert_eq!(human(0), "0 B");
        assert_eq!(human(1023), "1023 B");
        assert_eq!(human(1024), "1.0 KiB");
        assert_eq!(human(1536), "1.5 KiB");
        assert_eq!(human(10 * 1024 * 1024), "10.0 MiB");
        assert_eq!(human(3 * 1024 * 1024 * 1024), "3.0 GiB");
        // Nothing scales past the last unit, however big.
        assert_eq!(human(2 * 1024 * 1024 * 1024 * 1024 * 1024), "2048.0 TiB");
    }

    #[test]
    fn test_line_with_a_total() {
        let mut progress = Progress::new("sending payload.bin", Some(4096));
        progress.add(1024);

        let line = progress.line();
        assert!(line.starts_with("sending payload.bin  1.0 KiB / 4.0 KiB   25%"));
    }

    #[test]
    fn test_line_without_a_total() {
        let mut progress = Progress::new("receiving tree", None);
        progress.add(2048);

        let line = progress.line();
        assert!(line.starts_with("receiving tree  2.0 KiB"), "{}", line);
        // Nothing to be a percentage of.
        assert!(!line.contains('%'), "{}", line);
    }

    /// A total is an estimate on both sides, so beating it must not read as a
    /// bug, and a total too small to take a percentage of must not divide by
    /// zero.
    #[test]
    fn test_percentage_is_bounded() {
        let mut progress = Progress::new("sending x", Some(100));
        progress.add(400);
        assert!(progress.line().contains("100%"));

        let mut tiny = Progress::new("sending x", Some(1));
        tiny.add(1);
        assert!(tiny.line().contains("100%"));
    }

    /// A shorter reading has to cover the longer one it replaces, or the tail
    /// of the old line is left showing past the end of the new one.
    #[test]
    fn test_a_frame_covers_the_one_before_it() {
        let mut progress = Progress::new("sending a-long-name.bin", None);

        let first = progress.frame();
        assert!(first.starts_with('\r'));
        assert!(!first.ends_with(' '));

        progress.label = "sending x".to_string();
        let second = progress.frame();

        assert_eq!(second.chars().count(), first.chars().count());
        assert!(second.ends_with(' '));
    }

    #[test]
    fn test_meter_counts_what_it_passes_on() {
        let mut out = Vec::new();
        let mut meter = Meter::new(&mut out, Progress::new("sending x", None));

        meter.write_all(b"hello").unwrap();
        meter.write_all(b" world").unwrap();
        assert_eq!(meter.progress.done, 11);

        drop(meter);
        assert_eq!(out, b"hello world");
    }
}
