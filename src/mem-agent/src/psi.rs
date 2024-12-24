// Copyright (C) 2024 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use crate::info;
use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

const CGROUP_PATH: &str = "/sys/fs/cgroup/";
const MEM_PSI: &str = "memory.pressure";
const IO_PSI: &str = "io.pressure";

fn find_psi_subdirs() -> Result<PathBuf> {
    if PathBuf::from(CGROUP_PATH).is_dir() {
        for entry in fs::read_dir(CGROUP_PATH)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if path.join(MEM_PSI).is_file() && path.join(IO_PSI).is_file() {
                    return Ok(path.clone());
                }
            }
        }

        Err(anyhow!("cannot find cpuacct dir in {}", CGROUP_PATH))
    } else {
        Err(anyhow!("{} is not a directory", CGROUP_PATH))
    }
}

pub fn check(psi_path: &PathBuf) -> Result<PathBuf> {
    if crate::misc::is_test_environment() {
        return Ok(psi_path.clone());
    }

    let p = if psi_path.as_os_str().is_empty() {
        find_psi_subdirs().map_err(|e| anyhow!("find_psi_subdirs failed: {}", e))?
    } else {
        psi_path.clone()
    };

    let mem_psi_path = p.join(MEM_PSI);
    let _ = OpenOptions::new()
        .read(true)
        .write(true)
        .open(mem_psi_path.clone())
        .map_err(|e| anyhow!("open file {:?} failed: {}", mem_psi_path, e))?;

    info!("psi is available at {:?}", p);

    Ok(p)
}

fn read_pressure_some_total(file_path: PathBuf) -> Result<u64> {
    let file = File::open(file_path).map_err(|e| anyhow!("File::open failed: {}", e))?;
    let mut reader = BufReader::new(file);

    let mut first_line = String::new();
    if reader
        .read_line(&mut first_line)
        .map_err(|e| anyhow!("reader.read_line failed: {}", e))?
        <= 0
    {
        return Err(anyhow!("File is empty"));
    }

    let parts: Vec<&str> = first_line.split_whitespace().collect();
    let total_str = parts.get(4).ok_or_else(|| anyhow!("format is not right"))?;
    let val = total_str
        .split('=')
        .nth(1)
        .ok_or_else(|| anyhow!("format is not right"))?;

    let total_value = val
        .parse::<u64>()
        .map_err(|e| anyhow!("parse {} failed: {}", total_str, e))?;

    Ok(total_value)
}

#[derive(Debug, Clone)]
pub struct Period {
    path: PathBuf,
    last_psi: u64,
    last_update_time: DateTime<Utc>,
    include_child: bool,
}

impl Period {
    pub fn new(path: &PathBuf, include_child: bool) -> Self {
        Self {
            path: path.to_owned(),
            last_psi: 0,
            last_update_time: Utc::now(),
            include_child,
        }
    }

    fn get_path_pressure_us(&self, psi_name: &str) -> Result<u64> {
        let cur_path = self.path.join(psi_name);
        let mut parent_val = read_pressure_some_total(cur_path.clone())
            .map_err(|e| anyhow!("read_pressure_some_total {:?} failed: {}", cur_path, e))?;

        if !self.include_child {
            let mut child_val = 0;
            let entries = fs::read_dir(self.path.clone())
                .map_err(|e| anyhow!("fs::read_dir failed: {}", e))?;
            for entry in entries {
                let entry = entry.map_err(|e| anyhow!("get path failed: {}", e))?;
                let epath = entry.path();

                if epath.is_dir() {
                    let full_path = self.path.join(entry.file_name()).join(psi_name);

                    child_val += read_pressure_some_total(full_path.clone()).map_err(|e| {
                        anyhow!("read_pressure_some_total {:?} failed: {}", full_path, e)
                    })?;
                }
            }
            if parent_val < child_val {
                parent_val = 0;
            } else {
                parent_val -= child_val;
            }
        }

        Ok(parent_val)
    }

    pub fn get_percent(&mut self) -> Result<u64> {
        let now = Utc::now();
        let mut psi = self
            .get_path_pressure_us(MEM_PSI)
            .map_err(|e| anyhow!("get_path_pressure_us MEM_PSI {:?} failed: {}", self.path, e))?;
        psi += self
            .get_path_pressure_us(IO_PSI)
            .map_err(|e| anyhow!("get_path_pressure_us IO_PSI {:?} failed: {}", self.path, e))?;

        let mut percent = 0;

        if self.last_psi != 0 && self.last_psi < psi && self.last_update_time < now {
            let us = (now - self.last_update_time).num_milliseconds() as u64 * 1000;

            if us != 0 {
                percent = (psi - self.last_psi) * 100 / us;
            }
        }

        self.last_psi = psi;
        self.last_update_time = now;

        Ok(percent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_read_pressure_some_total() {
        remove_fake_file();
        let val = read_pressure_some_total(PathBuf::from(setup_fake_file())).unwrap();
        assert_eq!(val, 37820);
        remove_fake_file();
    }

    #[test]
    fn test_period() {
        remove_fake_cgroup_dir();

        let dir = setup_fake_cgroup_dir();

        let period = Period::new(&dir, true);
        let us = period.get_path_pressure_us(MEM_PSI).unwrap();
        assert_eq!(us, 37820);
        let us = period.get_path_pressure_us(IO_PSI).unwrap();
        assert_eq!(us, 82345);

        let period = Period::new(&dir, false);
        let us = period.get_path_pressure_us(MEM_PSI).unwrap();
        assert_eq!(us, 26688);
        let us = period.get_path_pressure_us(IO_PSI).unwrap();
        assert_eq!(us, 66879);

        remove_fake_cgroup_dir();
    }

    fn write_fake_file(path: &PathBuf, data: &str) {
        let mut file = File::create(path).unwrap();
        file.write_all(data.as_bytes()).unwrap();
    }

    fn setup_fake_file() -> String {
        let data = r#"some avg10=0.00 avg60=0.00 avg300=0.00 total=37820
        full avg10=0.00 avg60=0.00 avg300=0.00 total=28881
    "#;

        write_fake_file(&PathBuf::from("test_psi"), data);

        "test_psi".to_string()
    }

    fn remove_fake_file() {
        let _ = fs::remove_file("test_psi");
    }

    fn setup_fake_cgroup_dir() -> PathBuf {
        let dir = PathBuf::from("fake_cgroup");
        fs::create_dir(&dir).unwrap();
        let mem_psi = dir.join(MEM_PSI);
        let io_psi = dir.join(IO_PSI);
        let data = r#"some avg10=0.00 avg60=0.00 avg300=0.00 total=37820
        full avg10=0.00 avg60=0.00 avg300=0.00 total=28881
    "#;
        write_fake_file(&mem_psi, data);
        let data = r#"some avg10=0.00 avg60=0.00 avg300=0.00 total=82345
        full avg10=0.00 avg60=0.00 avg300=0.00 total=67890
    "#;
        write_fake_file(&io_psi, data);

        let child_dir = dir.join("c1");
        fs::create_dir(&child_dir).unwrap();
        let child_mem_psi = child_dir.join(MEM_PSI);
        let child_io_psi = child_dir.join(IO_PSI);
        let data = r#"some avg10=0.00 avg60=0.00 avg300=0.00 total=3344
        full avg10=0.00 avg60=0.00 avg300=0.00 total=1234
     "#;
        write_fake_file(&child_mem_psi, data);
        let data = r#"some avg10=0.00 avg60=0.00 avg300=0.00 total=5566
        full avg10=0.00 avg60=0.00 avg300=0.00 total=5678
     "#;
        write_fake_file(&child_io_psi, data);

        let child_dir = dir.join("c2");
        fs::create_dir(&child_dir).unwrap();
        let child_mem_psi = child_dir.join(MEM_PSI);
        let child_io_psi = child_dir.join(IO_PSI);
        let data = r#"some avg10=0.00 avg60=0.00 avg300=0.00 total=7788
        full avg10=0.00 avg60=0.00 avg300=0.00 total=4321
     "#;
        write_fake_file(&child_mem_psi, data);
        let data = r#"some avg10=0.00 avg60=0.00 avg300=0.00 total=9900
        full avg10=0.00 avg60=0.00 avg300=0.00 total=8765
     "#;
        write_fake_file(&child_io_psi, data);

        dir
    }

    fn remove_fake_cgroup_dir() {
        let _ = fs::remove_dir_all("fake_cgroup");
    }
}
