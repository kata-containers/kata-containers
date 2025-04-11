// Copyright 2022 Alibaba Corporation. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

//! # Upcall Device Manager Service.
//!
//! Provides basic operations for the upcall device manager, include:
//! - CPU / Mmio-Virtio Device's hot-plug
//! - CPU Device's hot-unplug

use std::fmt;
use std::mem;

use dbs_virtio_devices::vsock::backend::VsockStream;

use crate::{
    Result, UpcallClientError, UpcallClientRequest, UpcallClientResponse, UpcallClientService,
};

const DEV_MGR_MSG_SIZE: usize = 0x400;
const DEV_MGR_MAGIC_VERSION: u32 = 0x444D0100;
const DEV_MGR_BYTE: &[u8; 1usize] = b"d";

/// Device manager's op code.
#[allow(dead_code)]
#[repr(u32)]
enum DevMgrMsgType {
    Connect = 0x00000000,
    AddCpu = 0x00000001,
    DelCpu = 0x00000002,
    AddMem = 0x00000003,
    DelMem = 0x00000004,
    AddMmio = 0x00000005,
    DelMmio = 0x00000006,
    AddPci = 0x00000007,
    DelPci = 0x00000008,
}

/// Device manager's header for messages.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct DevMgrMsgHeader {
    pub magic_version: u32,
    pub msg_size: u32,
    pub msg_type: u32,
    pub msg_flags: u32,
}

/// Command struct to add/del a PCI Device.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PciDevRequest {
    /// PCI bus number
    pub busno: u8,
    /// Combined device number and function number
    pub devfn: u8,
}

/// Command struct to add/del a MMIO Virtio Device.
#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MmioDevRequest {
    /// base address of the virtio MMIO configuration window.
    pub mmio_base: u64,
    /// size of the virtio MMIO configuration window.
    pub mmio_size: u64,
    /// Interrupt number assigned to the MMIO virito device.
    pub mmio_irq: u32,
}

/// Command struct to add/del a vCPU.
#[repr(C)]
#[derive(Clone)]
pub struct CpuDevRequest {
    /// hotplug or hot unplug cpu count
    pub count: u8,
    #[cfg(target_arch = "x86_64")]
    /// apic version
    pub apic_ver: u8,
    #[cfg(target_arch = "x86_64")]
    /// apic id array
    pub apic_ids: [u8; 256],
}

impl PartialEq for CpuDevRequest {
    #[cfg(target_arch = "x86_64")]
    fn eq(&self, other: &CpuDevRequest) -> bool {
        self.count == other.count
            && self.apic_ver == other.apic_ver
            && self
                .apic_ids
                .iter()
                .zip(other.apic_ids.iter())
                .all(|(s, o)| s == o)
    }

    #[cfg(target_arch = "aarch64")]
    fn eq(&self, other: &CpuDevRequest) -> bool {
        self.count == other.count
    }
}

impl fmt::Debug for CpuDevRequest {
    #[cfg(target_arch = "x86_64")]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use std::fmt::Write as _;
        let mut apic_ids = String::from("[ ");
        for apic_id in self.apic_ids.iter() {
            if apic_id == &0 {
                break;
            }
            let _ = write!(apic_ids, "{apic_id}");
            apic_ids.push(' ');
        }
        apic_ids.push_str(" ]");
        f.debug_struct("CpuDevRequest")
            .field("count", &self.count)
            .field("apic_ver", &self.apic_ver)
            .field("apic_ids", &apic_ids)
            .finish()
    }

    #[cfg(target_arch = "aarch64")]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CpuDevRequest")
            .field("count", &self.count)
            .finish()
    }
}

/// Device manager's request representation in client side.
#[derive(Clone, PartialEq, Debug)]
pub enum DevMgrRequest {
    /// Add a MMIO virtio device
    AddMmioDev(MmioDevRequest),
    /// Del a MMIO device device
    DelMmioDev(MmioDevRequest),
    /// Add a VCPU
    AddVcpu(CpuDevRequest),
    /// Del a VCPU
    DelVcpu(CpuDevRequest),
    /// Add a PCI device
    AddPciDev(PciDevRequest),
    /// Delete a PCI device
    DelPciDev(PciDevRequest),
}

impl DevMgrRequest {
    /// Convert client side's representation into server side's representation.
    pub fn build(&self) -> Box<[u8; DEV_MGR_MSG_SIZE]> {
        let buffer = Box::new([0; DEV_MGR_MSG_SIZE]);
        let size_hdr = mem::size_of::<DevMgrMsgHeader>();
        let msg_hdr = unsafe { &mut *(buffer.as_ptr() as *mut DevMgrMsgHeader) };

        msg_hdr.magic_version = DEV_MGR_MAGIC_VERSION;
        msg_hdr.msg_flags = 0;

        match self {
            DevMgrRequest::AddMmioDev(s) => {
                msg_hdr.msg_type = DevMgrMsgType::AddMmio as u32;
                msg_hdr.msg_size = mem::size_of::<MmioDevRequest>() as u32;
                let mmio_dev =
                    unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut MmioDevRequest) };
                *mmio_dev = *s;
            }
            DevMgrRequest::DelMmioDev(s) => {
                msg_hdr.msg_type = DevMgrMsgType::DelMmio as u32;
                msg_hdr.msg_size = mem::size_of::<MmioDevRequest>() as u32;
                let mmio_dev =
                    unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut MmioDevRequest) };
                *mmio_dev = *s;
            }
            DevMgrRequest::AddVcpu(s) => {
                msg_hdr.msg_type = DevMgrMsgType::AddCpu as u32;
                msg_hdr.msg_size = mem::size_of::<CpuDevRequest>() as u32;
                let vcpu_dev = unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut CpuDevRequest) };
                *vcpu_dev = s.clone();
            }
            DevMgrRequest::DelVcpu(s) => {
                msg_hdr.msg_type = DevMgrMsgType::DelCpu as u32;
                msg_hdr.msg_size = mem::size_of::<CpuDevRequest>() as u32;
                let vcpu_dev = unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut CpuDevRequest) };
                *vcpu_dev = s.clone();
            }
            DevMgrRequest::AddPciDev(s) => {
                msg_hdr.msg_type = DevMgrMsgType::AddPci as u32;
                msg_hdr.msg_size = mem::size_of::<PciDevRequest>() as u32;
                let pci_dev = unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut PciDevRequest) };
                *pci_dev = *s;
            }
            DevMgrRequest::DelPciDev(s) => {
                msg_hdr.msg_type = DevMgrMsgType::DelPci as u32;
                msg_hdr.msg_size = mem::size_of::<PciDevRequest>() as u32;
                let pci_dev = unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut PciDevRequest) };
                *pci_dev = *s;
            }
        }

        buffer
    }
}

/// Device manager's response from cpu device.
#[repr(C)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CpuDevResponse {
    #[cfg(target_arch = "x86_64")]
    /// apic id index of last act cpu
    pub apic_id_index: u32,
    #[cfg(target_arch = "aarch64")]
    /// cpu id of last act cpu
    pub cpu_id: u32,
}

/// Device manager's response inner message.
#[derive(Debug, Eq, PartialEq)]
pub struct DevMgrResponseInfo<I> {
    /// 0 means success and other result is the error code.
    pub result: i32,
    /// Additional info returned by device.
    pub info: I,
}

/// Device manager's response representation in client side.
#[derive(Debug, Eq, PartialEq)]
pub enum DevMgrResponse {
    /// Add mmio device's response (no response body)
    AddMmioDev(DevMgrResponseInfo<()>),
    /// Add / Del cpu device's response
    CpuDev(DevMgrResponseInfo<CpuDevResponse>),
    /// Other response
    Other(DevMgrResponseInfo<()>),
}

impl DevMgrResponse {
    /// Convert server side's representation into client side's representation.
    fn make(buffer: &[u8]) -> Result<Self> {
        let size_hdr = mem::size_of::<DevMgrMsgHeader>();
        let msg_hdr = unsafe { &mut *(buffer.as_ptr() as *mut DevMgrMsgHeader) };
        let result = unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut i32) };

        match msg_hdr.msg_type {
            msg_type
                if msg_type == DevMgrMsgType::AddCpu as u32
                    || msg_type == DevMgrMsgType::DelCpu as u32 =>
            {
                let response = unsafe {
                    &mut *(buffer[(size_hdr + mem::size_of::<u32>())..].as_ptr()
                        as *mut CpuDevResponse)
                };
                Ok(DevMgrResponse::CpuDev(DevMgrResponseInfo {
                    result: *result,
                    info: response.clone(),
                }))
            }
            msg_type if msg_type == DevMgrMsgType::AddMmio as u32 => {
                Ok(DevMgrResponse::AddMmioDev(DevMgrResponseInfo {
                    result: *result,
                    info: (),
                }))
            }
            _ => Ok(DevMgrResponse::Other(DevMgrResponseInfo {
                result: *result,
                info: (),
            })),
        }
    }
}

/// Device manager service, realized upcall client service.
#[derive(Default)]
pub struct DevMgrService {}

impl UpcallClientService for DevMgrService {
    fn connection_start(&self, stream: &mut Box<dyn VsockStream>) -> Result<()> {
        stream
            .write_all(DEV_MGR_BYTE)
            .map_err(UpcallClientError::ServiceConnect)
    }

    fn connection_check(&self, stream: &mut Box<dyn VsockStream>) -> Result<()> {
        let mut buf = [0; DEV_MGR_MSG_SIZE];
        stream
            .read_exact(&mut buf)
            .map_err(UpcallClientError::ServiceConnect)?;
        let hdr = unsafe { &*(buf.as_ptr() as *const DevMgrMsgHeader) };
        if hdr.magic_version == DEV_MGR_MAGIC_VERSION
            && hdr.msg_size == 0
            && hdr.msg_flags == 0
            && hdr.msg_type == DevMgrMsgType::Connect as u32
        {
            Ok(())
        } else {
            Err(UpcallClientError::InvalidMessage(format!(
                "upcall device manager expect msg_type {:?}, but received {}",
                DevMgrMsgType::Connect as u32,
                hdr.msg_type
            )))
        }
    }

    fn send_request(
        &self,
        stream: &mut Box<dyn VsockStream>,
        request: UpcallClientRequest,
    ) -> Result<()> {
        let msg = match request {
            UpcallClientRequest::DevMgr(req) => req.build(),
            // we don't have other message type yet
            #[cfg(test)]
            UpcallClientRequest::FakeRequest => unimplemented!(),
        };
        stream
            .write_all(&*msg)
            .map_err(UpcallClientError::SendRequest)
    }

    fn handle_response(&self, stream: &mut Box<dyn VsockStream>) -> Result<UpcallClientResponse> {
        let mut buf = [0; DEV_MGR_MSG_SIZE];
        stream
            .read_exact(&mut buf)
            .map_err(UpcallClientError::GetResponse)?;
        let response = DevMgrResponse::make(&buf)?;

        Ok(UpcallClientResponse::DevMgr(response))
    }
}

#[cfg(test)]
mod tests {
    use dbs_virtio_devices::vsock::backend::{VsockBackend, VsockInnerBackend};

    use super::*;

    #[test]
    fn test_build_dev_mgr_request() {
        let size_hdr = mem::size_of::<DevMgrMsgHeader>();
        // add mmio dev request
        {
            let add_mmio_dev_request = MmioDevRequest {
                mmio_base: 0,
                mmio_size: 1,
                mmio_irq: 2,
            };
            let dev_mgr_request = DevMgrRequest::AddMmioDev(add_mmio_dev_request);
            let buffer = dev_mgr_request.build();

            // valid total size
            assert_eq!(buffer.len(), DEV_MGR_MSG_SIZE);

            // valid header
            let msg_hdr = unsafe { &mut *(buffer.as_ptr() as *mut DevMgrMsgHeader) };
            assert_eq!(msg_hdr.magic_version, DEV_MGR_MAGIC_VERSION);
            assert_eq!(msg_hdr.msg_flags, 0);
            assert_eq!(msg_hdr.msg_type, DevMgrMsgType::AddMmio as u32);
            assert_eq!(msg_hdr.msg_size, mem::size_of::<MmioDevRequest>() as u32);

            // valid request
            let mmio_dev_req =
                unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut MmioDevRequest) };
            assert_eq!(mmio_dev_req, &add_mmio_dev_request);
        }

        // add vcpu dev request
        {
            let cpu_dev_request = CpuDevRequest {
                count: 1,
                #[cfg(target_arch = "x86_64")]
                apic_ver: 2,
                #[cfg(target_arch = "x86_64")]
                apic_ids: [3; 256],
            };
            let dev_mgr_request = DevMgrRequest::AddVcpu(cpu_dev_request.clone());
            let buffer = dev_mgr_request.build();

            // valid total size
            assert_eq!(buffer.len(), DEV_MGR_MSG_SIZE);

            // valid header
            let msg_hdr = unsafe { &mut *(buffer.as_ptr() as *mut DevMgrMsgHeader) };
            assert_eq!(msg_hdr.magic_version, DEV_MGR_MAGIC_VERSION);
            assert_eq!(msg_hdr.msg_flags, 0);
            assert_eq!(msg_hdr.msg_type, DevMgrMsgType::AddCpu as u32);
            assert_eq!(msg_hdr.msg_size, mem::size_of::<CpuDevRequest>() as u32);

            // valid request
            let cpu_dev_req = unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut CpuDevRequest) };
            assert_eq!(cpu_dev_req, &cpu_dev_request);
        }

        // del vcpu dev request
        {
            let cpu_dev_request = CpuDevRequest {
                count: 1,
                #[cfg(target_arch = "x86_64")]
                apic_ver: 2,
                #[cfg(target_arch = "x86_64")]
                apic_ids: [3; 256],
            };
            let dev_mgr_request = DevMgrRequest::DelVcpu(cpu_dev_request.clone());
            let buffer = dev_mgr_request.build();

            // valid total size
            assert_eq!(buffer.len(), DEV_MGR_MSG_SIZE);

            // valid header
            let msg_hdr = unsafe { &mut *(buffer.as_ptr() as *mut DevMgrMsgHeader) };
            assert_eq!(msg_hdr.magic_version, DEV_MGR_MAGIC_VERSION);
            assert_eq!(msg_hdr.msg_flags, 0);
            assert_eq!(msg_hdr.msg_type, DevMgrMsgType::DelCpu as u32);
            assert_eq!(msg_hdr.msg_size, mem::size_of::<CpuDevRequest>() as u32);

            // valid request
            let cpu_dev_req = unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut CpuDevRequest) };
            assert_eq!(cpu_dev_req, &cpu_dev_request);
        }
    }

    #[test]
    fn test_make_dev_mgr_response() {
        let size_hdr = mem::size_of::<DevMgrMsgHeader>();

        // test cpu response
        {
            let buffer = [0; DEV_MGR_MSG_SIZE];
            let msg_hdr = unsafe { &mut *(buffer.as_ptr() as *mut DevMgrMsgHeader) };

            msg_hdr.magic_version = DEV_MGR_MAGIC_VERSION;

            msg_hdr.msg_type = DevMgrMsgType::AddCpu as u32;
            msg_hdr.msg_size = mem::size_of::<CpuDevRequest>() as u32;
            msg_hdr.msg_flags = 0;

            let result = unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut i32) };
            *result = 0;

            let vcpu_result = unsafe {
                &mut *(buffer[(size_hdr + mem::size_of::<u32>())..].as_ptr() as *mut CpuDevResponse)
            };

            #[cfg(target_arch = "x86_64")]
            {
                vcpu_result.apic_id_index = 1;
            }
            #[cfg(target_arch = "aarch64")]
            {
                vcpu_result.cpu_id = 1;
            }

            match DevMgrResponse::make(&buffer).unwrap() {
                DevMgrResponse::CpuDev(resp) => {
                    assert_eq!(resp.result, 0);
                    #[cfg(target_arch = "x86_64")]
                    assert_eq!(resp.info.apic_id_index, 1);
                    #[cfg(target_arch = "aarch64")]
                    assert_eq!(resp.info.cpu_id, 1);
                }
                _ => unreachable!(),
            }
        }

        // test add mmio response
        {
            let buffer = [0; DEV_MGR_MSG_SIZE];
            let msg_hdr = unsafe { &mut *(buffer.as_ptr() as *mut DevMgrMsgHeader) };

            msg_hdr.magic_version = DEV_MGR_MAGIC_VERSION;

            msg_hdr.msg_type = DevMgrMsgType::AddMmio as u32;
            msg_hdr.msg_size = 0;
            msg_hdr.msg_flags = 0;

            let result = unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut i32) };
            *result = 0;

            match DevMgrResponse::make(&buffer).unwrap() {
                DevMgrResponse::AddMmioDev(resp) => {
                    assert_eq!(resp.result, 0);
                }
                _ => unreachable!(),
            }
        }

        // test result error
        {
            let buffer = [0; DEV_MGR_MSG_SIZE];
            let msg_hdr = unsafe { &mut *(buffer.as_ptr() as *mut DevMgrMsgHeader) };

            msg_hdr.magic_version = DEV_MGR_MAGIC_VERSION;

            msg_hdr.msg_type = DevMgrMsgType::AddMmio as u32;
            msg_hdr.msg_size = 0;
            msg_hdr.msg_flags = 0;

            let result = unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut i32) };
            *result = 1;

            match DevMgrResponse::make(&buffer).unwrap() {
                DevMgrResponse::AddMmioDev(resp) => {
                    assert_eq!(resp.result, 1);
                }
                _ => unreachable!(),
            }
        }

        // test invalid unknown msg flag
        {
            let buffer = [0; DEV_MGR_MSG_SIZE];
            let msg_hdr = unsafe { &mut *(buffer.as_ptr() as *mut DevMgrMsgHeader) };

            msg_hdr.magic_version = DEV_MGR_MAGIC_VERSION;

            msg_hdr.msg_type = 0xabcd1234;
            msg_hdr.msg_size = 0;
            msg_hdr.msg_flags = 0;

            let result = unsafe { &mut *(buffer[size_hdr..].as_ptr() as *mut i32) };
            *result = 1;

            match DevMgrResponse::make(&buffer).unwrap() {
                DevMgrResponse::Other(resp) => {
                    assert_eq!(resp.result, 1);
                }
                _ => unreachable!(),
            }
        }
    }

    fn get_vsock_inner_backend_stream_pair() -> (Box<dyn VsockStream>, Box<dyn VsockStream>) {
        let mut vsock_backend = VsockInnerBackend::new().unwrap();
        let connector = vsock_backend.get_connector();
        let outer_stream = connector.connect().unwrap();
        let inner_stream = vsock_backend.accept().unwrap();

        (inner_stream, outer_stream)
    }

    #[test]
    fn test_dev_mgr_service_connection_start() {
        let (mut inner_stream, mut outer_stream) = get_vsock_inner_backend_stream_pair();
        let dev_mgr_service = DevMgrService {};

        assert!(dev_mgr_service.connection_start(&mut inner_stream).is_ok());
        let mut reader_buf = [0; 1];
        outer_stream.read_exact(&mut reader_buf).unwrap();
        assert_eq!(reader_buf, [b'd']);
    }

    #[test]
    fn test_dev_mgr_service_send_request() {
        let (mut inner_stream, mut outer_stream) = get_vsock_inner_backend_stream_pair();
        let dev_mgr_service = DevMgrService {};

        let add_mmio_dev_request = DevMgrRequest::AddMmioDev(MmioDevRequest {
            mmio_base: 0,
            mmio_size: 1,
            mmio_irq: 2,
        });
        let request = UpcallClientRequest::DevMgr(add_mmio_dev_request.clone());

        assert!(dev_mgr_service
            .send_request(&mut outer_stream, request)
            .is_ok());

        let mut reader_buf = [0; DEV_MGR_MSG_SIZE];
        inner_stream.read_exact(&mut reader_buf).unwrap();

        assert!(add_mmio_dev_request
            .build()
            .iter()
            .zip(reader_buf.iter())
            .all(|(req, buf)| req == buf));
    }

    #[test]
    fn test_dev_mgr_service_handle_response() {
        let (mut inner_stream, mut outer_stream) = get_vsock_inner_backend_stream_pair();
        let dev_mgr_service = DevMgrService {};

        let buffer = [0; DEV_MGR_MSG_SIZE];
        let msg_hdr = unsafe { &mut *(buffer.as_ptr() as *mut DevMgrMsgHeader) };
        msg_hdr.magic_version = DEV_MGR_MAGIC_VERSION;
        msg_hdr.msg_type = DevMgrMsgType::AddMmio as u32;
        msg_hdr.msg_size = 0;

        inner_stream.write_all(&buffer).unwrap();
        assert!(dev_mgr_service.handle_response(&mut outer_stream).is_ok());
    }
}
