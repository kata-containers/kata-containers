// Copyright 2020 Ant Group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

//! Nydus error events and performance related metrics.
//!
//! There are several types of metrics supported:
//! - Global error events of type [`ErrorHolder`]
//! - Storage backend metrics of type ['BackendMetrics']
//! - Blobcache metrics of type ['BlobcacheMetrics']
//! - Filesystem metrics of type ['FsIoStats`], supported by Rafs in fuse/virtiofs only.

use std::collections::{HashMap, HashSet};
use std::ops::{Deref, Drop};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime};

use serde_json::Error as SerdeError;

use nydus_error::logger::ErrorHolder;

use crate::InodeBitmap;

/// Type of `inode`.
pub type Inode = u64;

/// Type of file operation statistics counter.
#[derive(PartialEq, Copy, Clone)]
pub enum StatsFop {
    Getattr,
    Readlink,
    Open,
    Release,
    Read,
    Statfs,
    Getxattr,
    Listxattr,
    Opendir,
    Lookup,
    Readdir,
    Readdirplus,
    Access,
    Forget,
    BatchForget,
    Max,
}

/// Errors related to IoStats.
#[derive(Debug)]
pub enum IoStatsError {
    /// Non-exist counter.
    NoCounter,
    /// Failed to serialize message.
    Serialize(SerdeError),
}

type IoStatsResult<T> = Result<T, IoStatsError>;

// Block size separated counters.
// [0-3]: <1K;1K~;4K~;16K~;
// [5-7]: 64K~;128K~;512K~;1M~
const BLOCK_READ_SIZES_MAX: usize = 8;

#[inline]
fn request_size_index(size: usize) -> usize {
    let ceil = (size >> 10).leading_zeros();
    let shift = (std::cmp::max(ceil, 53) - 53) << 2;

    (0x0112_2334_5567u64 >> shift) as usize & 0xf
}

// <=1ms, <=20ms, <=50ms, <=100ms, <=500ms, <=1s, <=2s, >2s
const READ_LATENCY_RANGE_MAX: usize = 8;

fn latency_millis_range_index(elapsed: u64) -> usize {
    match elapsed {
        _ if elapsed <= 1 => 0,
        _ if elapsed <= 20 => 1,
        _ if elapsed <= 50 => 2,
        _ if elapsed <= 100 => 3,
        _ if elapsed <= 500 => 4,
        _ if elapsed <= 1000 => 5,
        _ if elapsed <= 2000 => 6,
        _ => 7,
    }
}

// <=200us, <=1ms, <=20ms, <=50ms, <=500ms, <=1s, <=2s, >2s
fn latency_micros_range_index(elapsed: u64) -> usize {
    match elapsed {
        _ if elapsed <= 200 => 0,
        _ if elapsed <= 1_000 => 1,
        _ if elapsed <= 20_000 => 2,
        _ if elapsed <= 50_000 => 3,
        _ if elapsed <= 500_000 => 4,
        _ if elapsed <= 1_000_000 => 5,
        _ if elapsed <= 2_000_000 => 6,
        _ => 7,
    }
}

// Defining below global static metrics set so that a specific metrics counter can
// be found as per the rafs backend mountpoint/id. Remind that nydusd can have
// multiple backends mounted.
lazy_static! {
    static ref FS_METRICS: RwLock<HashMap<String, Arc<FsIoStats>>> = Default::default();
}

lazy_static! {
    static ref BACKEND_METRICS: RwLock<HashMap<String, Arc<BackendMetrics>>> = Default::default();
}

lazy_static! {
    static ref BLOBCACHE_METRICS: RwLock<HashMap<String, Arc<BlobcacheMetrics>>> =
        Default::default();
}

lazy_static! {
    pub static ref ERROR_HOLDER: Arc<Mutex<ErrorHolder>> =
        Arc::new(Mutex::new(ErrorHolder::new(500, 50 * 1024)));
}

/// Trait to manipulate per inode statistics metrics.
pub trait InodeStatsCounter {
    fn stats_fop_inc(&self, fop: StatsFop);
    fn stats_fop_err_inc(&self, fop: StatsFop);
    fn stats_cumulative(&self, fop: StatsFop, value: usize);
}

/// Per inode io statistics metrics.
#[derive(Default, Debug, Serialize)]
pub struct InodeIoStats {
    total_fops: BasicMetric,
    data_read: BasicMetric,
    // Cumulative bytes for different block size.
    block_count_read: [BasicMetric; BLOCK_READ_SIZES_MAX],
    fop_hits: [BasicMetric; StatsFop::Max as usize],
    fop_errors: [BasicMetric; StatsFop::Max as usize],
}

impl InodeStatsCounter for InodeIoStats {
    fn stats_fop_inc(&self, fop: StatsFop) {
        self.fop_hits[fop as usize].inc();
        self.total_fops.inc();
    }

    fn stats_fop_err_inc(&self, fop: StatsFop) {
        self.fop_errors[fop as usize].inc();
    }

    fn stats_cumulative(&self, fop: StatsFop, value: usize) {
        if fop == StatsFop::Read {
            self.data_read.add(value as u64);
            // Put counters into $BLOCK_READ_COUNT_MAX catagories
            // 1K; 4K; 16K; 64K, 128K, 512K, 1M
            let idx = request_size_index(value);
            self.block_count_read[idx].inc();
        }
    }
}

/// Records how a file is accessed.
/// For security sake, each file can associate an access pattern recorder, which
/// is globally configured through nydusd configuration file.
/// For now, the pattern is composed of:
///     1. How many times a file is read regardless of io block size and request offset.
///        And this counter can not be cleared.
///     2. First time point at which this file is read. It's wall-time in unit of seconds.
///     3. File path relative to current rafs root.
///
/// Yes, we now don't have an abundant pattern recorder now. It can be negotiated in the
/// future about how to enrich it.
///
#[derive(Default, Debug, Serialize)]
pub struct AccessPattern {
    ino: u64,
    nr_read: BasicMetric,
    /// In unit of seconds.
    first_access_time_secs: AtomicU64,
    first_access_time_nanos: AtomicU32,
}

impl AccessPattern {
    fn record_access_time(&self) {
        if self.first_access_time_secs.load(Ordering::Relaxed) == 0 {
            let t = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap();
            self.first_access_time_secs
                .store(t.as_secs(), Ordering::Relaxed);
            self.first_access_time_nanos
                .store(t.subsec_nanos(), Ordering::Relaxed);
        }
    }
}

/// Filesystem level statistics and metrics.
///
/// Currently only Rafs in Fuse/Virtiofs mode supports filesystem level statistics and metrics.
#[derive(Default, Debug, Serialize)]
pub struct FsIoStats {
    // Whether to enable each file accounting switch.
    // As fop accounting might consume much memory space, it is disabled by default.
    // But global fop accounting is always working within each Rafs.
    files_account_enabled: AtomicBool,
    access_pattern_enabled: AtomicBool,
    record_latest_read_files_enabled: AtomicBool,
    // Flag to enable record operation latency.
    measure_latency: AtomicBool,

    id: String,
    // Total number of files that are currently open.
    nr_opens: BasicMetric,
    // Total bytes read against the filesystem.
    data_read: BasicMetric,
    // Cumulative bytes for different block size.
    block_count_read: [BasicMetric; BLOCK_READ_SIZES_MAX],
    // Counters for successful various file operations.
    fop_hits: [BasicMetric; StatsFop::Max as usize],
    // Counters for failed file operations.
    fop_errors: [BasicMetric; StatsFop::Max as usize],

    // Cumulative latency's life cycle is equivalent to Rafs, unlike incremental
    // latency which will be cleared each time dumped. Unit as micro-seconds.
    //   * @total means io_stats simply adds every fop latency to the counter which is never cleared.
    //     It is useful for other tools to calculate their metrics report.
    fop_cumulative_latency_total: [BasicMetric; StatsFop::Max as usize],
    // Record how many times read latency drops to the ranges.
    // This helps us to understand the io service time stability.
    read_latency_dist: [BasicMetric; READ_LATENCY_RANGE_MAX],

    // Rwlock closes the race that more than one threads are creating counters concurrently.
    #[serde(skip_serializing, skip_deserializing)]
    file_counters: RwLock<HashMap<Inode, Arc<InodeIoStats>>>,
    #[serde(skip_serializing, skip_deserializing)]
    access_patterns: RwLock<HashMap<Inode, Arc<AccessPattern>>>,
    // record regular file read
    #[serde(skip_serializing, skip_deserializing)]
    recent_read_files: InodeBitmap,
}

macro_rules! impl_iostat_option {
    ($get:ident, $set:ident, $opt:ident) => {
        #[inline]
        fn $get(&self) -> bool {
            self.$opt.load(Ordering::Relaxed)
        }

        #[inline]
        pub fn $set(&self, switch: bool) {
            self.$opt.store(switch, Ordering::Relaxed)
        }
    };
}

impl FsIoStats {
    /// Create a new instance of [`FsIoStats`] for filesystem `id`.
    pub fn new(id: &str) -> Arc<FsIoStats> {
        let c = Arc::new(FsIoStats {
            id: id.to_string(),
            ..Default::default()
        });
        FS_METRICS
            .write()
            .unwrap()
            .insert(id.to_string(), c.clone());
        c.init();
        c
    }

    /// Initialize the [`FsIoStats`] object.
    pub fn init(&self) {
        self.files_account_enabled.store(false, Ordering::Relaxed);
        self.measure_latency.store(true, Ordering::Relaxed);
    }

    impl_iostat_option!(files_enabled, toggle_files_recording, files_account_enabled);
    impl_iostat_option!(
        access_pattern_enabled,
        toggle_access_pattern,
        access_pattern_enabled
    );
    impl_iostat_option!(
        record_latest_read_files_enabled,
        toggle_latest_read_files_recording,
        record_latest_read_files_enabled
    );

    /// Prepare for recording statistics information about `ino`.
    pub fn new_file_counter(&self, ino: Inode) {
        if self.files_enabled() {
            let mut counters = self.file_counters.write().unwrap();
            if counters.get(&ino).is_none() {
                counters.insert(ino, Arc::new(InodeIoStats::default()));
            }
        }

        if self.access_pattern_enabled() {
            let mut records = self.access_patterns.write().unwrap();
            if records.get(&ino).is_none() {
                records.insert(
                    ino,
                    Arc::new(AccessPattern {
                        ino,
                        ..Default::default()
                    }),
                );
            }
        }
    }

    fn file_stats_update(&self, ino: Inode, fop: StatsFop, bsize: usize, success: bool) {
        self.fop_update(fop, bsize, success);

        if self.files_enabled() {
            let counters = self.file_counters.read().unwrap();
            match counters.get(&ino) {
                Some(c) => {
                    c.stats_fop_inc(fop);
                    c.stats_cumulative(fop, bsize);
                }
                None => warn!("No iostats counter for file {}", ino),
            }
        }

        if self.access_pattern_enabled() && fop == StatsFop::Read {
            let records = self.access_patterns.read().unwrap();
            match records.get(&ino) {
                Some(r) => {
                    r.nr_read.inc();
                    r.record_access_time();
                }
                None => warn!("No pattern record for file {}", ino),
            }
        }

        if self.record_latest_read_files_enabled() && fop == StatsFop::Read && success {
            self.recent_read_files.set(ino);
        }
    }

    fn fop_update(&self, fop: StatsFop, value: usize, success: bool) {
        // Linux kernel no longer splits IO into sizes smaller than 128K.
        // So 512K and 1M is added.
        // We put block count into 5 catagories e.g. 1K; 4K; 16K; 64K; 128K; 512K; 1M
        if fop == StatsFop::Read {
            let idx = request_size_index(value);
            self.block_count_read[idx].inc()
        }

        if success {
            self.fop_hits[fop as usize].inc();
            match fop {
                StatsFop::Read => self.data_read.add(value as u64),
                StatsFop::Open => self.nr_opens.inc(),
                StatsFop::Release => self.nr_opens.dec(),
                _ => (),
            };
        } else {
            self.fop_errors[fop as usize].inc();
        }
    }

    /// Mark starting of filesystem operation.
    pub fn latency_start(&self) -> Option<SystemTime> {
        if !self.measure_latency.load(Ordering::Relaxed) {
            return None;
        }

        Some(SystemTime::now())
    }

    /// Mark ending of filesystem operation and record statistics.
    pub fn latency_end(&self, start: &Option<SystemTime>, fop: StatsFop) {
        if let Some(start) = start {
            if let Ok(d) = SystemTime::elapsed(start) {
                let elapsed = saturating_duration_micros(&d);
                self.read_latency_dist[latency_micros_range_index(elapsed)].inc();
                self.fop_cumulative_latency_total[fop as usize].add(elapsed);
            }
        }
    }

    fn export_files_stats(&self) -> Result<String, IoStatsError> {
        serde_json::to_string(
            self.file_counters
                .read()
                .expect("Not expect poisoned lock")
                .deref(),
        )
        .map_err(IoStatsError::Serialize)
    }

    fn export_latest_read_files(&self) -> String {
        serde_json::json!(self.recent_read_files.bitmap_to_array_and_clear()).to_string()
    }

    fn export_files_access_patterns(&self) -> Result<String, IoStatsError> {
        serde_json::to_string(
            &self
                .access_patterns
                .read()
                .expect("Not poisoned lock")
                .deref()
                .values()
                .filter(|r| r.nr_read.count() != 0)
                .collect::<Vec<&Arc<AccessPattern>>>(),
        )
        .map_err(IoStatsError::Serialize)
    }

    fn export_fs_stats(&self) -> Result<String, IoStatsError> {
        serde_json::to_string(self).map_err(IoStatsError::Serialize)
    }
}

/// Guard object to record file operation metrics associated with an inode.
///
/// Call its `settle()` method to generate an on-stack recorder.
/// If the operation succeeds, call `mark_success()` to change the recorder's internal state.
/// If the operation fails, its internal state will not be changed.
/// Finally, when the recorder is being destroyed, iostats counter will be updated.
pub struct FopRecorder<'a> {
    fop: StatsFop,
    inode: u64,
    success: bool,
    // Now, the size only makes sense for `Read` FOP.
    size: usize,
    ios: &'a FsIoStats,
}

impl<'a> Drop for FopRecorder<'a> {
    fn drop(&mut self) {
        self.ios
            .file_stats_update(self.inode, self.fop, self.size, self.success);
    }
}

impl<'a> FopRecorder<'a> {
    /// Create a guard object for filesystem operation `fop` associated with `inode`.
    pub fn settle<T>(fop: StatsFop, inode: u64, ios: &'a T) -> Self
    where
        T: AsRef<FsIoStats>,
    {
        FopRecorder {
            fop,
            inode,
            success: false,
            size: 0,
            ios: ios.as_ref(),
        }
    }

    /// Mark operation as success.
    pub fn mark_success(&mut self, size: usize) {
        self.success = true;
        self.size = size;
    }
}

/// Export file metrics of a filesystem.
pub fn export_files_stats(
    name: &Option<String>,
    latest_read_files: bool,
) -> Result<String, IoStatsError> {
    let fs_metrics = FS_METRICS.read().unwrap();

    match name {
        Some(k) => fs_metrics.get(k).ok_or(IoStatsError::NoCounter).map(|v| {
            if !latest_read_files {
                v.export_files_stats()
            } else {
                Ok(v.export_latest_read_files())
            }
        })?,
        None => {
            if fs_metrics.len() == 1 {
                if let Some(ios) = fs_metrics.values().next() {
                    return if !latest_read_files {
                        ios.export_files_stats()
                    } else {
                        Ok(ios.export_latest_read_files())
                    };
                }
            }
            Err(IoStatsError::NoCounter)
        }
    }
}

/// Export file access pattern of a filesystem.
pub fn export_files_access_pattern(name: &Option<String>) -> Result<String, IoStatsError> {
    let fs_metrics = FS_METRICS.read().unwrap();
    match name {
        Some(k) => fs_metrics
            .get(k)
            .ok_or(IoStatsError::NoCounter)
            .map(|v| v.export_files_access_patterns())?,
        None => {
            if fs_metrics.len() == 1 {
                if let Some(ios) = fs_metrics.values().next() {
                    return ios.export_files_access_patterns();
                }
            }
            Err(IoStatsError::NoCounter)
        }
    }
}

/// Export filesystem metrics.
pub fn export_global_stats(name: &Option<String>) -> Result<String, IoStatsError> {
    // With only one rafs instance, we allow caller to ask for an unknown ios name.
    let fs_metrics = FS_METRICS.read().unwrap();

    match name {
        Some(k) => fs_metrics
            .get(k)
            .ok_or(IoStatsError::NoCounter)
            .map(|v| v.export_fs_stats())?,
        None => {
            if fs_metrics.len() == 1 {
                if let Some(ios) = fs_metrics.values().next() {
                    return ios.export_fs_stats();
                }
            }
            Err(IoStatsError::NoCounter)
        }
    }
}

/// Export storage backend metrics.
pub fn export_backend_metrics(name: &Option<String>) -> IoStatsResult<String> {
    let metrics = BACKEND_METRICS.read().unwrap();

    match name {
        Some(k) => metrics
            .get(k)
            .ok_or(IoStatsError::NoCounter)
            .map(|v| v.export_metrics())?,
        None => {
            if metrics.len() == 1 {
                if let Some(m) = metrics.values().next() {
                    return m.export_metrics();
                }
            }
            Err(IoStatsError::NoCounter)
        }
    }
}

/// Export blob cache metircs.
pub fn export_blobcache_metrics(id: &Option<String>) -> IoStatsResult<String> {
    let metrics = BLOBCACHE_METRICS.read().unwrap();

    match id {
        Some(k) => metrics
            .get(k)
            .ok_or(IoStatsError::NoCounter)
            .map(|v| v.export_metrics())?,
        None => {
            if metrics.len() == 1 {
                if let Some(m) = metrics.values().next() {
                    return m.export_metrics();
                }
            }
            Err(IoStatsError::NoCounter)
        }
    }
}

/// Export global error events.
pub fn export_events() -> IoStatsResult<String> {
    serde_json::to_string(ERROR_HOLDER.lock().unwrap().deref()).map_err(IoStatsError::Serialize)
}

/// Trait to manipulate metric counters.
pub trait Metric {
    /// Adds `value` to the current counter.
    fn add(&self, value: u64);
    /// Increments by 1 unit the current counter.
    fn inc(&self) {
        self.add(1);
    }
    /// Returns current value of the counter.
    fn count(&self) -> u64;
    /// Subtract `value` from the current counter.
    fn sub(&self, value: u64);
    /// Decrease the current counter.
    fn dec(&self) {
        self.sub(1);
    }
}

/// Basic 64-bit metric counter.
#[derive(Default, Serialize, Debug)]
pub struct BasicMetric(AtomicU64);

impl Metric for BasicMetric {
    fn add(&self, value: u64) {
        self.0.fetch_add(value, Ordering::Relaxed);
    }

    fn count(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }

    fn sub(&self, value: u64) {
        self.0.fetch_sub(value, Ordering::Relaxed);
    }
}

/// Metrics for storage backends.
#[derive(Default, Serialize, Debug)]
pub struct BackendMetrics {
    #[serde(skip_serializing, skip_deserializing)]
    id: String,
    // type of storage backend.
    backend_type: String,
    // Cumulative count of read request to backend
    read_count: BasicMetric,
    // Cumulative count of read failure to backend
    read_errors: BasicMetric,
    // Cumulative amount of data from to backend in unit of Byte. External tools
    // are responsible for calculating BPS from this field.
    read_amount_total: BasicMetric,
    // In unit of millisecond
    read_cumulative_latency_millis_total: BasicMetric,
    read_cumulative_latency_millis_dist: [BasicMetric; BLOCK_READ_SIZES_MAX],
    read_count_block_size_dist: [BasicMetric; BLOCK_READ_SIZES_MAX],
    // Categorize metrics as per their latency and request size
    read_latency_sizes_dist: [[BasicMetric; READ_LATENCY_RANGE_MAX]; BLOCK_READ_SIZES_MAX],
}

impl BackendMetrics {
    /// Create a [`BackendMetrics`] object for a storage backend.
    pub fn new(id: &str, backend_type: &str) -> Arc<Self> {
        let backend_metrics = Arc::new(Self {
            id: id.to_string(),
            backend_type: backend_type.to_string(),
            ..Default::default()
        });

        BACKEND_METRICS
            .write()
            .unwrap()
            .insert(id.to_string(), backend_metrics.clone());

        backend_metrics
    }

    /// Release a [`BackendMetrics`] object for a storage backend.
    pub fn release(&self) -> IoStatsResult<()> {
        BACKEND_METRICS
            .write()
            .unwrap()
            .remove(&self.id)
            .map(|_| ())
            .ok_or(IoStatsError::NoCounter)
    }

    /// Mark starting of an IO operations.
    pub fn begin(&self) -> SystemTime {
        SystemTime::now()
    }

    /// Mark ending of an IO operations.
    pub fn end(&self, begin: &SystemTime, size: usize, error: bool) {
        if let Ok(d) = SystemTime::elapsed(begin) {
            let elapsed = saturating_duration_millis(&d);

            self.read_count.inc();
            if error {
                self.read_errors.inc();
            }

            self.read_cumulative_latency_millis_total.add(elapsed);
            self.read_amount_total.add(size as u64);
            let lat_idx = latency_millis_range_index(elapsed);
            let size_idx = request_size_index(size);
            self.read_cumulative_latency_millis_dist[size_idx].add(elapsed);
            self.read_count_block_size_dist[size_idx].inc();
            self.read_latency_sizes_dist[size_idx][lat_idx].inc();
        }
    }

    fn export_metrics(&self) -> IoStatsResult<String> {
        serde_json::to_string(self).map_err(IoStatsError::Serialize)
    }
}

// This function assumes that the counted duration won't be too long.
fn saturating_duration_millis(d: &Duration) -> u64 {
    let d_secs = d.as_secs();
    if d_secs == 0 {
        d.subsec_millis() as u64
    } else {
        d_secs
            .saturating_mul(1000)
            .saturating_add(d.subsec_millis() as u64)
    }
}

fn saturating_duration_micros(d: &Duration) -> u64 {
    let d_secs = d.as_secs();
    if d_secs == 0 {
        d.subsec_micros() as u64
    } else {
        d_secs
            .saturating_mul(1_000_000)
            .saturating_add(d.subsec_micros() as u64)
    }
}

#[derive(Debug, Default, Serialize)]
pub struct BlobcacheMetrics {
    #[serde(skip_serializing, skip_deserializing)]
    id: String,
    // Prefer to let external tool get file's state like file size and disk usage.
    // Because stat(2) file may get blocked.
    pub underlying_files: Mutex<HashSet<String>>,
    pub store_path: String,
    // Cache hit percentage = (partial_hits + whole_hits) / total
    pub partial_hits: BasicMetric,
    pub whole_hits: BasicMetric,
    // How many `read` requests are processed by the blobcache instance.
    // This metric will be helpful when comparing with cache hits times.
    pub total: BasicMetric,
    // Scale of blobcache. Blobcache does not evict entries.
    // Means the number of chunks in ready status.
    pub entries_count: BasicMetric,
    // Together with below two fields, we can figure out average merging size thus
    // to estimate the possibility to merge backend IOs.
    // In unit of Bytes
    pub prefetch_data_amount: BasicMetric,
    pub prefetch_mr_count: BasicMetric,
    pub prefetch_workers: AtomicUsize,
    pub prefetch_unmerged_chunks: BasicMetric,
    pub buffered_backend_size: BasicMetric,
    pub data_all_ready: AtomicBool,
}

impl BlobcacheMetrics {
    /// Create a [`BlobcacheMetrics`] object for a blob cache manager.
    pub fn new(id: &str, store_path: &str) -> Arc<Self> {
        let metrics = Arc::new(Self {
            id: id.to_string(),
            store_path: store_path.to_string(),
            ..Default::default()
        });

        // Old metrics will be dropped when BlobCache is swapped. So we don't
        // have to worry about swapping its metrics either which means it's
        // not necessary to release metrics recorder when blobcache is dropped due to swapping.
        BLOBCACHE_METRICS
            .write()
            .unwrap()
            .insert(id.to_string(), metrics.clone());

        metrics
    }

    /// Release a [`BlobcacheMetrics`] object for a blob cache manager.
    pub fn release(&self) -> IoStatsResult<()> {
        BLOBCACHE_METRICS
            .write()
            .unwrap()
            .remove(&self.id)
            .map(|_| ())
            .ok_or(IoStatsError::NoCounter)
    }

    /// Export blobcache metric information.
    pub fn export_metrics(&self) -> IoStatsResult<String> {
        serde_json::to_string(self).map_err(IoStatsError::Serialize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_size_index() {
        assert_eq!(request_size_index(0x0), 0);
        assert_eq!(request_size_index(0x3ff), 0);
        assert_eq!(request_size_index(0x400), 1);
        assert_eq!(request_size_index(0xfff), 1);
        assert_eq!(request_size_index(0x1000), 2);
        assert_eq!(request_size_index(0x3fff), 2);
        assert_eq!(request_size_index(0x4000), 3);
        assert_eq!(request_size_index(0xffff), 3);
        assert_eq!(request_size_index(0x1_0000), 4);
        assert_eq!(request_size_index(0x1_ffff), 4);
        assert_eq!(request_size_index(0x2_0000), 5);
        assert_eq!(request_size_index(0x7_ffff), 5);
        assert_eq!(request_size_index(0x8_0000), 6);
        assert_eq!(request_size_index(0xf_ffff), 6);
        assert_eq!(request_size_index(0x10_0000), 7);
        assert_eq!(request_size_index(usize::MAX), 7);
    }

    #[test]
    fn test_block_read_count() {
        let g = FsIoStats::default();
        g.init();
        g.fop_update(StatsFop::Read, 4000, true);
        assert_eq!(g.block_count_read[1].count(), 1);

        g.fop_update(StatsFop::Read, 4096, true);
        assert_eq!(g.block_count_read[2].count(), 1);

        g.fop_update(StatsFop::Read, 65535, true);
        assert_eq!(g.block_count_read[3].count(), 1);

        g.fop_update(StatsFop::Read, 131072, true);
        assert_eq!(g.block_count_read[5].count(), 1);

        g.fop_update(StatsFop::Read, 65520, true);
        assert_eq!(g.block_count_read[3].count(), 2);

        g.fop_update(StatsFop::Read, 2015520, true);
        assert_eq!(g.block_count_read[3].count(), 2);
    }
}
