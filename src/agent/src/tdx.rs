// Copyright (c) 2023 Microsoft Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{bail, Result};
use nix::fcntl::{self, OFlag};
use nix::sys::stat::Mode;
use std::os::fd::{AsRawFd, FromRawFd};
use vmm_sys_util::ioctl::ioctl_with_val;
use vmm_sys_util::{ioctl_ioc_nr, ioctl_iowr_nr};

#[repr(C)]
#[derive(Default)]
/// Type header of TDREPORT_STRUCT.
struct TdTransportType {
    /// Type of the TDREPORT (0 - SGX, 81 - TDX, rest are reserved).
    type_: u8,

    /// Subtype of the TDREPORT (Default value is 0).
    sub_type: u8,

    /// TDREPORT version (Default value is 0).
    version: u8,

    /// Added for future extension.
    reserved: u8,
}

#[repr(C)]
/// TDX guest report data, MAC and TEE hashes.
struct ReportMac {
    /// TDREPORT type header.
    type_: TdTransportType,

    /// Reserved for future extension.
    reserved1: [u8; 12],

    /// CPU security version.
    cpu_svn: [u8; 16],

    /// SHA384 hash of TEE TCB INFO.
    tee_tcb_info_hash: [u8; 48],

    /// SHA384 hash of TDINFO_STRUCT.
    tee_td_info_hash: [u8; 48],

    /// User defined unique data passed in TDG.MR.REPORT request.
    reportdata: [u8; 64],

    /// Reserved for future extension.
    reserved2: [u8; 32],

    /// CPU MAC ID.
    mac: [u8; 32],
}

impl Default for ReportMac {
    fn default() -> Self {
        Self {
            type_: Default::default(),
            reserved1: [0; 12],
            cpu_svn: [0; 16],
            tee_tcb_info_hash: [0; 48],
            tee_td_info_hash: [0; 48],
            reportdata: [0; 64],
            reserved2: [0; 32],
            mac: [0; 32],
        }
    }
}

#[repr(C)]
#[derive(Default)]
/// TDX guest measurements and configuration.
struct TdInfo {
    /// TDX Guest attributes (like debug, spet_disable, etc).
    attr: [u8; 8],

    /// Extended features allowed mask.
    xfam: u64,

    /// Build time measurement register.
    mrtd: [u64; 6],

    /// Software-defined ID for non-owner-defined configuration of the guest - e.g., run-time or OS configuration.
    mrconfigid: [u64; 6],

    /// Software-defined ID for the guest owner.
    mrowner: [u64; 6],

    /// Software-defined ID for owner-defined configuration of the guest - e.g., specific to the workload.
    mrownerconfig: [u64; 6],

    /// Run time measurement registers.
    rtmr: [u64; 24],

    /// For future extension.
    reserved: [u64; 14],
}

#[repr(C)]
/// Output of TDCALL[TDG.MR.REPORT].
struct TdReport {
    /// Mac protected header of size 256 bytes.
    report_mac: ReportMac,

    /// Additional attestable elements in the TCB are not reflected in the report_mac.
    tee_tcb_info: [u8; 239],

    /// Added for future extension.
    reserved: [u8; 17],

    /// Measurements and configuration data of size 512 bytes.
    tdinfo: TdInfo,
}

impl Default for TdReport {
    fn default() -> Self {
        Self {
            report_mac: Default::default(),
            tee_tcb_info: [0; 239],
            reserved: [0; 17],
            tdinfo: Default::default(),
        }
    }
}

#[repr(C)]
/// Request struct for TDX_CMD_GET_REPORT0 IOCTL.
struct TdxReportReq {
    /// User buffer with REPORTDATA to be included into TDREPORT.
    /// Typically it can be some nonce provided by attestation, service,
    /// so the generated TDREPORT can be uniquely verified.
    reportdata: [u8; 64],

    /// User buffer to store TDREPORT output from TDCALL[TDG.MR.REPORT].
    tdreport: TdReport,
}

impl Default for TdxReportReq {
    fn default() -> Self {
        Self {
            reportdata: [0; 64],
            tdreport: Default::default(),
        }
    }
}

// Get TDREPORT0 (a.k.a. TDREPORT subtype 0) using TDCALL[TDG.MR.REPORT].
ioctl_iowr_nr!(
    TDX_CMD_GET_REPORT0,
    'T' as ::std::os::raw::c_uint,
    1,
    TdxReportReq
);

pub fn get_tdx_mrconfigid() -> Result<Vec<u8>> {
    let fd = {
        let raw_fd = fcntl::open(
            "/dev/tdx_guest",
            OFlag::O_CLOEXEC | OFlag::O_RDWR | OFlag::O_SYNC,
            Mode::empty(),
        )?;
        unsafe { std::fs::File::from_raw_fd(raw_fd) }
    };

    let mut req = TdxReportReq {
        reportdata: [0; 64],
        tdreport: Default::default(),
    };
    let ret = unsafe {
        ioctl_with_val(
            &fd.as_raw_fd(),
            TDX_CMD_GET_REPORT0(),
            &mut req as *mut TdxReportReq as std::os::raw::c_ulong,
        )
    };
    if ret < 0 {
        bail!(
            "TDX_CMD_GET_REPORT0 failed: {:?}",
            std::io::Error::last_os_error(),
        );
    }

    let mrconfigid: Vec<u8> = req
        .tdreport
        .tdinfo
        .mrconfigid
        .iter()
        .flat_map(|val| val.to_le_bytes())
        .collect();
    Ok(mrconfigid)
}
