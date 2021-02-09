// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 And Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module contains the implementation of the `blkio` cgroup subsystem.
//!
//! See the Kernel's documentation for more information about this subsystem, found at:
//!  [Documentation/cgroup-v1/blkio-controller.txt](https://www.kernel.org/doc/Documentation/cgroup-v1/blkio-controller.txt)
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::error::ErrorKind::*;
use crate::error::*;

use crate::{
    BlkIoResources, ControllIdentifier, ControllerInternal, Controllers, Resources, Subsystem,
};

/// A controller that allows controlling the `blkio` subsystem of a Cgroup.
///
/// In essence, using the `blkio` controller one can limit and throttle the tasks' usage of block
/// devices in the control group.
#[derive(Debug, Clone)]
pub struct BlkIoController {
    base: PathBuf,
    path: PathBuf,
    v2: bool,
}

#[derive(Eq, PartialEq, Debug)]
/// Per-device information
pub struct BlkIoData {
    /// The major number of the device.
    pub major: i16,
    /// The minor number of the device.
    pub minor: i16,
    /// The data that is associated with the device.
    pub data: u64,
}

#[derive(Eq, PartialEq, Debug)]
/// Per-device activity from the control group.
pub struct IoService {
    /// The major number of the device.
    pub major: i16,
    /// The minor number of the device.
    pub minor: i16,
    /// How many items were read from the device.
    pub read: u64,
    /// How many items were written to the device.
    pub write: u64,
    /// How many items were synchronously transferred.
    pub sync: u64,
    /// How many items were asynchronously transferred.
    pub r#async: u64,
    /// Total number of items transferred.
    pub total: u64,
}

#[derive(Eq, PartialEq, Debug)]
/// Per-device activity from the control group.
/// Only for cgroup v2
pub struct IoStat {
    /// The major number of the device.
    pub major: i16,
    /// The minor number of the device.
    pub minor: i16,
    /// How many bytes were read from the device.
    pub rbytes: u64,
    /// How many bytes were written to the device.
    pub wbytes: u64,
    /// How many iops were read from the device.
    pub rios: u64,
    /// How many iops were written to the device.
    pub wios: u64,
    /// How many discard bytes were read from the device.
    pub dbytes: u64,
    /// How many discard iops were written to the device.
    pub dios: u64,
}

fn parse_io_service(s: String) -> Result<Vec<IoService>> {
    s.lines()
        .filter(|x| x.split_whitespace().collect::<Vec<_>>().len() == 3)
        .map(|x| {
            let mut spl = x.split_whitespace();
            (spl.nth(0).unwrap(), spl.nth(0).unwrap(), spl.nth(0).unwrap())
        })
        .map(|(a, b, c)| {
            let mut spl = a.split(":");
            (spl.nth(0).unwrap(), spl.nth(0).unwrap(), b, c)
        })
        .collect::<Vec<_>>()
        .chunks(5)
        .map(|x| {
            match x {
                [(major, minor, "Read", read_val), (_, _, "Write", write_val),
                   (_, _, "Sync", sync_val), (_, _, "Async", async_val),
                   (_, _, "Total", total_val)] =>
                    Some(IoService {
                        major: major.parse::<i16>().unwrap(),
                        minor: minor.parse::<i16>().unwrap(),
                        read: read_val.parse::<u64>().unwrap(),
                        write: write_val.parse::<u64>().unwrap(),
                        sync: sync_val.parse::<u64>().unwrap(),
                        r#async: async_val.parse::<u64>().unwrap(),
                        total: total_val.parse::<u64>().unwrap(),
                    }),
               _ => None,
            }
        })
        .fold(Ok(Vec::new()), |acc, x| {
            if acc.is_err() || x.is_none() {
                Err(Error::new(ParseError))
            } else {
                let mut acc = acc.unwrap();
                acc.push(x.unwrap());
                Ok(acc)
            }
        })
}

fn get_value(s: &str) -> String {
    let arr = s.split(':').collect::<Vec<&str>>();
    if arr.len() != 2 {
        return "0".to_string();
    }
    arr[1].to_string()
}

fn parse_io_stat(s: String) -> Result<Vec<IoStat>> {
    // line:
    // 8:0 rbytes=180224 wbytes=0 rios=3 wios=0 dbytes=0 dios=0
    let v = s
        .lines()
        .filter(|x| x.split_whitespace().collect::<Vec<_>>().len() == 7)
        .map(|x| {
            let arr = x.split_whitespace().collect::<Vec<&str>>();
            let device = arr[0].split(":").collect::<Vec<&str>>();
            let (major, minor) = (device[0], device[1]);

            IoStat {
                major: major.parse::<i16>().unwrap(),
                minor: minor.parse::<i16>().unwrap(),
                rbytes: get_value(arr[1]).parse::<u64>().unwrap(),
                wbytes: get_value(arr[2]).parse::<u64>().unwrap(),
                rios: get_value(arr[3]).parse::<u64>().unwrap(),
                wios: get_value(arr[4]).parse::<u64>().unwrap(),
                dbytes: get_value(arr[5]).parse::<u64>().unwrap(),
                dios: get_value(arr[6]).parse::<u64>().unwrap(),
            }
        })
        .collect::<Vec<IoStat>>();

    Ok(v)
}

fn parse_io_service_total(s: String) -> Result<u64> {
    s.lines()
        .filter(|x| x.split_whitespace().collect::<Vec<_>>().len() == 2)
        .fold(Err(Error::new(ParseError)), |_, x| {
            match x.split_whitespace().collect::<Vec<_>>().as_slice() {
                ["Total", val] => val.parse::<u64>().map_err(|_| Error::new(ParseError)),
                _ => Err(Error::new(ParseError)),
            }
        })
}

fn parse_blkio_data(s: String) -> Result<Vec<BlkIoData>> {
    let r = s
        .chars()
        .map(|x| if x == ':' { ' ' } else { x })
        .collect::<String>();

    let r = r
        .lines()
        .flat_map(|x| x.split_whitespace())
        .collect::<Vec<_>>();

    let r = r.chunks(3).collect::<Vec<_>>();

    let mut res = Vec::new();

    let err = r.iter().try_for_each(|x| match x {
        [major, minor, data] => {
            res.push(BlkIoData {
                major: major.parse::<i16>().unwrap(),
                minor: minor.parse::<i16>().unwrap(),
                data: data.parse::<u64>().unwrap(),
            });
            Ok(())
        }
        _ => Err(Error::new(ParseError)),
    });

    if err.is_err() {
        return Err(Error::new(ParseError));
    } else {
        return Ok(res);
    }
}

/// Current state and statistics about how throttled are the block devices when accessed from the
/// controller's control group.
#[derive(Default, Debug)]
pub struct BlkIoThrottle {
    /// Statistics about the bytes transferred between the block devices by the tasks in this
    /// control group.
    pub io_service_bytes: Vec<IoService>,
    /// Total amount of bytes transferred to and from the block devices.
    pub io_service_bytes_total: u64,
    /// Same as `io_service_bytes`, but contains all descendant control groups.
    pub io_service_bytes_recursive: Vec<IoService>,
    /// Total amount of bytes transferred to and from the block devices, including all descendant
    /// control groups.
    pub io_service_bytes_recursive_total: u64,
    /// The number of I/O operations performed on the devices as seen by the throttling policy.
    pub io_serviced: Vec<IoService>,
    /// The total number of I/O operations performed on the devices as seen by the throttling
    /// policy.
    pub io_serviced_total: u64,
    /// Same as `io_serviced`, but contains all descendant control groups.
    pub io_serviced_recursive: Vec<IoService>,
    /// Same as `io_serviced`, but contains all descendant control groups and contains only the
    /// total amount.
    pub io_serviced_recursive_total: u64,
    /// The upper limit of bytes per second rate of read operation on the block devices by the
    /// control group's tasks.
    pub read_bps_device: Vec<BlkIoData>,
    /// The upper limit of I/O operation per second, when said operation is a read operation.
    pub read_iops_device: Vec<BlkIoData>,
    /// The upper limit of bytes per second rate of write operation on the block devices by the
    /// control group's tasks.
    pub write_bps_device: Vec<BlkIoData>,
    /// The upper limit of I/O operation per second, when said operation is a write operation.
    pub write_iops_device: Vec<BlkIoData>,
}

/// Statistics and state of the block devices.
#[derive(Default, Debug)]
pub struct BlkIo {
    /// The number of BIOS requests merged into I/O requests by the control group's tasks.
    pub io_merged: Vec<IoService>,
    /// Same as `io_merged`, but only reports the total number.
    pub io_merged_total: u64,
    /// Same as `io_merged`, but contains all descendant control groups.
    pub io_merged_recursive: Vec<IoService>,
    /// Same as `io_merged_recursive`, but only reports the total number.
    pub io_merged_recursive_total: u64,
    /// The number of requests queued for I/O operations by the tasks of the control group.
    pub io_queued: Vec<IoService>,
    /// Same as `io_queued`, but only reports the total number.
    pub io_queued_total: u64,
    /// Same as `io_queued`, but contains all descendant control groups.
    pub io_queued_recursive: Vec<IoService>,
    /// Same as `io_queued_recursive`, but contains all descendant control groups.
    pub io_queued_recursive_total: u64,
    /// The number of bytes transferred from and to the block device (as seen by the CFQ I/O scheduler).
    pub io_service_bytes: Vec<IoService>,
    /// Same as `io_service_bytes`, but contains all descendant control groups.
    pub io_service_bytes_total: u64,
    /// Same as `io_service_bytes`, but contains all descendant control groups.
    pub io_service_bytes_recursive: Vec<IoService>,
    /// Total amount of bytes transferred between the tasks and block devices, including the
    /// descendant control groups' numbers.
    pub io_service_bytes_recursive_total: u64,
    /// The number of I/O operations (as seen by the CFQ I/O scheduler) between the devices and the
    /// control group's tasks.
    pub io_serviced: Vec<IoService>,
    /// The total number of I/O operations performed on the devices as seen by the throttling
    /// policy.
    pub io_serviced_total: u64,
    /// Same as `io_serviced`, but contains all descendant control groups.
    pub io_serviced_recursive: Vec<IoService>,
    /// Same as `io_serviced`, but contains all descendant control groups and contains only the
    /// total amount.
    pub io_serviced_recursive_total: u64,
    /// The total time spent between dispatch and request completion for I/O requests (as seen by
    /// the CFQ I/O scheduler) by the control group's tasks.
    pub io_service_time: Vec<IoService>,
    /// Same as `io_service_time`, but contains all descendant control groups and contains only the
    /// total amount.
    pub io_service_time_total: u64,
    /// Same as `io_service_time`, but contains all descendant control groups.
    pub io_service_time_recursive: Vec<IoService>,
    /// Same as `io_service_time_recursive`, but contains all descendant control groups and only
    /// the total amount.
    pub io_service_time_recursive_total: u64,
    /// Total amount of time spent waiting for a free slot in the CFQ I/O scheduler's queue.
    pub io_wait_time: Vec<IoService>,
    /// Same as `io_wait_time`, but only reports the total amount.
    pub io_wait_time_total: u64,
    /// Same as `io_wait_time`, but contains all descendant control groups.
    pub io_wait_time_recursive: Vec<IoService>,
    /// Same as `io_wait_time_recursive`, but only reports the total amount.
    pub io_wait_time_recursive_total: u64,
    /// How much weight do the control group's tasks have when competing against the descendant
    /// control group's tasks.
    pub leaf_weight: u64,
    /// Same as `leaf_weight`, but per-block-device.
    pub leaf_weight_device: Vec<BlkIoData>,
    /// Total number of sectors transferred between the block devices and the control group's
    /// tasks.
    pub sectors: Vec<BlkIoData>,
    /// Same as `sectors`, but contains all descendant control groups.
    pub sectors_recursive: Vec<BlkIoData>,
    /// Similar statistics, but as seen by the throttle policy.
    pub throttle: BlkIoThrottle,
    /// The time the control group had access to the I/O devices.
    pub time: Vec<BlkIoData>,
    /// Same as `time`, but contains all descendant control groups.
    pub time_recursive: Vec<BlkIoData>,
    /// The weight of this control group.
    pub weight: u64,
    /// Same as `weight`, but per-block-device.
    pub weight_device: Vec<BlkIoData>,

    /// IoStat for cgroup v2
    pub io_stat: Vec<IoStat>,
}

impl ControllerInternal for BlkIoController {
    fn control_type(&self) -> Controllers {
        Controllers::BlkIo
    }
    fn get_path(&self) -> &PathBuf {
        &self.path
    }
    fn get_path_mut(&mut self) -> &mut PathBuf {
        &mut self.path
    }
    fn get_base(&self) -> &PathBuf {
        &self.base
    }

    fn is_v2(&self) -> bool {
        self.v2
    }

    fn apply(&self, res: &Resources) -> Result<()> {
        // get the resources that apply to this controller
        let res: &BlkIoResources = &res.blkio;

        if res.update_values {
            if let Some(weight) = res.weight {
                let _ = self.set_weight(weight as u64);
            }
            if let Some(leaf_weight) = res.leaf_weight {
                let _ = self.set_leaf_weight(leaf_weight as u64);
            }

            for dev in &res.weight_device {
                if let Some(weight) = dev.weight {
                    let _ = self.set_weight_for_device(dev.major, dev.minor, weight as u64);
                }
                if let Some(leaf_weight) = dev.leaf_weight {
                    let _ =
                        self.set_leaf_weight_for_device(dev.major, dev.minor, leaf_weight as u64);
                }
            }

            for dev in &res.throttle_read_bps_device {
                let _ = self.throttle_read_bps_for_device(dev.major, dev.minor, dev.rate);
            }

            for dev in &res.throttle_write_bps_device {
                let _ = self.throttle_write_bps_for_device(dev.major, dev.minor, dev.rate);
            }

            for dev in &res.throttle_read_iops_device {
                let _ = self.throttle_read_iops_for_device(dev.major, dev.minor, dev.rate);
            }

            for dev in &res.throttle_write_iops_device {
                let _ = self.throttle_write_iops_for_device(dev.major, dev.minor, dev.rate);
            }
        }

        Ok(())
    }
}

impl ControllIdentifier for BlkIoController {
    fn controller_type() -> Controllers {
        Controllers::BlkIo
    }
}

impl<'a> From<&'a Subsystem> for &'a BlkIoController {
    fn from(sub: &'a Subsystem) -> &'a BlkIoController {
        unsafe {
            match sub {
                Subsystem::BlkIo(c) => c,
                _ => {
                    assert_eq!(1, 0);
                    ::std::mem::uninitialized()
                }
            }
        }
    }
}

fn read_string_from(mut file: File) -> Result<String> {
    let mut string = String::new();
    match file.read_to_string(&mut string) {
        Ok(_) => Ok(string.trim().to_string()),
        Err(e) => Err(Error::with_cause(ReadFailed, e)),
    }
}

fn read_u64_from(mut file: File) -> Result<u64> {
    let mut string = String::new();
    match file.read_to_string(&mut string) {
        Ok(_) => string
            .trim()
            .parse()
            .map_err(|e| Error::with_cause(ParseError, e)),
        Err(e) => Err(Error::with_cause(ReadFailed, e)),
    }
}

impl BlkIoController {
    /// Constructs a new `BlkIoController` with `oroot` serving as the root of the control group.
    pub fn new(oroot: PathBuf, v2: bool) -> Self {
        let mut root = oroot;
        if !v2 {
            root.push(Self::controller_type().to_string());
        }
        Self {
            base: root.clone(),
            path: root,
            v2: v2,
        }
    }

    fn blkio_v2(&self) -> BlkIo {
        let mut blkio: BlkIo = Default::default();
        blkio.io_stat = self
            .open_path("io.stat", false)
            .and_then(read_string_from)
            .and_then(parse_io_stat)
            .unwrap_or(Vec::new());

        blkio
    }

    /// Gathers statistics about and reports the state of the block devices used by the control
    /// group's tasks.
    pub fn blkio(&self) -> BlkIo {
        if self.v2 {
            return self.blkio_v2();
        }
        BlkIo {
            io_merged: self
                .open_path("blkio.io_merged", false)
                .and_then(read_string_from)
                .and_then(parse_io_service)
                .unwrap_or(Vec::new()),
            io_merged_total: self
                .open_path("blkio.io_merged", false)
                .and_then(read_string_from)
                .and_then(parse_io_service_total)
                .unwrap_or(0),
            io_merged_recursive: self
                .open_path("blkio.io_merged_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_io_service)
                .unwrap_or(Vec::new()),
            io_merged_recursive_total: self
                .open_path("blkio.io_merged_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_io_service_total)
                .unwrap_or(0),
            io_queued: self
                .open_path("blkio.io_queued", false)
                .and_then(read_string_from)
                .and_then(parse_io_service)
                .unwrap_or(Vec::new()),
            io_queued_total: self
                .open_path("blkio.io_queued", false)
                .and_then(read_string_from)
                .and_then(parse_io_service_total)
                .unwrap_or(0),
            io_queued_recursive: self
                .open_path("blkio.io_queued_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_io_service)
                .unwrap_or(Vec::new()),
            io_queued_recursive_total: self
                .open_path("blkio.io_queued_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_io_service_total)
                .unwrap_or(0),
            io_service_bytes: self
                .open_path("blkio.io_service_bytes", false)
                .and_then(read_string_from)
                .and_then(parse_io_service)
                .unwrap_or(Vec::new()),
            io_service_bytes_total: self
                .open_path("blkio.io_service_bytes", false)
                .and_then(read_string_from)
                .and_then(parse_io_service_total)
                .unwrap_or(0),
            io_service_bytes_recursive: self
                .open_path("blkio.io_service_bytes_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_io_service)
                .unwrap_or(Vec::new()),
            io_service_bytes_recursive_total: self
                .open_path("blkio.io_service_bytes_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_io_service_total)
                .unwrap_or(0),
            io_serviced: self
                .open_path("blkio.io_serviced", false)
                .and_then(read_string_from)
                .and_then(parse_io_service)
                .unwrap_or(Vec::new()),
            io_serviced_total: self
                .open_path("blkio.io_serviced", false)
                .and_then(read_string_from)
                .and_then(parse_io_service_total)
                .unwrap_or(0),
            io_serviced_recursive: self
                .open_path("blkio.io_serviced_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_io_service)
                .unwrap_or(Vec::new()),
            io_serviced_recursive_total: self
                .open_path("blkio.io_serviced_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_io_service_total)
                .unwrap_or(0),
            io_service_time: self
                .open_path("blkio.io_service_time", false)
                .and_then(read_string_from)
                .and_then(parse_io_service)
                .unwrap_or(Vec::new()),
            io_service_time_total: self
                .open_path("blkio.io_service_time", false)
                .and_then(read_string_from)
                .and_then(parse_io_service_total)
                .unwrap_or(0),
            io_service_time_recursive: self
                .open_path("blkio.io_service_time_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_io_service)
                .unwrap_or(Vec::new()),
            io_service_time_recursive_total: self
                .open_path("blkio.io_service_time_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_io_service_total)
                .unwrap_or(0),
            io_wait_time: self
                .open_path("blkio.io_wait_time", false)
                .and_then(read_string_from)
                .and_then(parse_io_service)
                .unwrap_or(Vec::new()),
            io_wait_time_total: self
                .open_path("blkio.io_wait_time", false)
                .and_then(read_string_from)
                .and_then(parse_io_service_total)
                .unwrap_or(0),
            io_wait_time_recursive: self
                .open_path("blkio.io_wait_time_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_io_service)
                .unwrap_or(Vec::new()),
            io_wait_time_recursive_total: self
                .open_path("blkio.io_wait_time_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_io_service_total)
                .unwrap_or(0),
            leaf_weight: self
                .open_path("blkio.leaf_weight", false)
                .and_then(|file| read_u64_from(file))
                .unwrap_or(0u64),
            leaf_weight_device: self
                .open_path("blkio.leaf_weight_device", false)
                .and_then(read_string_from)
                .and_then(parse_blkio_data)
                .unwrap_or(Vec::new()),
            sectors: self
                .open_path("blkio.sectors", false)
                .and_then(read_string_from)
                .and_then(parse_blkio_data)
                .unwrap_or(Vec::new()),
            sectors_recursive: self
                .open_path("blkio.sectors_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_blkio_data)
                .unwrap_or(Vec::new()),
            throttle: BlkIoThrottle {
                io_service_bytes: self
                    .open_path("blkio.throttle.io_service_bytes", false)
                    .and_then(read_string_from)
                    .and_then(parse_io_service)
                    .unwrap_or(Vec::new()),
                io_service_bytes_total: self
                    .open_path("blkio.throttle.io_service_bytes", false)
                    .and_then(read_string_from)
                    .and_then(parse_io_service_total)
                    .unwrap_or(0),
                io_service_bytes_recursive: self
                    .open_path("blkio.throttle.io_service_bytes_recursive", false)
                    .and_then(read_string_from)
                    .and_then(parse_io_service)
                    .unwrap_or(Vec::new()),
                io_service_bytes_recursive_total: self
                    .open_path("blkio.throttle.io_service_bytes_recursive", false)
                    .and_then(read_string_from)
                    .and_then(parse_io_service_total)
                    .unwrap_or(0),
                io_serviced: self
                    .open_path("blkio.throttle.io_serviced", false)
                    .and_then(read_string_from)
                    .and_then(parse_io_service)
                    .unwrap_or(Vec::new()),
                io_serviced_total: self
                    .open_path("blkio.throttle.io_serviced", false)
                    .and_then(read_string_from)
                    .and_then(parse_io_service_total)
                    .unwrap_or(0),
                io_serviced_recursive: self
                    .open_path("blkio.throttle.io_serviced_recursive", false)
                    .and_then(read_string_from)
                    .and_then(parse_io_service)
                    .unwrap_or(Vec::new()),
                io_serviced_recursive_total: self
                    .open_path("blkio.throttle.io_serviced_recursive", false)
                    .and_then(read_string_from)
                    .and_then(parse_io_service_total)
                    .unwrap_or(0),
                read_bps_device: self
                    .open_path("blkio.throttle.read_bps_device", false)
                    .and_then(read_string_from)
                    .and_then(parse_blkio_data)
                    .unwrap_or(Vec::new()),
                read_iops_device: self
                    .open_path("blkio.throttle.read_iops_device", false)
                    .and_then(read_string_from)
                    .and_then(parse_blkio_data)
                    .unwrap_or(Vec::new()),
                write_bps_device: self
                    .open_path("blkio.throttle.write_bps_device", false)
                    .and_then(read_string_from)
                    .and_then(parse_blkio_data)
                    .unwrap_or(Vec::new()),
                write_iops_device: self
                    .open_path("blkio.throttle.write_iops_device", false)
                    .and_then(read_string_from)
                    .and_then(parse_blkio_data)
                    .unwrap_or(Vec::new()),
            },
            time: self
                .open_path("blkio.time", false)
                .and_then(read_string_from)
                .and_then(parse_blkio_data)
                .unwrap_or(Vec::new()),
            time_recursive: self
                .open_path("blkio.time_recursive", false)
                .and_then(read_string_from)
                .and_then(parse_blkio_data)
                .unwrap_or(Vec::new()),
            weight: self
                .open_path("blkio.weight", false)
                .and_then(|file| read_u64_from(file))
                .unwrap_or(0u64),
            weight_device: self
                .open_path("blkio.weight_device", false)
                .and_then(read_string_from)
                .and_then(parse_blkio_data)
                .unwrap_or(Vec::new()),
            io_stat: Vec::new(),
        }
    }

    /// Set the leaf weight on the control group's tasks, i.e., how are they weighted against the
    /// descendant control groups' tasks.
    pub fn set_leaf_weight(&self, w: u64) -> Result<()> {
        self.open_path("blkio.leaf_weight", true)
            .and_then(|mut file| {
                file.write_all(w.to_string().as_ref())
                    .map_err(|e| Error::with_cause(WriteFailed, e))
            })
    }

    /// Same as `set_leaf_weight()`, but settable per each block device.
    pub fn set_leaf_weight_for_device(&self, major: u64, minor: u64, weight: u64) -> Result<()> {
        self.open_path("blkio.leaf_weight_device", true)
            .and_then(|mut file| {
                file.write_all(format!("{}:{} {}", major, minor, weight).as_ref())
                    .map_err(|e| Error::with_cause(WriteFailed, e))
            })
    }

    /// Reset the statistics the kernel has gathered so far and start fresh.
    pub fn reset_stats(&self) -> Result<()> {
        self.open_path("blkio.reset_stats", true)
            .and_then(|mut file| {
                file.write_all("1".to_string().as_ref())
                    .map_err(|e| Error::with_cause(WriteFailed, e))
            })
    }

    /// Throttle the bytes per second rate of read operation affecting the block device
    /// `major:minor` to `bps`.
    pub fn throttle_read_bps_for_device(&self, major: u64, minor: u64, bps: u64) -> Result<()> {
        let mut file = "blkio.throttle.read_bps_device";
        let mut content = format!("{}:{} {}", major, minor, bps);
        if self.v2 {
            file = "io.max";
            content = format!("{}:{} rbps={}", major, minor, bps);
        }
        self.open_path(file, true).and_then(|mut file| {
            file.write_all(content.as_ref())
                .map_err(|e| Error::with_cause(WriteFailed, e))
        })
    }

    /// Throttle the I/O operations per second rate of read operation affecting the block device
    /// `major:minor` to `bps`.
    pub fn throttle_read_iops_for_device(&self, major: u64, minor: u64, iops: u64) -> Result<()> {
        let mut file = "blkio.throttle.read_iops_device";
        let mut content = format!("{}:{} {}", major, minor, iops);
        if self.v2 {
            file = "io.max";
            content = format!("{}:{} riops={}", major, minor, iops);
        }
        self.open_path(file, true).and_then(|mut file| {
            file.write_all(content.as_ref())
                .map_err(|e| Error::with_cause(WriteFailed, e))
        })
    }
    /// Throttle the bytes per second rate of write operation affecting the block device
    /// `major:minor` to `bps`.
    pub fn throttle_write_bps_for_device(&self, major: u64, minor: u64, bps: u64) -> Result<()> {
        let mut file = "blkio.throttle.write_bps_device";
        let mut content = format!("{}:{} {}", major, minor, bps);
        if self.v2 {
            file = "io.max";
            content = format!("{}:{} wbps={}", major, minor, bps);
        }
        self.open_path(file, true).and_then(|mut file| {
            file.write_all(content.as_ref())
                .map_err(|e| Error::with_cause(WriteFailed, e))
        })
    }

    /// Throttle the I/O operations per second rate of write operation affecting the block device
    /// `major:minor` to `bps`.
    pub fn throttle_write_iops_for_device(&self, major: u64, minor: u64, iops: u64) -> Result<()> {
        let mut file = "blkio.throttle.write_iops_device";
        let mut content = format!("{}:{} {}", major, minor, iops);
        if self.v2 {
            file = "io.max";
            content = format!("{}:{} wiops={}", major, minor, iops);
        }
        self.open_path(file, true).and_then(|mut file| {
            file.write_all(content.as_ref())
                .map_err(|e| Error::with_cause(WriteFailed, e))
        })
    }

    /// Set the weight of the control group's tasks.
    pub fn set_weight(&self, w: u64) -> Result<()> {
        // Attation: may not find in high kernel version.
        let mut file = "blkio.weight";
        if self.v2 {
            file = "io.bfq.weight";
        }
        self.open_path(file, true).and_then(|mut file| {
            file.write_all(w.to_string().as_ref())
                .map_err(|e| Error::with_cause(WriteFailed, e))
        })
    }

    /// Same as `set_weight()`, but settable per each block device.
    pub fn set_weight_for_device(&self, major: u64, minor: u64, weight: u64) -> Result<()> {
        let mut file = "blkio.weight_device";
        if self.v2 {
            // Attation: there is no weight for device in runc
            // https://github.com/opencontainers/runc/blob/46be7b612e2533c494e6a251111de46d8e286ed5/libcontainer/cgroups/fs2/io.go#L30
            // may depends on IO schedulers https://wiki.ubuntu.com/Kernel/Reference/IOSchedulers
            file = "io.bfq.weight";
        }
        self.open_path(file, true).and_then(|mut file| {
            file.write_all(format!("{}:{} {}", major, minor, weight).as_ref())
                .map_err(|e| Error::with_cause(WriteFailed, e))
        })
    }
}

#[cfg(test)]
mod test {
    use crate::blkio::{parse_blkio_data, BlkIoData};
    use crate::blkio::{parse_io_service, parse_io_service_total, IoService};
    use crate::error::*;

    static TEST_VALUE: &str = "\
8:32 Read 4280320
8:32 Write 0
8:32 Sync 4280320
8:32 Async 0
8:32 Total 4280320
8:48 Read 5705479168
8:48 Write 56096055296
8:48 Sync 11213923328
8:48 Async 50587611136
8:48 Total 61801534464
8:16 Read 10059776
8:16 Write 0
8:16 Sync 10059776
8:16 Async 0
8:16 Total 10059776
8:0 Read 7192576
8:0 Write 0
8:0 Sync 7192576
8:0 Async 0
8:0 Total 7192576
Total 61823067136
 ";

    static TEST_WRONG_VALUE: &str = "\
8:32 Read 4280320
8:32 Write 0
8:32 Async 0
8:32 Total 4280320 8:48 Read 5705479168
8:48 Write 56096055296
8:48 Sync 11213923328
8:48 Async 50587611136
8:48 Total 61801534464
8:16 Read 10059776
8:16 Write 0
8:16 Sync 10059776
8:16 Async 0
8:16 Total 10059776
8:0 Read 7192576
8:0 Write 0
8:0 Sync 7192576
8:0 Async 0
8:0 Total 7192576
Total 61823067136
 ";

    static TEST_BLKIO_DATA: &str = "\
8:48 454480833999
8:32 228392923193
8:16 772456885
8:0 559583764
 ";

    #[test]
    fn test_parse_io_service_total() {
        let ok = parse_io_service_total(TEST_VALUE.to_string()).unwrap();
        assert_eq!(ok, 61823067136);
    }

    #[test]
    fn test_parse_io_service() {
        let ok = parse_io_service(TEST_VALUE.to_string()).unwrap();
        assert_eq!(
            ok,
            vec![
                IoService {
                    major: 8,
                    minor: 32,
                    read: 4280320,
                    write: 0,
                    sync: 4280320,
                    r#async: 0,
                    total: 4280320,
                },
                IoService {
                    major: 8,
                    minor: 48,
                    read: 5705479168,
                    write: 56096055296,
                    sync: 11213923328,
                    r#async: 50587611136,
                    total: 61801534464,
                },
                IoService {
                    major: 8,
                    minor: 16,
                    read: 10059776,
                    write: 0,
                    sync: 10059776,
                    r#async: 0,
                    total: 10059776,
                },
                IoService {
                    major: 8,
                    minor: 0,
                    read: 7192576,
                    write: 0,
                    sync: 7192576,
                    r#async: 0,
                    total: 7192576,
                }
            ]
        );
        let err = parse_io_service(TEST_WRONG_VALUE.to_string()).unwrap_err();
        assert_eq!(err.kind(), &ErrorKind::ParseError,);
    }

    #[test]
    fn test_parse_blkio_data() {
        assert_eq!(
            parse_blkio_data(TEST_BLKIO_DATA.to_string()).unwrap(),
            vec![
                BlkIoData {
                    major: 8,
                    minor: 48,
                    data: 454480833999,
                },
                BlkIoData {
                    major: 8,
                    minor: 32,
                    data: 228392923193,
                },
                BlkIoData {
                    major: 8,
                    minor: 16,
                    data: 772456885,
                },
                BlkIoData {
                    major: 8,
                    minor: 0,
                    data: 559583764,
                }
            ]
        );
    }
}
