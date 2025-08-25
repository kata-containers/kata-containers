// Copyright (C) 2024 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use chrono::Duration as ChronoDuration;
use chrono::{DateTime, Utc};
use tokio::time::Duration as TokioDuration;

fn chrono_to_tokio_duration(chrono_duration: ChronoDuration) -> TokioDuration {
    if chrono_duration.num_nanoseconds().unwrap_or(0) >= 0 {
        TokioDuration::new(
            chrono_duration.num_seconds() as u64,
            (chrono_duration.num_nanoseconds().unwrap_or(0) % 1_000_000_000) as u32,
        )
    } else {
        TokioDuration::new(0, 0)
    }
}

#[derive(Debug, Clone)]
pub struct Timeout {
    sleep_duration: ChronoDuration,
    start_wait_time: DateTime<Utc>,
}

impl Timeout {
    pub fn new(secs: u64) -> Self {
        Self {
            sleep_duration: ChronoDuration::microseconds(secs as i64 * 1000000),
            /* Make sure the first time to timeout */
            start_wait_time: Utc::now() - ChronoDuration::microseconds(secs as i64 * 1000000) * 2,
        }
    }

    pub fn is_timeout(&self) -> bool {
        let now = Utc::now();
        now >= self.start_wait_time + self.sleep_duration
    }

    pub fn reset(&mut self) {
        self.start_wait_time = Utc::now();
    }

    pub fn remaining_tokio_duration(&self) -> TokioDuration {
        let now = Utc::now();

        if now >= self.start_wait_time + self.sleep_duration {
            return TokioDuration::ZERO;
        }

        chrono_to_tokio_duration(self.start_wait_time + self.sleep_duration - now)
    }

    pub fn set_sleep_duration(&mut self, secs: u64) {
        self.sleep_duration = ChronoDuration::microseconds(secs as i64 * 1000000);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_timeout() {
        let mut timeout = Timeout::new(1);

        // timeout should be timeout at once.
        assert_eq!(timeout.is_timeout(), true);

        timeout.reset();

        assert_eq!(timeout.is_timeout(), false);
        thread::sleep(Duration::from_secs(2));
        assert_eq!(timeout.is_timeout(), true);

        timeout.set_sleep_duration(2);
        timeout.reset();

        assert_eq!(timeout.is_timeout(), false);
        thread::sleep(Duration::from_secs(1));
        assert_eq!(timeout.is_timeout(), false);

        thread::sleep(Duration::from_secs(1));
        assert_eq!(timeout.is_timeout(), true);
    }
}
