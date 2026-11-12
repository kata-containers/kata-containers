// Copyright (c) 2026 Google LLC
//
// SPDX-License-Identifier: Apache-2.0
//

use std::ffi::CString;
use std::fs::{self, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::{anyhow, bail, ensure, Context, Result};
use slog::{info, o, warn, Logger};

const ZRAM_SYSFS_DIR: &str = "/sys/block/zram0";
const ZRAM_DEV_PATH: &str = "/dev/zram0";
const PROC_MEMINFO_PATH: &str = "/proc/meminfo";
const PROC_SWAPS_PATH: &str = "/proc/swaps";
const VM_SWAPPINESS_PATH: &str = "/proc/sys/vm/swappiness";
const VM_PAGE_CLUSTER_PATH: &str = "/proc/sys/vm/page-cluster";

// Linux swap header constants (include/linux/swap.h)
const SWAP_HEADER_VERSION: u32 = 1;
const SWAP_HEADER_MAGIC: &[u8; 10] = b"SWAPSPACE2";
const SWAP_BOOTBITS_SIZE: usize = 1024;
const MIN_SWAP_PAGES: u64 = 10;

pub const DEFAULT_ZRAM_SIZE_PERCENT: u64 = 100;
pub const DEFAULT_ZRAM_COMP_ALGORITHM: &str = "lz4";
pub const DEFAULT_ZRAM_SWAPPINESS: u8 = 150;
pub const MAX_ZRAM_SWAPPINESS: u8 = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZramConfig {
    pub size_percent: u64,
    pub comp_algorithm: String,
    pub swappiness: u8,
}

impl Default for ZramConfig {
    fn default() -> Self {
        Self {
            size_percent: DEFAULT_ZRAM_SIZE_PERCENT,
            comp_algorithm: String::from(DEFAULT_ZRAM_COMP_ALGORITHM),
            swappiness: DEFAULT_ZRAM_SWAPPINESS,
        }
    }
}

pub fn parse_memtotal_bytes(meminfo: &str) -> Result<u64> {
    for line in meminfo.lines() {
        if let Some(rest) = line.strip_prefix("MemTotal:") {
            let kb_str = rest
                .split_whitespace()
                .next()
                .ok_or_else(|| anyhow!("missing value for MemTotal"))?;
            let kb = kb_str
                .parse::<u64>()
                .context("failed to parse MemTotal kB value")?;
            return kb
                .checked_mul(1024)
                .ok_or_else(|| anyhow!("MemTotal overflow in bytes"));
        }
    }
    bail!("MemTotal not found in meminfo")
}

pub fn calculate_disksize_bytes(
    memtotal_bytes: u64,
    size_percent: u64,
    page_size: usize,
) -> Result<u64> {
    ensure!(size_percent > 0, "zram size_percent must be > 0");
    ensure!(
        page_size >= 4096 && page_size.is_power_of_two(),
        "invalid system page_size: {page_size}"
    );

    let raw_bytes = memtotal_bytes
        .checked_mul(size_percent)
        .ok_or_else(|| anyhow!("zram size calculation overflow"))?
        / 100;

    let page_mask = !(page_size as u64 - 1);
    let aligned_bytes = raw_bytes & page_mask;
    let min_bytes = MIN_SWAP_PAGES * (page_size as u64);
    ensure!(
        aligned_bytes >= min_bytes,
        "calculated zram disksize ({aligned_bytes} bytes) is smaller than minimum swap size ({min_bytes} bytes)"
    );

    Ok(aligned_bytes)
}

pub fn format_swap_header(disksize_bytes: u64, page_size: usize) -> Result<Vec<u8>> {
    ensure!(
        page_size >= SWAP_BOOTBITS_SIZE + 32,
        "page_size ({page_size}) too small for swap header"
    );

    let total_pages = disksize_bytes / (page_size as u64);
    ensure!(
        total_pages >= MIN_SWAP_PAGES,
        "swap device requires at least {MIN_SWAP_PAGES} pages, got {total_pages}"
    );

    let last_page = (total_pages - 1).min(u32::MAX as u64) as u32;
    let nr_badpages: u32 = 0;

    let mut header = vec![0u8; page_size];
    header[SWAP_BOOTBITS_SIZE..SWAP_BOOTBITS_SIZE + 4]
        .copy_from_slice(&SWAP_HEADER_VERSION.to_ne_bytes());
    header[SWAP_BOOTBITS_SIZE + 4..SWAP_BOOTBITS_SIZE + 8]
        .copy_from_slice(&last_page.to_ne_bytes());
    header[SWAP_BOOTBITS_SIZE + 8..SWAP_BOOTBITS_SIZE + 12]
        .copy_from_slice(&nr_badpages.to_ne_bytes());

    let magic_offset = page_size - SWAP_HEADER_MAGIC.len();
    header[magic_offset..page_size].copy_from_slice(SWAP_HEADER_MAGIC);

    Ok(header)
}

pub fn write_swap_header(dev_path: &Path, disksize_bytes: u64, page_size: usize) -> Result<()> {
    let header = format_swap_header(disksize_bytes, page_size)?;
    let mut file = OpenOptions::new()
        .write(true)
        .open(dev_path)
        .with_context(|| format!("failed to open {} for swap formatting", dev_path.display()))?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(&header)?;
    file.sync_all()?;
    Ok(())
}

pub fn setup_zram(logger: &Logger, config: &ZramConfig) -> Result<()> {
    let logger = logger.new(o!("subsystem" => "zram"));

    let sysfs_dir = Path::new(ZRAM_SYSFS_DIR);
    let disksize_path = sysfs_dir.join("disksize");
    if !disksize_path.exists() {
        bail!(
            "{} does not exist (ensure CONFIG_ZRAM=y is enabled in the guest kernel)",
            disksize_path.display()
        );
    }

    if let Ok(swaps) = fs::read_to_string(PROC_SWAPS_PATH) {
        if swaps.lines().any(|line| line.starts_with(ZRAM_DEV_PATH)) {
            info!(logger, "zram swap already active on {}", ZRAM_DEV_PATH);
            return Ok(());
        }
    }

    let meminfo = fs::read_to_string(PROC_MEMINFO_PATH)
        .with_context(|| format!("failed to read {PROC_MEMINFO_PATH}"))?;
    let memtotal_bytes = parse_memtotal_bytes(&meminfo)?;

    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    ensure!(page_size > 0, "sysconf(_SC_PAGESIZE) returned {page_size}");
    let page_size = page_size as usize;

    let disksize_bytes = calculate_disksize_bytes(memtotal_bytes, config.size_percent, page_size)?;

    let comp_alg_path = sysfs_dir.join("comp_algorithm");
    if !config.comp_algorithm.is_empty() && comp_alg_path.exists() {
        if let Err(e) = fs::write(&comp_alg_path, &config.comp_algorithm) {
            warn!(
                logger,
                "failed to set zram compression algorithm '{}', using kernel default: {}",
                config.comp_algorithm,
                e
            );
        }
    }

    fs::write(&disksize_path, disksize_bytes.to_string()).with_context(|| {
        format!(
            "failed to write {disksize_bytes} to {}",
            disksize_path.display()
        )
    })?;

    let dev_path = Path::new(ZRAM_DEV_PATH);
    write_swap_header(dev_path, disksize_bytes, page_size)?;

    let c_dev_path = CString::new(ZRAM_DEV_PATH).context("invalid zram device path")?;
    let ret = unsafe { libc::swapon(c_dev_path.as_ptr(), 0) };
    if ret != 0 {
        let err = std::io::Error::last_os_error();
        bail!("swapon({ZRAM_DEV_PATH}) failed: {err}");
    }

    if let Err(e) = fs::write(VM_SWAPPINESS_PATH, config.swappiness.to_string()) {
        warn!(logger, "failed to set {}: {}", VM_SWAPPINESS_PATH, e);
    }
    if let Err(e) = fs::write(VM_PAGE_CLUSTER_PATH, "0") {
        warn!(logger, "failed to set {}: {}", VM_PAGE_CLUSTER_PATH, e);
    }

    info!(
        logger,
        "initialized zram swap";
        "device" => ZRAM_DEV_PATH,
        "disksize_bytes" => disksize_bytes,
        "size_percent" => config.size_percent,
        "comp_algorithm" => &config.comp_algorithm,
        "swappiness" => config.swappiness,
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_memtotal_bytes() {
        let sample = "MemTotal:         360448 kB\nMemFree:          284120 kB\n";
        let bytes = parse_memtotal_bytes(sample).unwrap();
        assert_eq!(bytes, 360448 * 1024);

        assert!(parse_memtotal_bytes("MemFree: 100 kB\n").is_err());
        assert!(parse_memtotal_bytes("MemTotal: invalid kB\n").is_err());
    }

    #[test]
    fn test_calculate_disksize_bytes() {
        let memtotal = 352 * 1024 * 1024; // 352 MiB
        assert_eq!(
            calculate_disksize_bytes(memtotal, 100, 4096).unwrap(),
            memtotal
        );
        assert_eq!(
            calculate_disksize_bytes(memtotal, 50, 4096).unwrap(),
            176 * 1024 * 1024
        );
        // Unaligned byte count should round down to page boundary
        assert_eq!(
            calculate_disksize_bytes(200_000, 33, 4096).unwrap(),
            4096 * 16 // 66_000 & !4095 == 65_536
        );
    }

    #[test]
    fn test_calculate_disksize_bytes_bounds() {
        assert!(calculate_disksize_bytes(352 * 1024 * 1024, 0, 4096).is_err());
        assert!(calculate_disksize_bytes(32 * 1024, 100, 4096).is_err()); // < 10 pages
        assert!(calculate_disksize_bytes(352 * 1024 * 1024, 100, 3000).is_err());
    }

    #[test]
    fn test_write_swap_header() {
        let page_size = 4096usize;
        let disksize = (64 * page_size) as u64;
        let tmp = NamedTempFile::new().unwrap();
        tmp.as_file().set_len(disksize).unwrap();

        write_swap_header(tmp.path(), disksize, page_size).unwrap();

        let data = fs::read(tmp.path()).unwrap();
        assert_eq!(&data[page_size - 10..page_size], b"SWAPSPACE2");
        let version = u32::from_ne_bytes(data[1024..1028].try_into().unwrap());
        let last_page = u32::from_ne_bytes(data[1028..1032].try_into().unwrap());
        let nr_badpages = u32::from_ne_bytes(data[1032..1036].try_into().unwrap());
        assert_eq!(version, 1);
        assert_eq!(last_page, 63);
        assert_eq!(nr_badpages, 0);
    }
}
