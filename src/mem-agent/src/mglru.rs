// Copyright (C) 2023 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use crate::debug;
use crate::warn;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader};
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;

const WORKINGSET_ANON: usize = 0;
const WORKINGSET_FILE: usize = 1;
const LRU_GEN_ENABLED_PATH: &str = "/sys/kernel/mm/lru_gen/enabled";
const LRU_GEN_PATH: &str = "/sys/kernel/debug/lru_gen";
const MEMCGS_PATH: &str = "/sys/fs/cgroup/memory";

fn lru_gen_head_parse(line: &str) -> Result<(usize, String)> {
    let words: Vec<&str> = line.split_whitespace().map(|word| word.trim()).collect();
    if words.len() != 3 || words[0] != "memcg" {
        return Err(anyhow!("line {} format is not right", line));
    }

    let id = usize::from_str_radix(words[1], 10)
        .map_err(|e| anyhow!("parse line {} failed: {}", line, e))?;

    Ok((id, words[2].to_string()))
}

#[derive(Debug, PartialEq)]
pub struct GenLRU {
    pub seq: u64,
    pub anon: u64,
    pub file: u64,
    pub birth: DateTime<Utc>,
}

impl GenLRU {
    fn new() -> Self {
        Self {
            seq: 0,
            anon: 0,
            file: 0,
            birth: Utc::now(),
        }
    }
}

#[derive(Debug, PartialEq)]
pub struct MGenLRU {
    pub min_seq: u64,
    pub max_seq: u64,
    pub last_birth: DateTime<Utc>,
    pub min_lru_index: usize,
    pub lru: Vec<GenLRU>,
}

impl MGenLRU {
    fn new() -> Self {
        Self {
            min_seq: 0,
            max_seq: 0,
            last_birth: Utc::now(),
            min_lru_index: 0,
            lru: Vec::new(),
        }
    }
}

//result:
//  last_line, HashMap<node_id, MGenLRU>
fn lru_gen_lines_parse(reader: &mut BufReader<File>) -> Result<(String, HashMap<usize, MGenLRU>)> {
    let mut line = String::new();
    let mut ret_hash = HashMap::new();
    while line.len() > 0
        || reader
            .read_line(&mut line)
            .map_err(|e| anyhow!("read file {} failed: {}", LRU_GEN_PATH, e))?
            > 0
    {
        let words: Vec<&str> = line.split_whitespace().map(|word| word.trim()).collect();
        if words.len() == 2 && words[0] == "node" {
            // Got a new node
            let node_id = usize::from_str_radix(words[1], 10)
                .map_err(|e| anyhow!("parse line {} failed: {}", line, e))?;
            let (ret_line, node_size) = lru_gen_seq_lines_parse(reader)
                .map_err(|e| anyhow!("lru_gen_seq_lines_parse failed: {}", e))?;
            if let Some(size) = node_size {
                ret_hash.insert(node_id, size);
            }
            line = ret_line;
        } else {
            // Cannot get node, return the line let caller handle it.
            break;
        }
    }

    Ok((line, ret_hash))
}

fn str_to_u64(str: &str) -> Result<u64> {
    if str.starts_with("-") {
        warn!("{} format {} is not right", LRU_GEN_PATH, str);
        return Ok(0);
    }
    Ok(u64::from_str_radix(str, 10)?)
}

//result:
//  last_line, Option<MGenLRU>
fn lru_gen_seq_lines_parse(reader: &mut BufReader<File>) -> Result<(String, Option<MGenLRU>)> {
    let mut line = String::new();
    let mut ret = MGenLRU::new();
    let mut got = false;

    while reader
        .read_line(&mut line)
        .map_err(|e| anyhow!("read file {} failed: {}", LRU_GEN_PATH, e))?
        > 0
    {
        let words: Vec<&str> = line.split_whitespace().map(|word| word.trim()).collect();
        if words.len() != 4 {
            //line is not format of seq line
            break;
        }

        let msecs = i64::from_str_radix(words[1], 10)
            .map_err(|e| anyhow!("parse line {} failed: {}", line, e))?;
        // Use milliseconds because will got build error with try_milliseconds.
        #[allow(deprecated)]
        let birth = Utc::now() - Duration::milliseconds(msecs);

        let mut gen = GenLRU::new();
        gen.birth = birth;

        gen.seq = u64::from_str_radix(words[0], 10)
            .map_err(|e| anyhow!("parse line {} failed: {}", line, e))?;
        gen.anon = str_to_u64(&words[2 + WORKINGSET_ANON])
            .map_err(|e| anyhow!("parse line {} failed: {}", line, e))?;
        gen.file = str_to_u64(&words[2 + WORKINGSET_FILE])
            .map_err(|e| anyhow!("parse line {} failed: {}", line, e))?;

        if !got {
            ret.min_seq = gen.seq;
            ret.max_seq = gen.seq;
            ret.last_birth = birth;
            got = true;
        } else {
            ret.min_seq = std::cmp::min(ret.min_seq, gen.seq);
            ret.max_seq = std::cmp::max(ret.max_seq, gen.seq);
            if ret.last_birth < birth {
                ret.last_birth = birth;
            }
        }
        if gen.seq == ret.min_seq {
            ret.min_lru_index = ret.lru.len();
        }
        ret.lru.push(gen);

        line.clear();
    }

    Ok((line, if got { Some(ret) } else { None }))
}

// Just handle the path in the target_patchs. But if len of target_patchs is 0, will handle all paths.
// if parse_line is false
// HashMap<node_id, MGenLRU> will be empty.
//result:
// HashMap<path, (id, HashMap<node_id, MGenLRU>)>
fn lru_gen_file_parse(
    mut reader: &mut BufReader<File>,
    target_patchs: &HashSet<String>,
    parse_line: bool,
) -> Result<HashMap<String, (usize, HashMap<usize, MGenLRU>)>> {
    let mut line = String::new();
    let mut ret_hash = HashMap::new();
    while line.len() > 0
        || reader
            .read_line(&mut line)
            .map_err(|e| anyhow!("read file {} failed: {}", LRU_GEN_PATH, e))?
            > 0
    {
        let mut clear_line = true;
        // Not handle the Err of lru_gen_head_parse because all lines of file will be checked.
        if let Ok((id, path)) = lru_gen_head_parse(&line) {
            if target_patchs.len() == 0 || target_patchs.contains(&path) {
                let seq_data = if parse_line {
                    let (ret_line, data) = lru_gen_lines_parse(&mut reader).map_err(|e| {
                        anyhow!(
                            "lru_gen_seq_lines_parse file {} failed: {}",
                            LRU_GEN_PATH,
                            e
                        )
                    })?;
                    line = ret_line;
                    clear_line = false;
                    data
                } else {
                    HashMap::new()
                };

                /*trace!(
                    "lru_gen_file_parse path {} id {} seq_data {:#?}",
                    path,
                    id,
                    seq_data
                );*/

                ret_hash.insert(path.clone(), (id, seq_data));
            }
        }
        if clear_line {
            line.clear();
        }
    }
    Ok(ret_hash)
}

fn file_parse(
    target_patchs: &HashSet<String>,
    parse_line: bool,
) -> Result<HashMap<String, (usize, HashMap<usize, MGenLRU>)>> {
    let file = File::open(LRU_GEN_PATH)
        .map_err(|e| anyhow!("open file {} failed: {}", LRU_GEN_PATH, e))?;

    let mut reader = BufReader::new(file);

    lru_gen_file_parse(&mut reader, target_patchs, parse_line)
}

//result:
// HashMap<path, (id, ino, HashMap<node_id, MGenLRU>)>
pub fn host_memcgs_get(
    target_patchs: &HashSet<String>,
    parse_line: bool,
) -> Result<HashMap<String, (usize, usize, HashMap<usize, MGenLRU>)>> {
    let mgs = file_parse(target_patchs, parse_line)
        .map_err(|e| anyhow!("mglru file_parse failed: {}", e))?;

    let mut host_mgs = HashMap::new();
    for (path, (id, mglru)) in mgs {
        let host_path = PathBuf::from(MEMCGS_PATH).join(path.trim_start_matches('/'));

        let metadata = match fs::metadata(host_path.clone()) {
            Err(e) => {
                if id != 0 {
                    debug!("fs::metadata {:?} fail: {}", host_path, e);
                }
                continue;
            }
            Ok(m) => m,
        };

        host_mgs.insert(path, (id, metadata.ino() as usize, mglru));
    }

    Ok(host_mgs)
}

pub fn check() -> Result<()> {
    if crate::misc::is_test_environment() {
        return Ok(());
    }

    let content = fs::read_to_string(LRU_GEN_ENABLED_PATH)
        .map_err(|e| anyhow!("open file {} failed: {}", LRU_GEN_ENABLED_PATH, e))?;
    let content = content.trim();
    let r = if content.starts_with("0x") {
        u32::from_str_radix(&content[2..], 16)
    } else {
        content.parse()
    };
    let enabled = r.map_err(|e| anyhow!("parse file {} failed: {}", LRU_GEN_ENABLED_PATH, e))?;
    if enabled != 7 {
        fs::write(LRU_GEN_ENABLED_PATH, "7")
            .map_err(|e| anyhow!("write file {} failed: {}", LRU_GEN_ENABLED_PATH, e))?;
    }

    let _ = OpenOptions::new()
        .read(true)
        .write(true)
        .open(LRU_GEN_PATH)
        .map_err(|e| anyhow!("open file {} failed: {}", LRU_GEN_PATH, e))?;

    Ok(())
}

pub fn run_aging(
    memcg_id: usize,
    numa_id: usize,
    max_seq: u64,
    can_swap: bool,
    force_scan: bool,
) -> Result<()> {
    let cmd = format!(
        "+ {} {} {} {} {}",
        memcg_id, numa_id, max_seq, can_swap as i32, force_scan as i32
    );
    //trace!("send cmd {} to {}", cmd, LRU_GEN_PATH);
    fs::write(LRU_GEN_PATH, &cmd)
        .map_err(|e| anyhow!("write file {} cmd {} failed: {}", LRU_GEN_PATH, cmd, e))?;
    Ok(())
}

pub fn run_eviction(
    memcg_id: usize,
    numa_id: usize,
    min_seq: u64,
    swappiness: u8,
    nr_to_reclaim: usize,
) -> Result<()> {
    let cmd = format!(
        "- {} {} {} {} {}",
        memcg_id, numa_id, min_seq, swappiness, nr_to_reclaim
    );
    //trace!("send cmd {} to {}", cmd, LRU_GEN_PATH);
    fs::write(LRU_GEN_PATH, &cmd)
        .map_err(|e| anyhow!("write file {} cmd {} failed: {}", LRU_GEN_PATH, cmd, e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use maplit::hashmap;
    use once_cell::sync::OnceCell;
    use slog::{Drain, Level, Logger};
    use slog_async;
    use slog_scope::set_global_logger;
    use slog_term;
    use std::collections::HashMap;
    use std::fs;
    use std::fs::File;
    use std::io::BufReader;
    use std::io::Write;
    use std::sync::Mutex;

    lazy_static::lazy_static! {
        static ref TEST_MUTEX: Mutex<()> = Mutex::new(());
    }
    static LOGGER: OnceCell<slog_scope::GlobalLoggerGuard> = OnceCell::new();

    impl GenLRU {
        pub fn new_from_data(seq: u64, anon: u64, file: u64, birth: DateTime<Utc>) -> Self {
            Self {
                seq,
                anon,
                file,
                birth,
            }
        }
    }

    pub fn init_logger() -> &'static slog_scope::GlobalLoggerGuard {
        LOGGER.get_or_init(|| {
            let decorator = slog_term::TermDecorator::new().stderr().build();
            let drain = slog_term::CompactFormat::new(decorator)
                .build()
                .filter_level(Level::Trace)
                .fuse();
            let drain = slog_async::Async::new(drain).build().fuse();
            let logger = Logger::root(drain, slog::o!());
            set_global_logger(logger.clone())
        })
    }

    #[test]
    fn test_lru_gen_file_parse_single_no_parse() {
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _logger = init_logger();

        let mut reader = setup_test_file();
        let paths = ["/justto.slice/boot.mount".to_string()]
            .iter()
            .cloned()
            .collect();
        let ret = lru_gen_file_parse(&mut reader, &paths, false).unwrap();
        assert_eq!(ret.len(), 1);
        assert_eq!(
            ret.get("/justto.slice/boot.mount"),
            Some(&(16, HashMap::new()))
        );
        remove_test_file();
    }

    #[test]
    fn test_lru_gen_file_parse_multi_no_parse() {
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _logger = init_logger();

        let mut reader = setup_test_file();
        let paths = [
            "/aabbc/tea-loglogl".to_string(),
            "/aabbc/staraabbc".to_string(),
            "/aabbc/TEAE-iaabbc".to_string(),
            "/justto.slice/cpupower.service".to_string(),
        ]
        .iter()
        .cloned()
        .collect();
        let ret = lru_gen_file_parse(&mut reader, &paths, false).unwrap();
        assert_eq!(ret.len(), 4);
        assert_eq!(ret.get("/justto.slice/boot.mount"), None);
        assert_eq!(ret.get("/aabbc/tea-loglogl"), Some(&(30, hashmap![])));
        assert_eq!(ret.get("/aabbc/staraabbc"), Some(&(22, hashmap![])));
        assert_eq!(ret.get("/aabbc/TEAE-iaabbc"), Some(&(21, hashmap![])));
        assert_eq!(
            ret.get("/justto.slice/cpupower.service"),
            Some(&(0, hashmap![]))
        );
        remove_test_file();
    }

    #[test]
    fn test_lru_gen_file_parse_multi_parse() {
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _logger = init_logger();

        let mut reader = setup_test_file();
        let paths = [
            "/aabbc/tea-loglogl".to_string(),
            "/aabbc/staraabbc".to_string(),
            "/aabbc/TEAE-iaabbc".to_string(),
            "/justto.slice/cpupower.service".to_string(),
        ]
        .iter()
        .cloned()
        .collect();
        let ret = lru_gen_file_parse(&mut reader, &paths, true).unwrap();
        assert_eq!(ret.len(), 4);
        assert_eq!(ret.get("/justto.slice/boot.mount"), None);
        let birth_vec: Vec<DateTime<Utc>> = ret["/aabbc/tea-loglogl"].1[&0]
            .lru
            .iter()
            .map(|g| g.birth)
            .collect();
        assert_eq!(
            ret.get("/aabbc/tea-loglogl"),
            Some(&(
                30,
                hashmap![0 => MGenLRU{min_seq: 0, max_seq: 3, last_birth: birth_vec[3], min_lru_index: 0,
                                      lru: vec![GenLRU::new_from_data(0, 20, 23, birth_vec[0]),
                                                GenLRU::new_from_data(1, 9, 23, birth_vec[1]),
                                                GenLRU::new_from_data(2, 20, 19, birth_vec[2]),
                                                GenLRU::new_from_data(3, 3, 8, birth_vec[3])]}]
            ))
        );
        let birth_vec: Vec<DateTime<Utc>> = ret["/aabbc/staraabbc"].1[&1]
            .lru
            .iter()
            .map(|g| g.birth)
            .collect();
        assert_eq!(
            ret.get("/aabbc/staraabbc"),
            Some(&(
                22,
                hashmap![1 => MGenLRU{min_seq: 2, max_seq: 5, last_birth: birth_vec[3],  min_lru_index: 0,
                                      lru: vec![GenLRU::new_from_data(2, 0, 86201, birth_vec[0]),
                                                GenLRU::new_from_data(3, 253, 0, birth_vec[1]),
                                                GenLRU::new_from_data(4, 0, 0, birth_vec[2]),
                                                GenLRU::new_from_data(5, 2976, 41252, birth_vec[3])]}]
            ))
        );
        let birth_vec: Vec<DateTime<Utc>> = ret["/aabbc/TEAE-iaabbc"].1[&0]
            .lru
            .iter()
            .map(|g| g.birth)
            .collect();
        let birth1_vec: Vec<DateTime<Utc>> = ret["/aabbc/TEAE-iaabbc"].1[&1]
            .lru
            .iter()
            .map(|g| g.birth)
            .collect();
        assert_eq!(
            ret.get("/aabbc/TEAE-iaabbc"),
            Some(&(
                21,
                hashmap![0 => MGenLRU{min_seq: 0, max_seq: 3, last_birth: birth_vec[3], min_lru_index: 0, lru: vec![GenLRU::new_from_data(0, 0, 1, birth_vec[0]),
                                                                        GenLRU::new_from_data(1, 2, 3, birth_vec[1]),
                                                                        GenLRU::new_from_data(2, 6, 7, birth_vec[2]),
                                                                        GenLRU::new_from_data(3, 8, 9, birth_vec[3])]},
                         1 => MGenLRU{min_seq: 3, max_seq: 6, last_birth: birth1_vec[3], min_lru_index: 0, lru: vec![GenLRU::new_from_data(3, 10, 11, birth1_vec[0]),
                                                                        GenLRU::new_from_data(4, 12, 16, birth1_vec[1]),
                                                                        GenLRU::new_from_data(5, 17, 18, birth1_vec[2]),
                                                                        GenLRU::new_from_data(6, 19, 20, birth1_vec[3])]}]
            ))
        );
        let birth_vec: Vec<DateTime<Utc>> = ret["/justto.slice/cpupower.service"].1[&0]
            .lru
            .iter()
            .map(|g| g.birth)
            .collect();
        assert_eq!(
            ret.get("/justto.slice/cpupower.service"),
            Some(&(
                0,
                hashmap![0 => MGenLRU{min_seq: 0, max_seq: 3, last_birth: birth_vec[3],  min_lru_index: 0, lru: vec![GenLRU::new_from_data(0, 0, 33, birth_vec[0]),
                                                                        GenLRU::new_from_data(1, 0, 0, birth_vec[1]),
                                                                        GenLRU::new_from_data(2, 0, 0, birth_vec[2]),
                                                                        GenLRU::new_from_data(3, 0, 115, birth_vec[3])]}]
            ))
        );
        remove_test_file();
    }

    #[test]
    fn test_lru_gen_file_parse_no_target_no_parse() {
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _logger = init_logger();

        let mut reader = setup_test_file();
        let paths = [].iter().cloned().collect();
        let ret = lru_gen_file_parse(&mut reader, &paths, false).unwrap();
        assert_eq!(ret.len(), 55);
        assert_eq!(ret.get("/justto.slice/boot.mount"), Some(&(16, hashmap![])));
        assert_eq!(ret.get("/aabbc/tea-loglogl"), Some(&(30, hashmap![])));
        assert_eq!(ret.get("/aabbc/staraabbc"), Some(&(22, hashmap![])));
        assert_eq!(ret.get("/aabbc/TEAE-iaabbc"), Some(&(21, hashmap![])));
        assert_eq!(
            ret.get("/justto.slice/cpupower.service"),
            Some(&(0, hashmap![]))
        );
        remove_test_file();
    }

    #[test]
    fn test_lru_gen_file_parse_no_target_parse() {
        let _lock = TEST_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        let _logger = init_logger();

        let mut reader = setup_test_file();
        let paths = [].iter().cloned().collect();
        let ret = lru_gen_file_parse(&mut reader, &paths, true).unwrap();
        assert_eq!(ret.len(), 55);
        let birth_vec: Vec<DateTime<Utc>> = ret["/aabbc/tea-loglogl"].1[&0]
            .lru
            .iter()
            .map(|g| g.birth)
            .collect();
        assert_eq!(
            ret.get("/aabbc/tea-loglogl"),
            Some(&(
                30,
                hashmap![0 => MGenLRU{min_seq: 0, max_seq: 3, last_birth: birth_vec[3],  min_lru_index: 0,
                                      lru: vec![GenLRU::new_from_data(0, 20, 23, birth_vec[0]),
                                                GenLRU::new_from_data(1, 9, 23, birth_vec[1]),
                                                GenLRU::new_from_data(2, 20, 19, birth_vec[2]),
                                                GenLRU::new_from_data(3, 3, 8, birth_vec[3])]}]
            ))
        );
        let birth_vec: Vec<DateTime<Utc>> = ret["/aabbc/staraabbc"].1[&1]
            .lru
            .iter()
            .map(|g| g.birth)
            .collect();
        assert_eq!(
            ret.get("/aabbc/staraabbc"),
            Some(&(
                22,
                hashmap![1 => MGenLRU{min_seq: 2, max_seq: 5, last_birth: birth_vec[3],  min_lru_index: 0,
                                      lru: vec![GenLRU::new_from_data(2, 0, 86201, birth_vec[0]),
                                                GenLRU::new_from_data(3, 253, 0, birth_vec[1]),
                                                GenLRU::new_from_data(4, 0, 0, birth_vec[2]),
                                                GenLRU::new_from_data(5, 2976, 41252, birth_vec[3])]}]
            ))
        );
        remove_test_file();
    }

    fn setup_test_file() -> BufReader<File> {
        let data = r#"
        memcg     1 /
        node     0
                 0  589359037          0             -0 
                 1  589359037         12           0 
                 2  589359037          0           0 
                 3  589359037       1265        2471 
       memcg     2 /justto.slice
        node     0
                 0  589334424          0           0 
                 1  589334424          0           0 
                 2  589334424          0           0 
                 3  589334424          0           0 
       memcg     3 /justto.slice/justtod-teawated.service
        node     0
                 0  589334423          0         217 
                 1  589334423          8           0 
                 2  589334423          0           0 
                 3  589334423        225       40293 
       memcg     0 /justto.slice/justtod-readahead-replay.service
        node     0
                 0  589334411          0      266694 
                 1  589334411          0           0 
                 2  589334411          0           0 
                 3  589334411          1          21 
       memcg     0 /justto.slice/tea0-domainname.service
        node     0
                 0  589334410          0           6 
                 1  589334410          0           0 
                 2  589334410          0           0 
                 3  589334410          0         198 
       memcg     6 /justto.slice/justto-serial\x2dgetty.slice
        node     0
                 0  589334408          0           1 
                 1  589334408          1           0 
                 2  589334408          0           0 
                 3  589334408         32           0 
       memcg     7 /justto.slice/justto-getty.slice
        node     0
                 0  589334408          0           0 
                 1  589334408          1           0 
                 2  589334408          0           0 
                 3  589334408         31           0 
       memcg     8 /justto.slice/sys-kernel-debug.mount
        node     0
                 0  589334407          0           6 
                 1  589334408          0           0 
                 2  589334408          0           0 
                 3  589334408          0           7 
       memcg    10 /justto.slice/dev-hugepages.mount
        node     0
                 0  589334406          0           1 
                 1  589334406          0           0 
                 2  589334406          0           0 
                 3  589334406          0           0 
       memcg     0 /justto.slice/justtod-readahead-collect.service
        node     0
                 0  589334405          0          96 
                 1  589334405          0           0 
                 2  589334405          0           0 
                 3  589334405          0           0 
       memcg    12 /justto.slice/justto-justtod\x2dfsck.slice
        node     0
                 0  589334403          0          25 
                 1  589334403          0           0 
                 2  589334403          0           0 
                 3  589334403          0         239 
       memcg    13 /justto.slice/justto-selinux\x2dpolicy\x2dmigrate\x2dlocal\x2dchanges.slice
        node     0
                 0  589334403          0           0 
                 1  589334403          0           0 
                 2  589334403          0           0 
                 3  589334403          0           0 
       memcg    14 /justto.slice/dev-mqueue.mount
        node     0
                 0  589334402          0           0 
                 1  589334402          0           0 
                 2  589334402          0           0 
                 3  589334402          0           0 
       memcg     0 /justto.slice/tea2-monitor.service
        node     0
                 0  589334401          0           9 
                 1  589334401          0           0 
                 2  589334401          0           0 
                 3  589334401          0         582 
       memcg     0 /justto.slice/kmod-static-nodes.service
        node     0
                 0  589334399          0           4 
                 1  589334399          1           0 
                 2  589334399          0           0 
                 3  589334399          0          33 
       memcg     0 /justto.slice/plymouth-start.service
        node     0
                 0  589334397          0           1 
                 1  589334397          0           0 
                 2  589334397          0           0 
                 3  589334397          0           0 
       memcg    18 /justto.slice/sys-kernel-config.mount
        node     0
                 0  589334396          0           0 
                 1  589334396          0           0 
                 2  589334396          0           0 
                 3  589334396          0           0 
       memcg     5 /justto.slice/tea2-teaetad.service
        node     0
                 0  589334383          0           4 
                 1  589334383          2           0 
                 2  589334383          0           0 
                 3  589334383        587          14 
       memcg     0 /justto.slice/justtod-remount-fs.service
        node     0
                 0  589334381          0           4 
                 1  589334381          0           0 
                 2  589334381          0           0 
                 3  589334381          0          11 
       memcg     0 /justto.slice/justtod-tmpfiles-setup-dev.service
        node     0
                 0  589334380          0          28 
                 1  589334380          0           0 
                 2  589334380          0           0 
                 3  589334380          0          32 
       memcg     0 /justto.slice/justtod-sysctl.service
        node     0
                 0  589334378          0          42 
                 1  589334378          0           0 
                 2  589334378          0           0 
                 3  589334378          0          13 
       memcg     0 /justto.slice/justtod-teawate-flush.service
        node     0
                 0  589334367          0           8 
                 1  589334367          0           0 
                 2  589334367          0           0 
                 3  589334367          0         141 
       memcg     0 /justto.slice/justtod-udev-trigger.service
        node     0
                 0  589334365          0           5 
                 1  589334365          0           0 
                 2  589334365          0           0 
                 3  589334365          0         103 
       memcg     0 /justto.slice/tea0-readonly.service
        node     0
                 0  589334364          0         163 
                 1  589334364          0           0 
                 2  589334364          0           0 
                 3  589334364          0          35 
       memcg     0 /justto.slice/justtod-random-seed.service
        node     0
                 0  589334363          0          38 
                 1  589334363          0           0 
                 2  589334363          0           0 
                 3  589334363          0           9 
       memcg    25 /justto.slice/justtod-udevd.service
        node     0
                 0  589334362          0       12553 
                 1  589334362        249           0 
                 2  589334362          0           0 
                 3  589334362        124        1415 
       memcg    15 /justto.slice/dev-disk-by\x2dlabel-SWAP.swap
        node     0
                 0  589334085          0           5 
                 1  589334085          0           0 
                 2  589334085          0           0 
                 3  589334085          0          10 
       memcg    16 /justto.slice/boot.mount
        node     0
                 0  589334035          0          26 
                 1  589334035          0           0 
                 2  589334035          0           0 
                 3  589334035          0           0 
       memcg     0 /justto.slice/plymouth-read-write.service
        node     0
                 0  589334011          0           9 
                 1  589334011          0           0 
                 2  589334011          0           0 
                 3  589334011          0          27 
       memcg     0 /justto.slice/tea0-import-state.service
        node     0
                 0  589334008          0           5 
                 1  589334008          0           0 
                 2  589334008          0           0 
                 3  589334008          0          45 
       memcg     0 /justto.slice/justtod-tmpfiles-setup.service
        node     0
                 0  589333868          0           8 
                 1  589333868          0           0 
                 2  589333868          0           0 
                 3  589333868          0           0 
       memcg     0 /justto.slice/justtod-update-utmp.service
        node     0
                 0  589333772          0           5 
                 1  589333772          1           0 
                 2  589333772          0           0 
                 3  589333772          0          74 
       memcg     0 /justto.slice/network.service
        node     0
                 0  589333758          0        2480 
                 1  589333758          0           0 
                 2  589333758          0           0 
                 3  589333758          0         542 
       memcg     0 /justto.slice/tea0-dmesg.service
        node     0
                 0  589333757          0          35 
                 1  589333757          0           0 
                 2  589333757          0           0 
                 3  589333757          0          37 
       memcg     0 /justto.slice/cpupower.service
        node     0
                 0  589333755          0          33 
                 1  589333755          0           0 
                 2  589333755          0           0 
                 3  589333755          0         115 
       memcg     0 /justto.slice/justtod-user-sessions.service
        node     0
                 0  589333749          0           4 
                 1  589333749          0           0 
                 2  589333749          0           0 
                 3  589333749          0           8 
       memcg     0 /justto.slice/sysstat.service
        node     0
                 0  589333747          0          17 
                 1  589333747          0           0 
                 2  589333747          0           0 
                 3  589333747          0          41 
       memcg    26 /justto.slice/mcelog.service
        node     0
                 0  589333745          0          41 
                 1  589333745          2           0 
                 2  589333745          0           0 
                 3  589333745        554          37 
       memcg    27 /justto.slice/dbus.service
        node     0
                 0  589333743          0          82 
                 1  589333743          1           0 
                 2  589333743          0           0 
                 3  589333743        119         216 
       memcg    20 /justto.slice/syslog-ng.service
        node     0
                 0  589333722          0        5889 
                 1  589333722          2           0 
                 2  589333722          0           0 
                 3  589333722        418        6488 
       memcg     0 /justto.slice/cpunoturbo.service
        node     0
                 0  589333596          0           1 
                 1  589333596          0           0 
                 2  589333596          0           0 
                 3  589333596          0           0 
       memcg     4 /justto.slice/staraabbcctl.service
        node     0
                 0  589327556          0           7 
                 1  589327556          2           0 
                 2  589327556          0           0 
                 3  589327556         69          11 
       memcg    19 /justto.slice/sshd.service
        node     0
                 0  589327547          0        9670 
                 1  589327547         11           0 
                 2  589327547          0           0 
                 3  589327547       2304         421 
       memcg     0 /justto.slice/vmcore-collect.service
        node     0
                 0  589327544          0           3 
                 1  589327544          0           0 
                 2  589327544          0           0 
                 3  589327544          0           0 
       memcg     0 /justto.slice/kdump.service
        node     0
                 0  589327543          0      417259 
                 1  589327543          2           0 
                 2  589327543          0           0 
                 3  589327543          0           0 
       memcg    31 /justto.slice/proc-sys-fs-binfmt_misc.mount
        node     0
                 0  589311768          0           0 
                 1  589311768          0           0 
                 2  589311768          0           0 
                 3  589311768          0           0 
       memcg    32 /justto.slice/ntpd.service
        node     0
                 0  589297199          0          14 
                 1  589297199          2           0 
                 2  589297199          0           0 
                 3  589297199        120         198 
       memcg    29 /justto.slice/crond.service
        node     0
                 0  589297184          0      115157 
                 1  589297184          2           0 
                 2  589297184          0           0 
                 3  589297184        195         324 
       memcg     0 /justto.slice/justtod-tmpfiles-clean.service
        node     0
                 0  588459896          0           8 
                 1  588459896          0           0 
                 2  588459896          0           0 
                 3  588459896          0           0 
       memcg     9 /docker.slice
        node     0
                 0  589334407          0       13919 
                 1  589334407          7           0 
                 2  589334407          0           0 
                 3  589334407       7254      146884 
       memcg    17 /aabbc
        node     0
                 0  589327431          0           0 
                 1  589327431          0           0 
                 2  589327431          0           0 
                 3  589327431          0           0 
       memcg    22 /aabbc/staraabbc
        node     1
                 2  589327430          0       86201 
                 3  589327430        253           0 
                 4  589327430          0           0 
                 5  589327430       2976       41252 
       memcg    21 /aabbc/TEAE-iaabbc
        node     0
                 0  589324388          0           1 
                 1  589324388          2           3 
                 2  589324388          6           7 
                 3  589324388          8           9 
        node     1
                 3  589324388         10          11 
                 4  589324388         12          16 
                 5  589324388         17          18 
                 6  589324388         19          20 
       memcg    28 /aabbc/teawa_tea
        node     0
                 0  589324387          0       69337 
                 1  589324387          2           0 
                 2  589324387          0           0 
                 3  589324387       1892        6103 
       memcg    30 /aabbc/tea-loglogl
        node     0
                 0  589324385         20          23 
                 1  589324385          9          23 
                 2  589324385         20          19 
                 3  589324380          3           8 

    "#;
        let mut file = File::create("test_lru_gen").unwrap();
        file.write_all(data.as_bytes()).unwrap();

        let file = File::open("test_lru_gen").unwrap();
        BufReader::new(file)
    }

    fn remove_test_file() {
        fs::remove_file("test_lru_gen").unwrap();
    }
}
