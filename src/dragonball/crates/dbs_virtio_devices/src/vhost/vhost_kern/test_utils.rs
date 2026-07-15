// Copyright (C) 2021-2023 Alibaba Cloud Computing. All rights reserved.
// Copyright (C) 2021-2023 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0 or BSD-3-Clause

use std::fs::File;
use std::os::fd::{AsRawFd, RawFd};

use vhost_rs::vhost_kern::VhostKernBackend;
use vhost_rs::{Error as VhostError, VhostUserMemoryRegionInfo, VringConfigData};
use vm_memory::GuestAddressSpace;
use vmm_sys_util::eventfd::EventFd;

pub type Result<T> = std::result::Result<T, VhostError>;

pub struct MockVhostNet<AS: GuestAddressSpace> {
    pub(crate) fd: i32,
    pub(crate) mem: AS,
}

impl<AS: GuestAddressSpace> MockVhostNet<AS> {
    pub fn new(mem: AS) -> Result<Self> {
        Ok(MockVhostNet { fd: 0, mem })
    }

    pub fn set_backend(&self, _queue_index: usize, _fd: Option<&File>) -> Result<()> {
        Ok(())
    }
}

impl<AS: GuestAddressSpace> VhostKernBackend for MockVhostNet<AS> {
    type AS = AS;

    fn mem(&self) -> &Self::AS {
        &self.mem
    }
}

impl<AS: GuestAddressSpace> AsRawFd for MockVhostNet<AS> {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

pub trait MockVhostBackend: std::marker::Sized {
    fn get_features(&mut self) -> Result<u64>;
    fn set_features(&mut self, features: u64) -> Result<()>;
    fn set_owner(&mut self) -> Result<()>;
    fn reset_owner(&mut self) -> Result<()>;
    fn set_mem_table(&mut self, regions: &[VhostUserMemoryRegionInfo]) -> Result<()>;
    fn set_log_base(&mut self, base: u64, fd: Option<RawFd>) -> Result<()>;
    fn set_log_fd(&mut self, fd: RawFd) -> Result<()>;
    fn set_vring_num(&mut self, queue_index: usize, num: u16) -> Result<()>;
    fn set_vring_addr(&mut self, queue_index: usize, config_data: &VringConfigData) -> Result<()>;
    fn set_vring_base(&mut self, queue_index: usize, base: u16) -> Result<()>;
    fn get_vring_base(&mut self, queue_index: usize) -> Result<u32>;
    fn set_vring_call(&mut self, queue_index: usize, fd: &EventFd) -> Result<()>;
    fn set_vring_kick(&mut self, queue_index: usize, fd: &EventFd) -> Result<()>;
    fn set_vring_err(&mut self, queue_index: usize, fd: &EventFd) -> Result<()>;
}

impl<T: VhostKernBackend> MockVhostBackend for T {
    fn set_owner(&mut self) -> Result<()> {
        Ok(())
    }

    fn reset_owner(&mut self) -> Result<()> {
        Ok(())
    }

    fn get_features(&mut self) -> Result<u64> {
        Ok(0)
    }

    fn set_features(&mut self, _features: u64) -> Result<()> {
        Ok(())
    }

    fn set_mem_table(&mut self, _regions: &[VhostUserMemoryRegionInfo]) -> Result<()> {
        Ok(())
    }

    fn set_log_base(&mut self, _base: u64, _fd: Option<RawFd>) -> Result<()> {
        Ok(())
    }

    fn set_log_fd(&mut self, _fd: RawFd) -> Result<()> {
        Ok(())
    }

    fn set_vring_num(&mut self, _queue_index: usize, _num: u16) -> Result<()> {
        Ok(())
    }

    fn set_vring_addr(
        &mut self,
        _queue_index: usize,
        _config_data: &VringConfigData,
    ) -> Result<()> {
        Ok(())
    }

    fn set_vring_base(&mut self, _queue_index: usize, _base: u16) -> Result<()> {
        Ok(())
    }

    fn get_vring_base(&mut self, _queue_index: usize) -> Result<u32> {
        Ok(0)
    }

    fn set_vring_call(&mut self, _queue_index: usize, _fd: &EventFd) -> Result<()> {
        Ok(())
    }

    fn set_vring_kick(&mut self, _queue_index: usize, _fd: &EventFd) -> Result<()> {
        Ok(())
    }

    fn set_vring_err(&mut self, _queue_index: usize, _fd: &EventFd) -> Result<()> {
        Ok(())
    }
}
