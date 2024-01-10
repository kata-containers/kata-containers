// Copyright (C) 2019-2023 Alibaba Cloud. All rights reserved.
// Copyright (C) 2019-2023 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

use log::{debug, error, warn};
use virtio_bindings::bindings::virtio_net::{
    virtio_net_ctrl_hdr, virtio_net_ctrl_mq, VIRTIO_NET_CTRL_MQ_VQ_PAIRS_SET,
};
use virtio_queue::{Descriptor, DescriptorChain};
use vm_memory::{Bytes, GuestMemory};

use crate::{DbsGuestAddressSpace, Error as VirtioError, Result as VirtioResult};

pub(crate) trait FromNetCtrl<T> {
    fn from_net_ctrl_st<M: GuestMemory>(mem: &M, desc: &Descriptor) -> VirtioResult<T> {
        let mut buf = vec![0u8; std::mem::size_of::<T>()];
        match mem.read_slice(&mut buf, desc.addr()) {
            Ok(_) => unsafe { Ok(std::ptr::read_volatile(&buf[..] as *const _ as *const T)) },
            Err(err) => {
                error!("Failed to read from memory, {}", err);
                Err(VirtioError::InternalError)
            }
        }
    }
}

impl FromNetCtrl<virtio_net_ctrl_hdr> for virtio_net_ctrl_hdr {}
impl FromNetCtrl<virtio_net_ctrl_mq> for virtio_net_ctrl_mq {}

pub(crate) fn virtio_handle_ctrl_mq<AS, F>(
    desc_chain: &mut DescriptorChain<&AS::M>,
    cmd: u8,
    mem: &AS::M,
    ctrl_mq_vq_pairs_set: F,
) -> VirtioResult<()>
where
    AS: DbsGuestAddressSpace,
    F: FnOnce(u16) -> VirtioResult<()>,
{
    if cmd == VIRTIO_NET_CTRL_MQ_VQ_PAIRS_SET as u8 {
        if let Some(next) = desc_chain.next() {
            if let Ok(ctrl_mq) = virtio_net_ctrl_mq::from_net_ctrl_st(mem, &next) {
                let curr_queues = ctrl_mq.virtqueue_pairs;
                ctrl_mq_vq_pairs_set(curr_queues)?;
            }
        }
    }
    Ok(())
}

pub(crate) fn virtio_handle_ctrl_status<AS>(
    driver_name: &str,
    desc_chain: &mut DescriptorChain<&AS::M>,
    status: u8,
    mem: &AS::M,
) -> VirtioResult<u32>
where
    AS: DbsGuestAddressSpace,
{
    let buf = vec![status];
    let mut total = 0;
    for next in desc_chain {
        if next.is_write_only() {
            match mem.write_slice(&buf, next.addr()) {
                Ok(_) => {
                    debug!("{}: succeed to update virtio ctrl status!", driver_name);
                    total += 1;
                }
                Err(_) => warn!("{}: failed to update ctrl status!", driver_name),
            }
        }
    }
    Ok(total)
}
