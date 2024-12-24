// Copyright (C) 2023 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use crate::mglru::{self, MGenLRU};
use crate::timer::Timeout;
use crate::{debug, error, info, trace};
use crate::{proc, psi};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Utc};
use nix::sched::sched_yield;
use page_size;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration as TokioDuration;

/* If last_inc_time to current_time small than IDLE_FRESH_IGNORE_SECS,
not do idle_fresh for this memcg.  */
const IDLE_FRESH_IGNORE_SECS: i64 = 60;

#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub disabled: bool,
    pub swap: bool,
    pub swappiness_max: u8,
    pub psi_path: PathBuf,
    pub period_secs: u64,
    pub period_psi_percent_limit: u8,
    pub eviction_psi_percent_limit: u8,
    pub eviction_run_aging_count_min: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            disabled: false,
            swap: false,
            swappiness_max: 50,
            psi_path: PathBuf::from(""),
            period_secs: 10 * 60,
            period_psi_percent_limit: 1,
            eviction_psi_percent_limit: 1,
            eviction_run_aging_count_min: 3,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OptionConfig {
    pub disabled: Option<bool>,
    pub swap: Option<bool>,
    pub swappiness_max: Option<u8>,
    pub psi_path: Option<PathBuf>,
    pub period_secs: Option<u64>,
    pub period_psi_percent_limit: Option<u8>,
    pub eviction_psi_percent_limit: Option<u8>,
    pub eviction_run_aging_count_min: Option<u64>,
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
}

impl Numa {
    fn new(mglru: &MGenLRU) -> Self {
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
        }
    }

    fn update(&mut self, mglru: &MGenLRU) {
        self.max_seq = mglru.max_seq;
        self.min_seq = mglru.min_seq;
        self.last_inc_time = mglru.last_birth;
        self.min_lru_file = mglru.lru[mglru.min_lru_index].file;
        self.min_lru_anon = mglru.lru[mglru.min_lru_index].anon;
    }
}

#[derive(Debug, Clone)]
pub struct MemCgroup {
    /* get from Linux kernel static inline unsigned short mem_cgroup_id(struct mem_cgroup *memcg) */
    pub id: u16,
    pub ino: usize,
    pub path: String,
    pub numa: HashMap<u32, Numa>,
    psi: psi::Period,

    pub sleep_psi_exceeds_limit: u64,
}

impl MemCgroup {
    fn new(
        id: &usize,
        ino: &usize,
        path: &String,
        hmg: &HashMap<usize, MGenLRU>,
        psi_path: &PathBuf,
    ) -> Self {
        let s = Self {
            id: *id as u16,
            ino: *ino,
            path: path.to_string(),
            numa: hmg
                .iter()
                .map(|(numa_id, mglru)| (*numa_id as u32, Numa::new(mglru)))
                .collect(),
            psi: psi::Period::new(&psi_path.join(path.trim_start_matches('/')), false),
            sleep_psi_exceeds_limit: 0,
        };
        info!("MemCgroup::new {:?}", s);
        s
    }

    fn update_from_hostmemcg(&mut self, hmg: &HashMap<usize, MGenLRU>) {
        self.numa
            .retain(|numa_id, _| hmg.contains_key(&(*numa_id as usize)));

        for (numa_id, mglru) in hmg {
            self.numa
                .entry(*numa_id as u32)
                .and_modify(|e| e.update(mglru))
                .or_insert(Numa::new(mglru));
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
    anon_eviction_max: u64,

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
    fn new(mcg: &MemCgroup, numa_id: usize, numa: &Numa) -> Self {
        Self {
            memcg_id: mcg.id as usize,
            numa_id: numa_id,
            path: mcg.path.clone(),
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
struct MemCgroups {
    timeout: Timeout,
    config: Config,
    id_map: HashMap<u16, MemCgroup>,
    ino2id: HashMap<usize, u16>,
    path2id: HashMap<String, u16>,
}

impl MemCgroups {
    fn new(config: Config) -> Self {
        Self {
            timeout: Timeout::new(config.period_secs),
            config,
            id_map: HashMap::new(),
            ino2id: HashMap::new(),
            path2id: HashMap::new(),
        }
    }

    /* Remove not exist in host or id, ino changed memcgroup */
    fn remove_changed(
        &mut self,
        mg_hash: &HashMap<String, (usize, usize, HashMap<usize, MGenLRU>)>,
    ) {
        /* Remove not exist in host or id, ino changed memcgroup */
        let mut remove_target = Vec::new();
        for (_, mg) in &self.id_map {
            let mut should_remove = true;

            if let Some((id, ino, _)) = mg_hash.get(&mg.path) {
                if mg.id as usize == *id && mg.ino == *ino {
                    should_remove = false;
                }
            }

            if should_remove {
                remove_target.push((mg.id, mg.ino, mg.path.clone()));
            }
        }

        for (id, ino, path) in remove_target {
            self.id_map.remove(&id);
            self.ino2id.remove(&ino);
            self.path2id.remove(&path);
            info!("Remove memcg {} {} {} because host changed.", id, ino, path)
        }
    }

    fn update_and_add(
        &mut self,
        mg_hash: &HashMap<String, (usize, usize, HashMap<usize, MGenLRU>)>,
    ) {
        for (path, (id, ino, hmg)) in mg_hash {
            if *id == 0 {
                info!(
                    "Not add {} {} {} because it is disabled.",
                    *id,
                    *ino,
                    path.to_string()
                )
            }
            if let Some(mg) = self.id_map.get_mut(&(*id as u16)) {
                mg.update_from_hostmemcg(&hmg);
            } else {
                self.id_map.insert(
                    *id as u16,
                    MemCgroup::new(id, ino, path, hmg, &self.config.psi_path),
                );
                self.ino2id.insert(*ino, *id as u16);
                self.path2id.insert(path.to_string(), *id as u16);
            }
        }
    }

    fn check_psi_get_info(&mut self) -> Vec<Info> {
        let mut info_ret = Vec::new();

        for (_, mcg) in self.id_map.iter_mut() {
            let percent = match mcg.psi.get_percent() {
                Ok(p) => p,
                Err(e) => {
                    debug!("mcg.psi.get_percent {} failed: {}", mcg.path, e);
                    continue;
                }
            };
            if percent > self.config.period_psi_percent_limit as u64 {
                mcg.sleep_psi_exceeds_limit += 1;
                info!("{} period psi {}% exceeds limit", mcg.path, percent);
                continue;
            }

            for (numa_id, numa) in &mcg.numa {
                info_ret.push(Info::new(&mcg, *numa_id as usize, numa));
            }
        }

        info_ret
    }

    fn update_info(&self, infov: &mut Vec<Info>) {
        let mut i = 0;
        while i < infov.len() {
            if let Some(mg) = self.id_map.get(&(infov[i].memcg_id as u16)) {
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
            if let Some(mg) = self.id_map.get_mut(&(infov[i].memcg_id as u16)) {
                if let Some(numa) = mg.numa.get_mut(&(infov[i].numa_id as u32)) {
                    numa.run_aging_count += 1;
                    if numa.run_aging_count >= self.config.eviction_run_aging_count_min {
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
            if let Some(mg) = self.id_map.get_mut(&(info.memcg_id as u16)) {
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
        if let Some(s) = new_config.swap {
            self.config.swap = s;
        }
        if let Some(s) = new_config.swappiness_max {
            self.config.swappiness_max = s;
        }
        if let Some(p) = new_config.psi_path {
            self.config.psi_path = p.clone();
        }
        if let Some(p) = new_config.period_psi_percent_limit {
            self.config.period_psi_percent_limit = p;
        }
        if let Some(p) = new_config.eviction_psi_percent_limit {
            self.config.eviction_psi_percent_limit = p;
        }
        if let Some(p) = new_config.eviction_run_aging_count_min {
            self.config.eviction_run_aging_count_min = p;
        }
        if let Some(p) = new_config.period_secs {
            self.config.period_secs = p;
            self.timeout.set_sleep_duration(p);
            if !self.config.disabled {
                need_reset_mas = true;
            }
        }

        info!("new memcg config: {:#?}", self.config);
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
pub struct MemCG {
    memcgs: Arc<RwLock<MemCgroups>>,
}

impl MemCG {
    pub fn new(mut config: Config) -> Result<Self> {
        mglru::check().map_err(|e| anyhow!("mglru::check failed: {}", e))?;
        config.psi_path =
            psi::check(&config.psi_path).map_err(|e| anyhow!("psi::check failed: {}", e))?;

        let memcg = Self {
            memcgs: Arc::new(RwLock::new(MemCgroups::new(config))),
        };

        Ok(memcg)
    }

    /*
     * If target_paths.len == 0,
     * will remove the updated or not exist cgroup in the host from MemCgroups.
     * If target_paths.len > 0, will not do that.
     */
    fn refresh(&mut self, target_paths: &HashSet<String>) -> Result<()> {
        let mg_hash = mglru::host_memcgs_get(target_paths, true)
            .map_err(|e| anyhow!("lru_gen_parse::file_parse failed: {}", e))?;

        let mut mgs = self.memcgs.blocking_write();

        if target_paths.len() == 0 {
            mgs.remove_changed(&mg_hash);
        }
        mgs.update_and_add(&mg_hash);

        Ok(())
    }

    fn run_aging(&mut self, infov: &mut Vec<Info>, swap: bool) {
        infov.retain(|info| {
            let now = Utc::now();
            if now.signed_duration_since(info.last_inc_time).num_seconds() < IDLE_FRESH_IGNORE_SECS
            {
                info!(
                    "{} not run aging because last_inc_time {}",
                    info.path, info.last_inc_time,
                );
                true
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

        let total = anon_count + file_count;
        let c = 200 * anon_count / total;

        c as u8
    }

    fn run_eviction(&mut self, infov: &mut Vec<Info>, mut swap: bool) -> Result<()> {
        if self
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
                anon_eviction_max: 0,
                stop_reason: EvictionStopReason::None,
            });
        }

        let mut removed_infov = Vec::new();

        let mut ret = Ok(());
        let eviction_psi_percent_limit = self
            .memcgs
            .blocking_read()
            .config
            .eviction_psi_percent_limit as u64;

        let swappiness_max = self.memcgs.blocking_read().config.swappiness_max;

        'main_loop: while infov.len() != 0 {
            let mut i = 0;
            while i < infov.len() {
                let path_set: HashSet<String> =
                    infov.iter().map(|info| info.path.clone()).collect();
                match self.refresh(&path_set) {
                    Ok(_) => {}
                    Err(e) => {
                        ret = Err(anyhow!("refresh failed: {}", e));
                        break 'main_loop;
                    }
                };
                self.update_info(infov);

                let ci = infov[i].clone();

                trace!("{} {} run_eviction single loop start", ci.path, ci.numa_id);

                if let Some(ref mut ei) = infov[i].eviction {
                    if ei.last_min_lru_file == 0 && ei.last_min_lru_anon == 0 {
                        // First loop
                        trace!("{} {} run_eviction begin", ci.path, ci.numa_id,);
                        if ci.min_lru_file == 0 {
                            if !swap || ci.min_lru_anon == 0 {
                                info!(
                                "{} {} run_eviction stop because min_lru_file is 0 or min_lru_anon is 0, release {} {} pages",
                                ci.path, ci.numa_id, ei.anon_page_count, ei.file_page_count,
                            );
                                ei.stop_reason = EvictionStopReason::NoMinLru;
                                removed_infov.push(infov.remove(i));
                                continue;
                            } else {
                                ei.only_swap_mode = true;
                                ei.anon_eviction_max =
                                    ci.min_lru_anon * swappiness_max as u64 / 200;
                                trace!(
                                    "{} {} run_eviction only swap mode anon_eviction_max {}",
                                    ci.path,
                                    ci.numa_id,
                                    ei.anon_eviction_max
                                );
                            }
                        }

                        ei.last_min_lru_file = ci.min_lru_file;
                        ei.last_min_lru_anon = ci.min_lru_anon;
                    } else {
                        if (!ei.only_swap_mode && ci.min_lru_file >= ei.last_min_lru_file)
                            || (ei.only_swap_mode && ci.min_lru_file > 0)
                            || (swap && ci.min_lru_anon > ei.last_min_lru_anon)
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
                        } else {
                            if ei.anon_page_count >= ei.anon_eviction_max {
                                info!(
                                "{} {} run_eviction stop because anon_page_count is bigger than anon_eviction_max {}, release {} {} pages",
                                ci.path, ci.numa_id, ei.anon_eviction_max, ei.anon_page_count, ei.file_page_count,
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
                        if percent > eviction_psi_percent_limit {
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

                    // get swapiness
                    let swappiness = if ei.only_swap_mode {
                        200
                    } else if !swap
                        || ci.min_lru_anon == 0
                        || match self.swap_not_available() {
                            Ok(b) => b,
                            Err(e) => {
                                ret = Err(anyhow!("swap_not_available failed: {:?}", e));
                                break 'main_loop;
                            }
                        }
                    {
                        0
                    } else {
                        let s = self.get_swappiness(ci.min_lru_anon, ci.min_lru_file);
                        if s > swappiness_max {
                            swappiness_max
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

    fn check_psi_get_info(&mut self) -> Vec<Info> {
        self.memcgs.blocking_write().check_psi_get_info()
    }

    fn update_info(&self, infov: &mut Vec<Info>) {
        self.memcgs.blocking_read().update_info(infov);
    }

    pub fn need_work(&self) -> bool {
        self.memcgs.blocking_read().need_work()
    }

    pub fn reset_timer(&mut self) {
        self.memcgs.blocking_write().timeout.reset();
    }

    pub fn get_remaining_tokio_duration(&self) -> TokioDuration {
        self.memcgs.blocking_read().get_remaining_tokio_duration()
    }

    pub async fn async_get_remaining_tokio_duration(&self) -> TokioDuration {
        self.memcgs.read().await.get_remaining_tokio_duration()
    }

    pub fn work(&mut self) -> Result<()> {
        /* Refresh memcgroups info from host and store it to infov.  */
        self.refresh(&HashSet::new())
            .map_err(|e| anyhow!("first refresh failed: {}", e))?;
        let mut infov = self.check_psi_get_info();

        let swap = self.memcgs.blocking_read().config.swap;

        /* Run aging with infov.  */
        self.run_aging(&mut infov, swap);

        self.run_eviction(&mut infov, swap)
            .map_err(|e| anyhow!("run_eviction failed: {}", e))?;

        Ok(())
    }

    pub async fn set_config(&mut self, new_config: OptionConfig) -> bool {
        self.memcgs.write().await.set_config(new_config)
    }

    pub async fn get_status(&self) -> Vec<MemCgroup> {
        let mut mcgs = Vec::new();

        let mgs = self.memcgs.read().await;
        for (_, m) in mgs.id_map.iter() {
            mcgs.push((*m).clone());
        }

        mcgs
    }
}

mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn test_memcg_swap_not_available() {
        let m = MemCG::new(Config::default()).unwrap();
        assert!(m.swap_not_available().is_ok());
    }

    #[test]
    fn test_memcg_get_swappiness() {
        let m = MemCG::new(Config::default()).unwrap();
        assert_eq!(m.get_swappiness(100, 50), 133);
    }

    #[test]
    fn test_memcg_need_work() {
        let m = MemCG::new(Config::default()).unwrap();
        assert_eq!(m.need_work(), true);
    }
}
