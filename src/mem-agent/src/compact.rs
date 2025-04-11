// Copyright (C) 2024 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use crate::proc;
use crate::psi;
use crate::timer::Timeout;
use crate::{debug, error, info, trace};
use anyhow::{anyhow, Result};
use nix::sched::sched_yield;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::Duration as TokioDuration;

const PAGE_REPORTING_MIN_ORDER: u8 = 9;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub disabled: bool,
    pub psi_path: PathBuf,
    pub period_secs: u64,
    pub period_psi_percent_limit: u8,
    pub compact_psi_percent_limit: u8,
    pub compact_sec_max: i64,

    // the order that want to get from compaction
    pub compact_order: u8,

    // compact_threshold is the pages number.
    // When examining the /proc/pagetypeinfo, if there's an increase in the
    // number of movable pages of orders smaller than the compact_order
    // compared to the amount following the previous compaction,
    // and this increase surpasses a certain threshold—specifically,
    // more than 'compact_threshold' number of pages.
    // Or the number of free pages has decreased by 'compact_threshold'
    // since the previous compaction.
    // then the system should initiate another round of memory compaction.
    pub compact_threshold: u64,

    // After one compaction, if there has not been a compaction within
    // the next compact_force_times times, a compaction will be forced
    // regardless of the system's memory situation.
    // If compact_force_times is set to 0, will do force compaction each time.
    // If compact_force_times is set to std::u64::MAX, will never do force compaction.
    pub compact_force_times: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            disabled: false,
            psi_path: PathBuf::from(""),
            period_secs: 10 * 60,
            period_psi_percent_limit: 1,
            compact_psi_percent_limit: 5,
            compact_sec_max: 30 * 60,
            compact_order: PAGE_REPORTING_MIN_ORDER,
            compact_threshold: 2 << PAGE_REPORTING_MIN_ORDER,
            compact_force_times: std::u64::MAX,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OptionConfig {
    pub disabled: Option<bool>,
    pub psi_path: Option<PathBuf>,
    pub period_secs: Option<u64>,
    pub period_psi_percent_limit: Option<u8>,
    pub compact_psi_percent_limit: Option<u8>,
    pub compact_sec_max: Option<i64>,

    pub compact_order: Option<u8>,

    pub compact_threshold: Option<u64>,

    pub compact_force_times: Option<u64>,
}

#[derive(Debug, Clone)]
struct CompactCore {
    timeout: Timeout,
    config: Config,
    psi: psi::Period,
    force_counter: u64,
    prev_free_movable_pages_after_compact: u64,
    prev_memfree_kb: u64,
}

impl CompactCore {
    fn new(config: Config) -> Self {
        Self {
            timeout: Timeout::new(config.period_secs),
            psi: psi::Period::new(&config.psi_path, true),
            force_counter: 0,
            prev_free_movable_pages_after_compact: 0,
            prev_memfree_kb: 0,
            config,
        }
    }

    fn psi_ok(&mut self) -> bool {
        if crate::misc::is_test_environment() {
            return false;
        }
        let percent = match self.psi.get_percent() {
            Ok(v) => v,
            Err(e) => {
                debug!("psi.get_percent failed: {}", e);
                return false;
            }
        };
        if percent > self.config.period_psi_percent_limit as u64 {
            info!(
                "compact will not work because period psi {}% exceeds limit",
                percent
            );
            false
        } else {
            true
        }
    }

    fn need_force_compact(&self) -> bool {
        if self.config.compact_force_times == std::u64::MAX {
            return false;
        }

        self.force_counter >= self.config.compact_force_times
    }

    fn check_compact_threshold(&self, memfree_kb: u64, free_movable_pages: u64) -> bool {
        if self.prev_memfree_kb > memfree_kb + (self.config.compact_threshold << 2) {
            return true;
        }

        let threshold = self.config.compact_threshold + self.prev_free_movable_pages_after_compact;
        if free_movable_pages > threshold {
            true
        } else {
            info!(
                "compact will not work because free movable pages {} less than threshold {} and prev_free {}kB current_free {}kB",
                free_movable_pages, threshold, self.prev_memfree_kb, memfree_kb
            );
            false
        }
    }

    fn get_special_psi(&self) -> psi::Period {
        psi::Period::new(&self.config.psi_path, true)
    }

    fn set_prev(&mut self, memfree_kb: u64, free_movable_pages: u64) {
        self.prev_memfree_kb = memfree_kb;
        self.prev_free_movable_pages_after_compact = free_movable_pages;
    }

    fn set_disabled(&mut self, disabled: bool) {
        if !disabled {
            self.timeout.reset();
        }

        self.config.disabled = disabled;
    }

    // return if MemAgentSleep need be reset
    fn set_config(&mut self, new_config: OptionConfig) -> bool {
        let mut need_reset_mas = false;

        if let Some(d) = new_config.disabled {
            if self.config.disabled != d {
                self.set_disabled(d);
                need_reset_mas = true;
            }
        }
        if let Some(p) = new_config.psi_path {
            self.config.psi_path = p.clone();
        }
        if let Some(p) = new_config.period_psi_percent_limit {
            self.config.period_psi_percent_limit = p;
        }
        if let Some(p) = new_config.compact_psi_percent_limit {
            self.config.compact_psi_percent_limit = p;
        }
        if let Some(p) = new_config.compact_sec_max {
            self.config.compact_sec_max = p;
        }
        if let Some(p) = new_config.compact_order {
            self.config.compact_order = p;
        }
        if let Some(p) = new_config.compact_threshold {
            self.config.compact_threshold = p;
        }
        if let Some(p) = new_config.compact_force_times {
            self.config.compact_force_times = p;
        }
        if let Some(p) = new_config.period_secs {
            self.config.period_secs = p;
            self.timeout.set_sleep_duration(p);
            if !self.config.disabled {
                need_reset_mas = true;
            }
        }

        info!("new compact config: {:#?}", self.config);
        if need_reset_mas {
            info!("need reset mem-agent sleep");
        }

        need_reset_mas
    }

    fn need_work(&self) -> bool {
        if self.config.disabled {
            return false;
        }
        self.timeout.is_timeout()
    }

    pub fn get_remaining_tokio_duration(&self) -> TokioDuration {
        if self.config.disabled {
            return TokioDuration::MAX;
        }

        self.timeout.remaining_tokio_duration()
    }
}

#[derive(Debug, Clone)]
pub struct Compact {
    core: Arc<RwLock<CompactCore>>,
}

impl Compact {
    pub fn new(mut config: Config) -> Result<Self> {
        config.psi_path =
            psi::check(&config.psi_path).map_err(|e| anyhow!("psi::check failed: {}", e))?;

        let c = Self {
            core: Arc::new(RwLock::new(CompactCore::new(config))),
        };

        Ok(c)
    }

    fn calculate_free_movable_pages(&self) -> Result<u64> {
        let file = File::open("/proc/pagetypeinfo")?;
        let reader = BufReader::new(file);

        let order_limit = self.core.blocking_read().config.compact_order as usize;

        let mut total_free_movable_pages = 0;

        for line in reader.lines() {
            let line = line?;
            if line.contains("Movable") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(index) = parts.iter().position(|&element| element == "Movable") {
                    for (order, &count_str) in parts[(index + 1)..].iter().enumerate() {
                        if order < order_limit {
                            if let Ok(count) = count_str.parse::<u64>() {
                                total_free_movable_pages += count * 1 << order;
                            }
                        }
                    }
                }
            }
        }

        Ok(total_free_movable_pages)
    }

    fn check_compact_threshold(&self) -> bool {
        let memfree_kb = match proc::get_memfree_kb() {
            Ok(v) => v,
            Err(e) => {
                error!("get_memfree_kb failed: {}", e);
                return false;
            }
        };
        let free_movable_pages = match self.calculate_free_movable_pages() {
            Ok(v) => v,
            Err(e) => {
                error!("calculate_free_movable_pages failed: {}", e);
                return false;
            }
        };

        self.core
            .blocking_read()
            .check_compact_threshold(memfree_kb, free_movable_pages)
    }

    fn set_prev(&mut self) -> Result<()> {
        let memfree_kb =
            proc::get_memfree_kb().map_err(|e| anyhow!("get_memfree_kb failed: {}", e))?;
        let free_movable_pages = self
            .calculate_free_movable_pages()
            .map_err(|e| anyhow!("calculate_free_movable_pages failed: {}", e))?;

        self.core
            .blocking_write()
            .set_prev(memfree_kb, free_movable_pages);

        Ok(())
    }

    fn do_compact(&self) -> Result<()> {
        let compact_psi_percent_limit = self.core.blocking_read().config.compact_psi_percent_limit;
        let mut compact_psi = self.core.blocking_read().get_special_psi();
        let mut rest_sec = self.core.blocking_read().config.compact_sec_max;

        if let Err(e) = sched_yield() {
            error!("sched_yield failed: {:?}", e);
        }

        info!("compact start");

        let mut child = Command::new("sh")
            .arg("-c")
            .arg("echo 1 > /proc/sys/vm/compact_memory")
            .spawn()
            .map_err(|e| anyhow!("Command::new failed: {}", e))?;

        debug!("compact pid {}", child.id());

        let mut killed = false;
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    debug!("compact done with status {}", status);
                    break;
                }
                Ok(None) => {
                    if killed {
                        if rest_sec <= 0 {
                            error!("compact killed but not quit");
                            break;
                        } else {
                            debug!("compact killed and keep wait");
                        }
                    } else {
                        if rest_sec <= 0 {
                            debug!("compact timeout");
                            child
                                .kill()
                                .map_err(|e| anyhow!("child.kill failed: {}", e))?;
                            killed = true;
                        }
                    }

                    let percent = compact_psi
                        .get_percent()
                        .map_err(|e| anyhow!("compact_psi.get_percent failed: {}", e))?;
                    if percent > compact_psi_percent_limit as u64 {
                        info!(
                            "compaction need stop because period psi {}% exceeds limit",
                            percent
                        );
                        child
                            .kill()
                            .map_err(|e| anyhow!("child.kill failed: {}", e))?;
                        killed = true;
                    }
                }
                Err(e) => {
                    // try_wait will fail with code 10 because some task will
                    // wait compact task before try_wait.
                    debug!("compact try_wait fail: {:?}", e);
                    break;
                }
            }

            thread::sleep(Duration::from_secs(1));
            rest_sec -= 1;
        }

        info!("compact stop");

        Ok(())
    }

    pub fn need_work(&self) -> bool {
        self.core.blocking_read().need_work()
    }

    pub fn reset_timer(&mut self) {
        self.core.blocking_write().timeout.reset();
    }

    pub fn get_remaining_tokio_duration(&self) -> TokioDuration {
        self.core.blocking_read().get_remaining_tokio_duration()
    }

    pub async fn async_get_remaining_tokio_duration(&self) -> TokioDuration {
        self.core.read().await.get_remaining_tokio_duration()
    }

    pub fn work(&mut self) -> Result<()> {
        let mut can_work = self.core.blocking_write().psi_ok();
        if can_work {
            if !self.core.blocking_read().need_force_compact() {
                if !self.check_compact_threshold() {
                    trace!("not enough free movable pages");
                    can_work = false;
                }
            } else {
                trace!("force compact");
            }
        }

        if can_work {
            self.do_compact()
                .map_err(|e| anyhow!("do_compact failed: {}", e))?;

            self.set_prev()?;

            self.core.blocking_write().force_counter = 0;
        } else {
            self.core.blocking_write().force_counter += 1;
        }

        Ok(())
    }

    pub async fn set_config(&mut self, new_config: OptionConfig) -> bool {
        self.core.write().await.set_config(new_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact() {
        let mut c = Compact::new(Config::default()).unwrap();
        assert!(c.work().is_ok());
    }
}
