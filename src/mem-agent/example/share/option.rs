// Copyright (C) 2024 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use crate::protocols::mem_agent as rpc;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct MemcgSetOption {
    #[structopt(long)]
    memcg_disabled: Option<bool>,
    #[structopt(long)]
    memcg_swap: Option<bool>,
    #[structopt(long)]
    memcg_swappiness_max: Option<u8>,
    #[structopt(long)]
    memcg_period_secs: Option<u64>,
    #[structopt(long)]
    memcg_period_psi_percent_limit: Option<u8>,
    #[structopt(long)]
    memcg_eviction_psi_percent_limit: Option<u8>,
    #[structopt(long)]
    memcg_eviction_run_aging_count_min: Option<u64>,
}

impl MemcgSetOption {
    #[allow(dead_code)]
    pub fn to_rpc_memcg_config(&self) -> rpc::MemcgConfig {
        let config = rpc::MemcgConfig {
            disabled: self.memcg_disabled,
            swap: self.memcg_swap,
            swappiness_max: self.memcg_swappiness_max.map(|v| v as u32),
            period_secs: self.memcg_period_secs,
            period_psi_percent_limit: self.memcg_period_psi_percent_limit.map(|v| v as u32),
            eviction_psi_percent_limit: self.memcg_eviction_psi_percent_limit.map(|v| v as u32),
            eviction_run_aging_count_min: self.memcg_eviction_run_aging_count_min,
            ..Default::default()
        };

        config
    }

    #[allow(dead_code)]
    pub fn to_mem_agent_memcg_config(&self) -> mem_agent::memcg::Config {
        let mut config = mem_agent::memcg::Config {
            ..Default::default()
        };

        if let Some(v) = self.memcg_disabled {
            config.disabled = v;
        }
        if let Some(v) = self.memcg_swap {
            config.swap = v;
        }
        if let Some(v) = self.memcg_swappiness_max {
            config.swappiness_max = v;
        }
        if let Some(v) = self.memcg_period_secs {
            config.period_secs = v;
        }
        if let Some(v) = self.memcg_period_psi_percent_limit {
            config.period_psi_percent_limit = v;
        }
        if let Some(v) = self.memcg_eviction_psi_percent_limit {
            config.eviction_psi_percent_limit = v;
        }
        if let Some(v) = self.memcg_eviction_run_aging_count_min {
            config.eviction_run_aging_count_min = v;
        }

        config
    }
}

#[derive(Debug, StructOpt)]
pub struct CompactSetOption {
    #[structopt(long)]
    compact_disabled: Option<bool>,
    #[structopt(long)]
    compact_period_secs: Option<u64>,
    #[structopt(long)]
    compact_period_psi_percent_limit: Option<u8>,
    #[structopt(long)]
    compact_psi_percent_limit: Option<u8>,
    #[structopt(long)]
    compact_sec_max: Option<i64>,
    #[structopt(long)]
    compact_order: Option<u8>,
    #[structopt(long)]
    compact_threshold: Option<u64>,
    #[structopt(long)]
    compact_force_times: Option<u64>,
}

impl CompactSetOption {
    #[allow(dead_code)]
    pub fn to_rpc_compact_config(&self) -> rpc::CompactConfig {
        let config = rpc::CompactConfig {
            disabled: self.compact_disabled,
            period_secs: self.compact_period_secs,
            period_psi_percent_limit: self.compact_period_psi_percent_limit.map(|v| v as u32),
            compact_psi_percent_limit: self.compact_psi_percent_limit.map(|v| v as u32),
            compact_sec_max: self.compact_sec_max,
            compact_order: self.compact_order.map(|v| v as u32),
            compact_threshold: self.compact_threshold,
            compact_force_times: self.compact_force_times,
            ..Default::default()
        };

        config
    }

    #[allow(dead_code)]
    pub fn to_mem_agent_compact_config(&self) -> mem_agent::compact::Config {
        let mut config = mem_agent::compact::Config {
            ..Default::default()
        };

        if let Some(v) = self.compact_disabled {
            config.disabled = v;
        }
        if let Some(v) = self.compact_period_secs {
            config.period_secs = v;
        }
        if let Some(v) = self.compact_period_psi_percent_limit {
            config.period_psi_percent_limit = v;
        }
        if let Some(v) = self.compact_psi_percent_limit {
            config.compact_psi_percent_limit = v;
        }
        if let Some(v) = self.compact_sec_max {
            config.compact_sec_max = v;
        }
        if let Some(v) = self.compact_order {
            config.compact_order = v;
        }
        if let Some(v) = self.compact_threshold {
            config.compact_threshold = v;
        }
        if let Some(v) = self.compact_force_times {
            config.compact_force_times = v;
        }

        config
    }
}
