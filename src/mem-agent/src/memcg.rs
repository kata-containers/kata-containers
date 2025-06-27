// Copyright (C) 2023 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use crate::cgroup::CGROUP_PATH;
use crate::mglru::{self, MGenLRU};
use crate::timer::Timeout;
use crate::{debug, error, info, trace, warn};
use crate::{proc, psi};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use nix::sched::sched_yield;
use page_size;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration as TokioDuration;

/* If last_inc_time to current_time small than IDLE_FRESH_IGNORE_SECS,
not do idle_fresh for this memcg.  */
const IDLE_FRESH_IGNORE_SECS: i64 = 60;

#[derive(Debug, Clone, Default)]
pub struct SingleOptionConfig {
    pub disabled: Option<bool>,
    pub swap: Option<bool>,
    pub swappiness_max: Option<u8>,
    pub period_secs: Option<u64>,
    pub period_psi_percent_limit: Option<u8>,
    pub eviction_psi_percent_limit: Option<u8>,
    pub eviction_run_aging_count_min: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct CgroupOptionConfig {
    pub path: String,
    pub numa_id: Vec<u32>,
    pub no_subdir: Option<bool>,
    pub config: SingleOptionConfig,
}

#[derive(Debug, Clone, Default)]
pub struct OptionConfig {
    pub del: Vec<(String, Vec<u32>)>,
    pub add: Vec<CgroupOptionConfig>,
    pub set: Vec<CgroupOptionConfig>,
    pub default: SingleOptionConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SingleConfig {
    pub disabled: bool,
    pub swap: bool,
    pub swappiness_max: u8,
    pub period_secs: u64,
    pub period_psi_percent_limit: u8,
    pub eviction_psi_percent_limit: u8,
    pub eviction_run_aging_count_min: u64,
}

impl Default for SingleConfig {
    fn default() -> Self {
        Self {
            disabled: false,
            swap: false,
            swappiness_max: 50,
            period_secs: 10 * 60,
            period_psi_percent_limit: 1,
            eviction_psi_percent_limit: 1,
            eviction_run_aging_count_min: 3,
        }
    }
}

impl SingleConfig {
    // return true if need reset
    fn set(&mut self, new_config: &SingleOptionConfig) -> bool {
        let mut need_reset = false;

        if let Some(p) = new_config.period_secs {
            if p != self.period_secs {
                self.period_secs = p;
                need_reset = true;
            }
        }

        if let Some(d) = new_config.disabled {
            if d != self.disabled {
                self.disabled = d;
                need_reset = true;
            }
        }
        if let Some(s) = new_config.swap {
            self.swap = s;
        }
        if let Some(s) = new_config.swappiness_max {
            self.swappiness_max = s;
        }
        if let Some(p) = new_config.period_psi_percent_limit {
            self.period_psi_percent_limit = p;
        }
        if let Some(p) = new_config.eviction_psi_percent_limit {
            self.eviction_psi_percent_limit = p;
        }
        if let Some(p) = new_config.eviction_run_aging_count_min {
            self.eviction_run_aging_count_min = p;
        }

        need_reset
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CgroupConfig {
    pub no_subdir: bool,
    pub numa_id: Vec<u32>,
    pub config: SingleConfig,
}

impl Default for CgroupConfig {
    fn default() -> Self {
        Self {
            no_subdir: false,
            // empty numa_id means this config not limit numa
            numa_id: vec![],
            config: SingleConfig::default(),
        }
    }
}

impl CgroupConfig {
    fn set(&mut self, config: &CgroupOptionConfig) -> bool {
        let mut need_reset = false;

        if let Some(no_subdir) = config.no_subdir {
            self.no_subdir = no_subdir;
            need_reset = true;
        }

        if self.config.set(&config.config) {
            need_reset = true;
        }

        need_reset
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub psi_path: PathBuf,
    pub default: SingleConfig,
    // path, numa_id_list, single_config
    pub cgroups: HashMap<String, Vec<CgroupConfig>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            psi_path: PathBuf::from(""),
            default: SingleConfig::default(),
            cgroups: HashMap::new(),
        }
    }
}

fn split_path_layers(input: &str) -> Vec<String> {
    let segments: Vec<&str> = input.split('/').filter(|s| !s.is_empty()).collect();
    let mut paths = Vec::with_capacity(segments.len());
    let mut current_path = String::new();

    for segment in segments {
        current_path.push('/');
        current_path.push_str(segment);
        paths.push(current_path.clone());
    }

    paths.reverse();
    paths.push('/'.to_string());
    paths
}

fn format_path(path: &str) -> String {
    let with_prefix = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{}", path)
    };

    let trimmed = with_prefix.trim_end_matches('/');

    if trimmed.is_empty() {
        "/".to_string()
    } else {
        trimmed.to_string()
    }
}

impl Config {
    fn format(&mut self) {
        let mut updates = Vec::new();
        for path in self.cgroups.keys().cloned().collect::<Vec<_>>() {
            let cur_path = format_path(&path);
            if path != cur_path {
                if let Some(value) = self.cgroups.remove(&cur_path) {
                    updates.push((cur_path, value));
                }
            }
        }
        for (path, value) in updates {
            self.cgroups.insert(path, value);
        }

        // make sure numa_id sorted
        for vec in self.cgroups.values_mut() {
            for c in vec.iter_mut() {
                c.numa_id.sort_unstable();
            }
        }

        // make sure the empty numa_id CgroupConfig at the end of Cgroup
        for vec in self.cgroups.values_mut() {
            let (keep, moved) = vec.drain(..).partition(|c| c.numa_id.len() > 0);
            *vec = keep;
            vec.extend(moved);
        }
    }

    fn path_to_numa_configs(
        &self,
        path: &str,
        mut numa_id: HashSet<u32>,
    ) -> Vec<(Vec<u32>, SingleConfig)> {
        let mut numa_configs = Vec::new();

        for curr_path in split_path_layers(path) {
            if let Some(ccs) = self.cgroups.get(&curr_path) {
                for cc in ccs {
                    // check subdir
                    if cc.no_subdir && curr_path != path {
                        // path is the subdir of path, but this config doesn;t allow subdir
                        continue;
                    }
                    // check numa
                    if cc.numa_id.is_empty() {
                        // All the remaining numa use this config.
                        numa_configs.push((numa_id.drain().collect::<Vec<_>>(), cc.config.clone()));
                    } else {
                        let mut ret_numa_id = Vec::new();
                        for cur_numa_id in cc.numa_id.clone() {
                            if numa_id.remove(&cur_numa_id) {
                                ret_numa_id.push(cur_numa_id);
                            }
                        }
                        if !ret_numa_id.is_empty() {
                            numa_configs.push((ret_numa_id, cc.config.clone()));
                        }
                    }
                }
            }
            if numa_id.is_empty() {
                // The configs of all numa_ids have been found.
                break;
            }
        }

        if numa_configs.is_empty() {
            numa_configs.push((numa_id.into_iter().collect(), self.default.clone()))
        }

        numa_configs
    }
}

#[derive(Debug, Clone)]
pub struct EvictionCount {
    pub page: u64,
    pub no_min_lru_file: u64,
    pub min_lru_inc: u64,
    pub other_error: u64,
    pub error: u64,
    pub psi_exceeds_limit: u64,
}

#[derive(Debug, Clone)]
pub struct Numa {
    pub max_seq: u64,
    pub min_seq: u64,
    pub last_inc_time: DateTime<Utc>,
    pub min_lru_file: u64,
    pub min_lru_anon: u64,

    pub run_aging_count: u64,
    pub eviction_count: EvictionCount,

    psi: psi::Period,
    pub sleep_psi_exceeds_limit: u64,
}

impl Numa {
    fn new(mglru: &MGenLRU, path: &str, psi_path: &PathBuf) -> Self {
        Self {
            max_seq: mglru.max_seq,
            min_seq: mglru.min_seq,
            last_inc_time: mglru.last_birth,
            min_lru_file: mglru.lru[mglru.min_lru_index].file,
            min_lru_anon: mglru.lru[mglru.min_lru_index].anon,
            run_aging_count: 0,
            eviction_count: EvictionCount {
                page: 0,
                no_min_lru_file: 0,
                min_lru_inc: 0,
                other_error: 0,
                error: 0,
                psi_exceeds_limit: 0,
            },
            psi: psi::Period::new(&psi_path.join(path.trim_start_matches('/')), false),
            sleep_psi_exceeds_limit: 0,
        }
    }

    fn update(&mut self, mglru: &MGenLRU) {
        self.max_seq = mglru.max_seq;
        self.min_seq = mglru.min_seq;
        self.last_inc_time = mglru.last_birth;
        self.min_lru_file = mglru.lru[mglru.min_lru_index].file;
        self.min_lru_anon = mglru.lru[mglru.min_lru_index].anon;
    }

    fn check_psi(&mut self, limit: u64) -> Result<bool> {
        let percent = self
            .psi
            .get_percent()
            .map_err(|e| anyhow!("psi.get_percent failed: {}", e))?;

        if percent > limit {
            info!("period psi {}% exceeds limit {}%", percent, limit);
            self.sleep_psi_exceeds_limit += 1;
            Ok(false)
        } else {
            Ok(true)
        }
    }
}

// Store the data of memcg.
// Doesn't include all numa becaue this data just has the numa that
// use same config.
#[derive(Debug, Clone)]
pub struct MemCgroup {
    pub numa: HashMap<u32, Numa>,

    /* get from Linux kernel static inline unsigned short mem_cgroup_id(struct mem_cgroup *memcg) */
    pub id: u16,
    pub ino: usize,
}

impl MemCgroup {
    fn new(
        id: &usize,
        ino: &usize,
        path: &str,
        numa: &Vec<u32>,
        hmg: &HashMap<usize, MGenLRU>,
        psi_path: &PathBuf,
    ) -> Self {
        let m = Self {
            id: *id as u16,
            ino: *ino,
            numa: numa
                .iter()
                .filter_map(|numa_id| {
                    if let Some(hmg) = hmg.get(&(*numa_id as usize)) {
                        Some((*numa_id, Numa::new(hmg, path, psi_path)))
                    } else {
                        None
                    }
                })
                .collect(),
        };

        debug!("MemCgroup::new {:?}", m);
        m
    }

    fn add_numa(
        &mut self,
        numa: &Vec<u32>,
        path: &str,
        psi_path: &PathBuf,
        hmg: &HashMap<usize, MGenLRU>,
    ) {
        for numa_id in numa {
            if let Some(hmg) = hmg.get(&(*numa_id as usize)) {
                self.numa.insert(*numa_id, Numa::new(hmg, path, psi_path));
            }
        }
    }

    fn update_from_hostmemcg(&mut self, hmg: &HashMap<usize, MGenLRU>) {
        for (numa_id, mglru) in hmg {
            self.numa
                .entry(*numa_id as u32)
                .and_modify(|e| e.update(mglru));
        }
    }
}

#[derive(Debug, Clone)]
enum EvictionStopReason {
    None,
    NoMinLru,
    MinLruInc,
    GetError,
    PsiExceedsLimit,
}

#[derive(Debug, Clone)]
struct EvictionInfo {
    psi: psi::Period,

    // the min_lru_file and min_lru_anon before last time mglru::run_eviction
    last_min_lru_file: u64,
    last_min_lru_anon: u64,

    // the evicted page count
    file_page_count: u64,
    anon_page_count: u64,

    only_swap_mode: bool,

    stop_reason: EvictionStopReason,
}

#[derive(Debug, Clone)]
struct Info {
    memcg_id: usize,
    numa_id: usize,
    path: String,
    max_seq: u64,
    min_seq: u64,
    last_inc_time: DateTime<Utc>,
    min_lru_file: u64,
    min_lru_anon: u64,

    eviction: Option<EvictionInfo>,
}

impl Info {
    fn new(path: &str, memcg_id: usize, numa_id: usize, numa: &Numa) -> Self {
        Self {
            memcg_id,
            numa_id: numa_id,
            path: path.to_string(),
            min_seq: numa.min_seq,
            max_seq: numa.max_seq,
            last_inc_time: numa.last_inc_time,
            min_lru_file: numa.min_lru_file,
            min_lru_anon: numa.min_lru_anon,
            eviction: None,
        }
    }

    fn update(&mut self, numa: &Numa) {
        self.min_seq = numa.min_seq;
        self.max_seq = numa.max_seq;
        self.last_inc_time = numa.last_inc_time;
        self.min_lru_file = numa.min_lru_file;
        self.min_lru_anon = numa.min_lru_anon;
    }
}

#[derive(Debug)]
struct NumaMap {
    id: u16,
    ino: usize,
    numa: Vec<u32>,
}

#[derive(Debug)]
struct PeriodSecsConfigMap {
    timeout: Timeout,

    // config->path->memcgroup->numa list
    cgs: HashMap<SingleConfig, HashMap<String, NumaMap>>,
}

// period_secs map
type ConfigMap = HashMap<u64, PeriodSecsConfigMap>;

#[derive(Debug)]
struct MemCgroups {
    is_cg_v2: bool,
    config: Config,

    // seconds->config->path->memcgroup->numa list
    // help to do timeout check
    config_map: ConfigMap,

    // path->memcgroup
    cgroups: HashMap<String, MemCgroup>,
}

impl MemCgroups {
    fn new(config: Config, is_cg_v2: bool) -> Self {
        Self {
            is_cg_v2,
            config,
            config_map: ConfigMap::new(),
            cgroups: HashMap::new(),
        }
    }

    fn remove_changed(
        &mut self,
        mg_hash: &HashMap<String, (usize, usize, HashMap<usize, MGenLRU>)>,
    ) {
        self.config_map.retain(|period, period_cgs| {
            period_cgs.cgs.retain(|conf, path_cgs| {
                path_cgs.retain(|path, numa| {
                    let mut should_keep = false;
                    if let Some((id, ino, _)) = mg_hash.get(path) {
                        if numa.id as usize == *id && numa.ino == *ino {
                            should_keep = true;
                        }
                    }
                    if !should_keep {
                        info!(
                            "Remove config_map {} {:?} {} {} {} because host changed.",
                            period, conf, path, numa.id, numa.ino
                        )
                    }
                    should_keep
                });
                path_cgs.len() != 0
            });
            period_cgs.cgs.len() != 0
        });

        self.cgroups.retain(|path, cgroup| {
            let mut should_keep = false;
            if let Some((id, ino, _)) = mg_hash.get(path) {
                if cgroup.id as usize == *id && cgroup.ino == *ino {
                    should_keep = true;
                }
            }
            if !should_keep {
                info!(
                    "Remove cgroups {} {} {} because host changed.",
                    path, cgroup.id, cgroup.ino
                )
            }
            should_keep
        });
    }

    fn update_and_add(
        &mut self,
        mg_hash: &HashMap<String, (usize, usize, HashMap<usize, MGenLRU>)>,
        update_cgroups: bool,
    ) {
        for (path, (id, ino, hmg)) in mg_hash {
            if *id == 0 {
                debug!(
                    "Not add {} {} {} because it is disabled.",
                    *id,
                    *ino,
                    path.to_string()
                )
            }

            let need_insert = if update_cgroups {
                if let Some(mg) = self.cgroups.get_mut(path) {
                    // Update current
                    mg.update_from_hostmemcg(&hmg);
                    false
                } else {
                    true
                }
            } else {
                true
            };

            if need_insert {
                // Create new and insert
                // Get the configs(numa may have different configs) for this memcg
                let numa_configs = self
                    .config
                    .path_to_numa_configs(path, hmg.keys().cloned().map(|k| k as u32).collect());

                for (numa_id, config) in &numa_configs {
                    loop {
                        if let Some(secs_config_map) = self.config_map.get_mut(&config.period_secs)
                        {
                            if let Some(config_map) = secs_config_map.cgs.get_mut(&config) {
                                if let Some(_) = config_map.get_mut(path) {
                                    error!(
                                        "update_and_add found an memcg {:?} {} existed",
                                        config, path
                                    );
                                } else {
                                    debug!(
                                        "update_and_add: add new config_map {:?} {} {} {}",
                                        config, path, *id, *ino
                                    );

                                    config_map.insert(
                                        path.clone(),
                                        NumaMap {
                                            id: *id as u16,
                                            ino: *ino,
                                            numa: numa_id.clone(),
                                        },
                                    );

                                    if update_cgroups {
                                        // update cgroups
                                        if let Some(cgroups) = self.cgroups.get_mut(path) {
                                            debug!(
                                                "update_and_add: add new cgroup {} {:?}",
                                                path, numa_id
                                            );

                                            cgroups.add_numa(
                                                &numa_id,
                                                path,
                                                &self.config.psi_path,
                                                hmg,
                                            );
                                        } else {
                                            debug!(
                                                "update_and_add: add new cgroup {} {:?}",
                                                path, numa_id
                                            );

                                            self.cgroups.insert(
                                                path.clone(),
                                                MemCgroup::new(
                                                    id,
                                                    ino,
                                                    path,
                                                    &numa_id,
                                                    hmg,
                                                    &self.config.psi_path,
                                                ),
                                            );
                                        }
                                    }
                                }
                                break;
                            } else {
                                secs_config_map.cgs.insert(config.clone(), HashMap::new());
                            }
                        } else {
                            self.config_map.insert(
                                config.period_secs,
                                PeriodSecsConfigMap {
                                    timeout: Timeout::new(config.period_secs),
                                    cgs: HashMap::new(),
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    fn check_psi_get_infos(&mut self, sec: u64) -> Vec<(SingleConfig, Vec<Info>)> {
        let mut infos_ret = Vec::new();

        if let Some(sec_config_map) = self.config_map.get(&sec) {
            for (single_config, path_map) in &sec_config_map.cgs {
                if single_config.disabled {
                    continue;
                }

                let mut info_ret = Vec::new();

                for (path, numa_map) in path_map {
                    if let Some(mcg) = self.cgroups.get_mut(path) {
                        for numa_id in &numa_map.numa {
                            if let Some(numa) = mcg.numa.get_mut(&numa_id) {
                                let pass = match numa
                                    .check_psi(single_config.period_psi_percent_limit as u64)
                                {
                                    Ok(p) => p,
                                    Err(e) => {
                                        warn!(
                                            "check_psi_get_infos: config {:?} cgroup {} numa_get_psi_percent failed: {}",
                                            single_config, path, e
                                        );
                                        continue;
                                    }
                                };

                                if !pass {
                                    info!("{} period psi exceeds limit", path);
                                    continue;
                                }

                                info_ret.push(Info::new(
                                    &path,
                                    mcg.id as usize,
                                    *numa_id as usize,
                                    numa,
                                ));
                            } else {
                                warn!(
                                    "check_psi_get_infos: config {:?} cgroup {} numa {} cannot get numa info",
                                    single_config, path, numa_id
                                );
                                continue;
                            }
                        }
                    }
                }

                if info_ret.len() > 0 {
                    infos_ret.push((single_config.clone(), info_ret));
                }
            }
        } else {
            debug!("check_psi_get_infos second {} is not exist", sec);
        }

        debug!("check_psi_get_infos second {} {:?}", sec, infos_ret);

        infos_ret
    }

    fn update_info(&self, infov: &mut Vec<Info>) {
        let mut i = 0;
        while i < infov.len() {
            if let Some(mg) = self.cgroups.get(&(infov[i].path)) {
                if let Some(numa) = mg.numa.get(&(infov[i].numa_id as u32)) {
                    infov[i].update(numa);
                    i += 1;
                    continue;
                }
            }

            infov.remove(i);
        }
    }

    fn inc_run_aging_count(&mut self, infov: &mut Vec<Info>) {
        let mut i = 0;
        while i < infov.len() {
            if let Some(mg) = self.cgroups.get_mut(&infov[i].path) {
                if let Some(numa) = mg.numa.get_mut(&(infov[i].numa_id as u32)) {
                    numa.run_aging_count += 1;
                    if numa.run_aging_count >= self.config.default.eviction_run_aging_count_min {
                        i += 1;
                        continue;
                    }
                }
            }

            infov.remove(i);
        }
    }

    fn record_eviction(&mut self, infov: &Vec<Info>) {
        for info in infov {
            if let Some(mg) = self.cgroups.get_mut(&(info.path)) {
                if let Some(numa) = mg.numa.get_mut(&(info.numa_id as u32)) {
                    if let Some(ei) = &info.eviction {
                        numa.eviction_count.page += ei.file_page_count + ei.anon_page_count;
                        match ei.stop_reason {
                            EvictionStopReason::None => numa.eviction_count.other_error += 1,
                            EvictionStopReason::NoMinLru => {
                                numa.eviction_count.no_min_lru_file += 1
                            }
                            EvictionStopReason::MinLruInc => numa.eviction_count.min_lru_inc += 1,
                            EvictionStopReason::GetError => numa.eviction_count.error += 1,
                            EvictionStopReason::PsiExceedsLimit => {
                                numa.eviction_count.psi_exceeds_limit += 1
                            }
                        }
                    }
                }
            }
        }
    }

    fn get_timeout_list(&self) -> Vec<u64> {
        let mut timeout_list = Vec::new();

        for (secs, secs_map) in &self.config_map {
            if secs_map.timeout.is_timeout() {
                timeout_list.push(*secs);
            }
        }

        timeout_list
    }

    fn reset_timers(&mut self, secs: &Vec<u64>) {
        for sec in secs {
            if let Some(secs_map) = self.config_map.get_mut(sec) {
                secs_map.timeout.reset();
            }
        }
    }

    pub fn get_remaining_tokio_duration(&self) -> TokioDuration {
        let mut ret = TokioDuration::MAX;

        for (_, secs_map) in &self.config_map {
            let cur = secs_map.timeout.remaining_tokio_duration();

            trace!(
                "get_remaining_tokio_duration: secs_map {:?} remaining_tokio_duration {:?}",
                secs_map,
                cur
            );

            if cur < ret {
                // check secs_map, make sure it has enabled config
                let mut has_enable_config = false;

                for (single_config, _) in &secs_map.cgs {
                    if !single_config.disabled {
                        has_enable_config = true;
                        break;
                    }
                }

                if has_enable_config {
                    ret = cur;
                }
            }
        }

        ret
    }

    // return if MemAgentSleep need be reset
    fn set_config(&mut self, config: OptionConfig) -> Result<bool> {
        // refresh
        let mg_hash = mglru::host_memcgs_get(&HashSet::new(), true, self.is_cg_v2)
            .map_err(|e| anyhow!("mglru::host_memcgs_get failed: {}", e))?;
        self.remove_changed(&mg_hash);
        self.update_and_add(&mg_hash, true);

        let mut need_reset = false;

        let orig_config = self.config.clone();

        // handle del
        for (path, numa) in config.del {
            let cur_path = format_path(&path);
            let should_del = if let Some(configs) = self.config.cgroups.get_mut(&cur_path) {
                configs.retain(|cfg| cfg.numa_id != numa);
                if configs.is_empty() {
                    true
                } else {
                    false
                }
            } else {
                false
            };

            if should_del {
                self.config.cgroups.remove(&cur_path);
                need_reset = true;
            }
        }

        // handle add
        for oc in config.add {
            loop {
                let cur_path = format_path(&oc.path);
                if let Some(numa_cgs) = self.config.cgroups.get_mut(&cur_path) {
                    let mut numa = oc.numa_id.clone();
                    numa.sort_unstable();
                    for cg in numa_cgs.clone() {
                        if cg.numa_id == numa {
                            self.config = orig_config;
                            return Err(anyhow!(
                                "path {} numa_id {:?} already exists",
                                cur_path,
                                numa
                            ));
                        }
                    }

                    let mut numa_cg = CgroupConfig::default();
                    numa_cg.numa_id = numa;
                    numa_cg.set(&oc);

                    numa_cgs.push(numa_cg);

                    need_reset = true;

                    break;
                } else {
                    self.config.cgroups.insert(cur_path, vec![]);
                }
            }
        }

        // handle set
        'outer: for oc in config.set {
            let cur_path = format_path(&oc.path);
            if let Some(numa_cgs) = self.config.cgroups.get_mut(&cur_path) {
                let mut numa = oc.numa_id.clone();
                numa.sort_unstable();
                for cg in numa_cgs {
                    if cg.numa_id == numa {
                        if cg.set(&oc) {
                            need_reset = true;
                        }
                        continue 'outer;
                    }
                }
            }
            self.config = orig_config;
            return Err(anyhow!(
                "path {} numa_id {:?} not exists",
                cur_path,
                oc.numa_id
            ));
        }

        if self.config.default.set(&config.default) {
            need_reset = true;
        }

        if need_reset {
            self.config.format();

            // remove old config_map
            self.config_map.clear();

            // add new config_map
            self.update_and_add(&mg_hash, false);
        }

        info!("new memcg config: {:#?}", self.config);
        trace!("new memcg config_map: {:#?}", self.config_map);
        if need_reset {
            info!("need reset mem-agent sleep");
        }

        Ok(need_reset)
    }
}

#[derive(Debug, Clone)]
pub struct MemCG {
    is_cg_v2: bool,
    memcgs: Arc<RwLock<MemCgroups>>,
}

fn div_round(a: u64, b: u64) -> u64 {
    let quotient = a / b;
    let remainder = a % b;
    if remainder >= b - remainder {
        quotient + 1
    } else {
        quotient
    }
}

impl MemCG {
    pub fn new(is_cg_v2: bool, mut config: Config) -> Result<Self> {
        mglru::check().map_err(|e| anyhow!("mglru::check failed: {}", e))?;

        if is_cg_v2 {
            config.psi_path = PathBuf::from(CGROUP_PATH);
        }

        config.psi_path =
            psi::check(&config.psi_path).map_err(|e| anyhow!("psi::check failed: {}", e))?;

        config.format();

        info!("memcg start with config: {:#?}", config);

        let mut memcg = Self {
            is_cg_v2,
            memcgs: Arc::new(RwLock::new(MemCgroups::new(config, is_cg_v2))),
        };

        /* Refresh memcgroups to self.memcgs.  */
        memcg
            .refresh(&HashSet::new())
            .map_err(|e| anyhow!("init refresh failed: {}", e))?;

        Ok(memcg)
    }

    pub fn work(&mut self, work_list: &Vec<u64>) -> Result<()> {
        /* Refresh memcgroups to self.memcgs.  */
        self.refresh(&HashSet::new())
            .map_err(|e| anyhow!("first refresh failed: {}", e))?;

        for sec in work_list {
            let sec = *sec;

            let mut infov = self.check_psi_get_infos(sec);

            self.run_aging(&mut infov);

            self.run_eviction(&mut infov)
                .map_err(|e| anyhow!("run_eviction second {} failed: {}", sec, e))?;
        }

        Ok(())
    }

    /*
     * If target_paths.len == 0,
     * will remove the updated or not exist cgroup in the host from MemCgroups.
     * If target_paths.len > 0, will not do that.
     */
    fn refresh(&mut self, target_paths: &HashSet<String>) -> Result<()> {
        let mg_hash = mglru::host_memcgs_get(target_paths, true, self.is_cg_v2)
            .map_err(|e| anyhow!("lru_gen_parse::file_parse failed: {}", e))?;

        let mut mgs = self.memcgs.blocking_write();

        if target_paths.len() == 0 {
            mgs.remove_changed(&mg_hash);
        }
        mgs.update_and_add(&mg_hash, true);

        Ok(())
    }

    fn run_aging(&mut self, config_infov: &mut Vec<(SingleConfig, Vec<Info>)>) {
        for (config, infov) in config_infov.iter_mut() {
            debug!("run_aging_single_config {:?}", config);
            self.run_aging_single_config(infov, config.swap);
        }
    }

    fn run_aging_single_config(&mut self, infov: &mut Vec<Info>, swap: bool) {
        infov.retain(|info| {
            let now = Utc::now();
            if now.signed_duration_since(info.last_inc_time).num_seconds() < IDLE_FRESH_IGNORE_SECS
            {
                info!(
                    "{} not run aging because last_inc_time {}",
                    info.path, info.last_inc_time,
                );
                false
            } else {
                let res = if let Err(e) =
                    mglru::run_aging(info.memcg_id, info.numa_id, info.max_seq, swap, true)
                {
                    error!(
                        "mglru::run_aging {} {} {} failed: {}",
                        info.path, info.memcg_id, info.numa_id, e
                    );
                    false
                } else {
                    true
                };

                if let Err(e) = sched_yield() {
                    error!("sched_yield failed: {:?}", e);
                }

                res
            }
        });

        self.memcgs.blocking_write().inc_run_aging_count(infov);
    }

    fn swap_not_available(&self) -> Result<bool> {
        let freeswap_kb = proc::get_freeswap_kb().context("proc::get_freeswap_kb")?;

        if freeswap_kb > (256 * page_size::get() as u64 / 1024) {
            Ok(false)
        } else {
            Ok(true)
        }
    }

    fn get_swappiness(&self, anon_count: u64, file_count: u64) -> u8 {
        assert!(
            anon_count != 0 && file_count != 0,
            "anon and file must be non-zero"
        );

        let c = div_round(200 * anon_count, anon_count + file_count);

        c as u8
    }

    fn run_eviction(&mut self, config_infov: &mut Vec<(SingleConfig, Vec<Info>)>) -> Result<()> {
        for (config, infov) in config_infov.iter_mut() {
            debug!("run_eviction_single_config {:?}", config);
            self.run_eviction_single_config(infov, &config)?;
        }

        Ok(())
    }

    fn run_eviction_single_config(
        &mut self,
        infov: &mut Vec<Info>,
        config: &SingleConfig,
    ) -> Result<()> {
        let mut swap = config.swap;

        if swap
            && self
                .swap_not_available()
                .context("self.swap_not_available")?
        {
            swap = false;
        }

        let psi_path = self.memcgs.blocking_read().config.psi_path.clone();
        for info in infov.into_iter() {
            info.eviction = Some(EvictionInfo {
                psi: psi::Period::new(&psi_path.join(info.path.trim_start_matches('/')), false),
                last_min_lru_file: 0,
                last_min_lru_anon: 0,
                file_page_count: 0,
                anon_page_count: 0,
                only_swap_mode: false,
                stop_reason: EvictionStopReason::None,
            });
        }

        let mut removed_infov = Vec::new();

        let mut ret = Ok(());

        'main_loop: while infov.len() != 0 {
            // update infov
            let path_set: HashSet<String> = infov.iter().map(|info| info.path.clone()).collect();
            match self.refresh(&path_set) {
                Ok(_) => {}
                Err(e) => {
                    ret = Err(anyhow!("refresh failed: {}", e));
                    break 'main_loop;
                }
            };
            self.update_info(infov);

            let mut i = 0;
            while i < infov.len() {
                let ci = infov[i].clone();

                trace!("{} {} run_eviction single loop start", ci.path, ci.numa_id);

                if let Some(ref mut ei) = infov[i].eviction {
                    if ci.max_seq - ci.min_seq + 1 != mglru::MAX_NR_GENS {
                        info!("{} {} run_eviction stop because max seq {} min seq {} not fit MAX_NR_GENS, release {} {} pages",
                                      ci.path, ci.numa_id, ci.max_seq, ci.min_seq, ei.anon_page_count, ei.file_page_count);
                        ei.stop_reason = EvictionStopReason::None;
                        removed_infov.push(infov.remove(i));
                        continue;
                    }

                    if ei.last_min_lru_file == 0 && ei.last_min_lru_anon == 0 {
                        // First loop
                        trace!("{} {} run_eviction begin", ci.path, ci.numa_id,);
                        if ci.min_lru_file == 0 {
                            if !swap || ci.min_lru_anon == 0 {
                                info!("{} {} run_eviction stop because min_lru_file is 0 or min_lru_anon is 0, release {} {} pages",
                                      ci.path, ci.numa_id, ei.anon_page_count, ei.file_page_count);
                                ei.stop_reason = EvictionStopReason::NoMinLru;
                                removed_infov.push(infov.remove(i));
                                continue;
                            } else {
                                ei.only_swap_mode = true;
                                trace!("{} {} run_eviction only swap mode", ci.path, ci.numa_id,);
                            }
                        }

                        ei.last_min_lru_file = ci.min_lru_file;
                        ei.last_min_lru_anon = ci.min_lru_anon;
                    } else {
                        if ci.min_lru_file >= ei.last_min_lru_file
                            && ci.min_lru_anon >= ei.last_min_lru_anon
                        {
                            info!(
                                "{} {} run_eviction stop because min_lru_file {} last_min_lru_file {} min_lru_anon {} last_min_lru_anon {}, release {} {} pages",
                                ci.path, ci.numa_id, ci.min_lru_file, ei.last_min_lru_file, ci.min_lru_anon, ei.last_min_lru_anon, ei.anon_page_count, ei.file_page_count,
                            );

                            ei.stop_reason = EvictionStopReason::MinLruInc;
                            removed_infov.push(infov.remove(i));
                            continue;
                        }

                        let released = ei.last_min_lru_anon - ci.min_lru_anon;
                        trace!(
                            "{} {} run_eviction anon {} pages",
                            ci.path,
                            ci.numa_id,
                            released
                        );
                        ei.anon_page_count += released;

                        let released = ei.last_min_lru_file - ci.min_lru_file;
                        trace!(
                            "{} {} run_eviction file {} pages",
                            ci.path,
                            ci.numa_id,
                            released
                        );
                        ei.file_page_count += released;

                        if !ei.only_swap_mode {
                            if ci.min_lru_file == 0 {
                                info!(
                                "{} {} run_eviction stop because min_lru_file is 0, release {} {} pages",
                                ci.path, ci.numa_id, ei.anon_page_count, ei.file_page_count,
                            );
                                ei.stop_reason = EvictionStopReason::NoMinLru;
                                removed_infov.push(infov.remove(i));
                                continue;
                            }
                        }

                        let percent = match ei.psi.get_percent() {
                            Ok(p) => p,
                            Err(e) => {
                                debug!(
                                    "{} {} ei.psi.get_percent failed: {}, release {} {} pages",
                                    ci.path, ci.numa_id, e, ei.anon_page_count, ei.file_page_count,
                                );
                                ei.stop_reason = EvictionStopReason::GetError;
                                removed_infov.push(infov.remove(i));
                                continue;
                            }
                        };
                        if percent > config.eviction_psi_percent_limit as u64 {
                            info!(
                                "{} {} run_eviction stop because period psi {}% exceeds limit, release {} {} pages",
                                ci.path, ci.numa_id, percent, ei.anon_page_count, ei.file_page_count,
                            );
                            ei.stop_reason = EvictionStopReason::PsiExceedsLimit;
                            removed_infov.push(infov.remove(i));
                            continue;
                        }

                        ei.last_min_lru_file = ci.min_lru_file;
                        ei.last_min_lru_anon = ci.min_lru_anon;
                    }

                    let swap_not_available = match self.swap_not_available() {
                        Ok(b) => b,
                        Err(e) => {
                            ret = Err(anyhow!("swap_not_available failed: {:?}", e));
                            break 'main_loop;
                        }
                    };

                    // get swapiness
                    let swappiness = if ei.only_swap_mode {
                        if swap_not_available {
                            info!(
                                "{} {} run_eviction stop because only_swap_mode and swap_not_available, release {} {} pages",
                                ci.path, ci.numa_id, ei.anon_page_count, ei.file_page_count,
                            );
                            ei.stop_reason = EvictionStopReason::NoMinLru;
                            removed_infov.push(infov.remove(i));
                            continue;
                        }
                        200
                    } else if !swap || ci.min_lru_anon == 0 || swap_not_available {
                        0
                    } else {
                        let s = self.get_swappiness(ci.min_lru_anon, ci.min_lru_file);
                        if s > config.swappiness_max {
                            config.swappiness_max
                        } else {
                            s
                        }
                    };

                    trace!(
                        "{} {} run_eviction min_seq {} swappiness {}",
                        ci.path,
                        ci.numa_id,
                        ci.min_seq,
                        swappiness
                    );

                    match mglru::run_eviction(ci.memcg_id, ci.numa_id, ci.min_seq, swappiness, 1) {
                        Ok(_) => {}
                        Err(e) => {
                            error!(
                                "{} {} mglru::run_eviction failed: {}, release {} {} pages",
                                ci.path, ci.numa_id, e, ei.anon_page_count, ei.file_page_count,
                            );
                            ei.stop_reason = EvictionStopReason::GetError;
                            removed_infov.push(infov.remove(i));
                            continue;
                        }
                    }

                    if let Err(e) = sched_yield() {
                        error!("sched_yield failed: {:?}", e);
                    }
                } else {
                    unreachable!();
                }

                i += 1;
            }
        }

        let mut mgs = self.memcgs.blocking_write();
        mgs.record_eviction(&infov);
        mgs.record_eviction(&removed_infov);

        ret
    }

    fn check_psi_get_infos(&mut self, sec: u64) -> Vec<(SingleConfig, Vec<Info>)> {
        self.memcgs.blocking_write().check_psi_get_infos(sec)
    }

    fn update_info(&self, infov: &mut Vec<Info>) {
        self.memcgs.blocking_read().update_info(infov);
    }

    pub fn get_timeout_list(&self) -> Vec<u64> {
        self.memcgs.blocking_read().get_timeout_list()
    }

    pub fn reset_timers(&mut self, work_list: &Vec<u64>) {
        self.memcgs.blocking_write().reset_timers(work_list);
    }

    pub fn get_remaining_tokio_duration(&self) -> TokioDuration {
        self.memcgs.blocking_read().get_remaining_tokio_duration()
    }

    pub async fn async_get_remaining_tokio_duration(&self) -> TokioDuration {
        self.memcgs.read().await.get_remaining_tokio_duration()
    }

    pub async fn set_config(&mut self, new_config: OptionConfig) -> Result<bool> {
        self.memcgs.write().await.set_config(new_config)
    }

    pub async fn get_status(&self) -> HashMap<String, MemCgroup> {
        self.memcgs.read().await.cgroups.clone()
    }
}

mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_memcg_swap_not_available() {
        let is_cg_v2 = crate::cgroup::is_cgroup_v2().unwrap();
        let m = MemCG::new(is_cg_v2, Config::default()).unwrap();
        assert!(m.swap_not_available().is_ok());
    }

    #[test]
    fn test_memcg_get_swappiness() {
        let is_cg_v2 = crate::cgroup::is_cgroup_v2().unwrap();
        let m = MemCG::new(is_cg_v2, Config::default()).unwrap();
        assert_eq!(m.get_swappiness(100, 50), 133);
    }

    #[test]
    fn test_memcg_get_timeout_list() {
        let is_cg_v2 = crate::cgroup::is_cgroup_v2().unwrap();
        let m = MemCG::new(is_cg_v2, Config::default()).unwrap();
        assert_eq!(m.get_timeout_list().len() > 0, true);
    }
}
