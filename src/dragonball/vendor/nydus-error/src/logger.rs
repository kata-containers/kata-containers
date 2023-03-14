// Copyright 2020 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::SystemTime;

use serde::Serialize;
use serde_json::Error as SerdeError;

/// Error codes for `ErrorHolder`.
#[derive(Debug)]
pub enum ErrorHolderError {
    TooLarge(usize),
    Serde(SerdeError),
}

/// `Result` specialized for `ErrorHolder`.
pub type Result<T> = std::result::Result<T, ErrorHolderError>;

/// Struct to record important or critical events or errors in circular buffer mode.
#[derive(Serialize, Default, Debug)]
pub struct ErrorHolder {
    max_errors: usize,
    total_errors: usize,
    max_size: usize,
    total_size: usize,
    errors: Mutex<VecDeque<String>>,
}

impl ErrorHolder {
    /// Create a `ErrorHolder` object.
    pub fn new(max_errors: usize, max_size: usize) -> Self {
        Self {
            max_errors,
            max_size,
            total_errors: 0,
            total_size: 0,
            errors: Mutex::new(VecDeque::with_capacity(max_errors)),
        }
    }

    /// Push an error into the circular buffer.
    pub fn push(&mut self, error: &str) -> Result<()> {
        let mut guard = self.errors.lock().unwrap();
        let formatted_error = format!("{} - {}", httpdate::fmt_http_date(SystemTime::now()), error);

        loop {
            if formatted_error.len() + self.total_size > self.max_size
                || self.total_errors >= self.max_errors
            {
                let victim = guard.pop_front();
                match victim {
                    Some(v) => {
                        self.total_size -= v.len();
                        self.total_errors -= 1;
                    }
                    None => return Err(ErrorHolderError::TooLarge(error.len())),
                }
            } else {
                break;
            }
        }

        self.total_size += formatted_error.len();
        self.total_errors += 1;
        guard.push_back(formatted_error);
        Ok(())
    }

    /// Export all errors in the circular buffer as an `JSON` string.
    pub fn export(&self) -> Result<String> {
        let _guard = self.errors.lock().unwrap();
        serde_json::to_string(self).map_err(ErrorHolderError::Serde)
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorHolder, ErrorHolderError};

    #[test]
    fn test_overflow() {
        let mut holder = ErrorHolder::new(10, 80);
        let error_msg = "123456789";
        let mut left = 16;
        while left >= 0 {
            let r = holder.push(error_msg);
            assert!(r.is_ok());
            left -= 1;
        }

        assert!(holder.total_errors <= 10);
        assert!(holder.total_size <= 80);

        let mut multi = 10;
        let mut error_msg_long = "".to_string();
        while multi >= 0 {
            multi -= 1;
            error_msg_long.push_str("123456789");
        }

        let r = holder.push(&error_msg_long);
        match r {
            Err(ErrorHolderError::TooLarge(len)) => assert_eq!(len, error_msg_long.len()),
            _ => panic!(),
        }
    }
}
