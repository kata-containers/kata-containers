// Copyright (c) 2020 Alibaba Cloud
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::{Result, Write};

use slog::{info, Logger};

/// Writer to convert each line written to it to a log record.
#[derive(Debug)]
pub struct LogWriter(Logger);

impl LogWriter {
    /// Create a new isntance of ['LogWriter'].
    pub fn new(logger: Logger) -> Self {
        LogWriter(logger)
    }
}

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        buf.split(|b| *b == b'\n').for_each(|it| {
            if !it.is_empty() {
                info!(self.0, "{}", String::from_utf8_lossy(it))
            }
        });

        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{create_logger, FileRotator};
    use std::fs;

    #[test]
    fn test_log_writer() {
        let tmpdir = tempfile::tempdir().unwrap();
        let mut path = tmpdir.path().to_path_buf();
        path.push("log.txt");

        let mut rotator = FileRotator::new(&path).unwrap();
        rotator.truncate_mode(false);
        rotator.rotate_threshold(4);
        rotator.rotate_count(1);

        let (logger, guard) = create_logger("test", "hi", slog::Level::Info, rotator);
        let mut writer = LogWriter::new(logger);

        writer.write_all("test1\nblabla".as_bytes()).unwrap();
        writer.flush().unwrap();
        writer.write_all("test2".as_bytes()).unwrap();
        writer.flush().unwrap();
        drop(guard);

        let content = fs::read_to_string(path).unwrap();
        assert!(!content.is_empty());
    }
}
