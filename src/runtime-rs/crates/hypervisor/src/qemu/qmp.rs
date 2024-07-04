// Copyright (c) 2024 Red Hat
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use std::fmt::{Debug, Error, Formatter};
use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::time::Duration;

use qapi::qmp;
use qapi_spec::Dictionary;

pub struct Qmp {
    qmp: qapi::Qmp<qapi::Stream<BufReader<UnixStream>, UnixStream>>,

    // This is basically the output of
    // `cat /sys/devices/system/memory/block_size_bytes`
    // on the guest.  Note a slightly peculiar behaviour with relation to
    // the size of hotplugged memory blocks: if an amount of memory is being
    // hotplugged whose size is not an integral multiple of page size
    // (4k usually) hotplugging fails immediately.  However, if the amount
    // is fine wrt the page size *but* isn't wrt this "guest memory block size"
    // hotplugging apparently succeeds, even though none of the hotplugged
    // blocks seem ever to be onlined in the guest by kata-agent.
    // Store as u64 to keep up the convention of bytes being represented as u64.
    guest_memory_block_size: u64,
}

// We have to implement Debug since the Hypervisor trait requires it and Qmp
// is ultimately stored in one of Hypervisor's implementations (Qemu).
// We can't do it automatically since the type of Qmp::qmp isn't Debug.
impl Debug for Qmp {
    fn fmt(&self, _f: &mut Formatter<'_>) -> Result<(), Error> {
        Ok(())
    }
}

impl Qmp {
    pub fn new(qmp_sock_path: &str) -> Result<Self> {
        let stream = UnixStream::connect(qmp_sock_path)?;

        // Set the read timeout to protect runtime-rs from blocking forever
        // trying to set up QMP connection if qemu fails to launch.  The exact
        // value is a matter of judegement.  Setting it too long would risk
        // being ineffective since container runtime would timeout first anyway
        // (containerd's task creation timeout is 2 s by default).  OTOH
        // setting it too short would risk interfering with a normal launch,
        // perhaps just seeing some delay due to a heavily loaded host.
        stream.set_read_timeout(Some(Duration::from_millis(250)))?;

        let mut qmp = Qmp {
            qmp: qapi::Qmp::new(qapi::Stream::new(
                BufReader::new(stream.try_clone()?),
                stream,
            )),
            guest_memory_block_size: 0,
        };

        let info = qmp.qmp.handshake()?;
        info!(sl!(), "QMP initialized: {:#?}", info);

        Ok(qmp)
    }

    pub fn hotplug_vcpus(&mut self, vcpu_cnt: u32) -> Result<u32> {
        let hotpluggable_cpus = self.qmp.execute(&qmp::query_hotpluggable_cpus {})?;
        //info!(sl!(), "hotpluggable CPUs: {:#?}", hotpluggable_cpus);

        let mut hotplugged = 0;
        for vcpu in &hotpluggable_cpus {
            if hotplugged >= vcpu_cnt {
                break;
            }
            let core_id = match vcpu.props.core_id {
                Some(id) => id,
                None => continue,
            };
            if vcpu.qom_path.is_some() {
                info!(sl!(), "hotpluggable vcpu {} hotplugged already", core_id);
                continue;
            }
            let socket_id = match vcpu.props.socket_id {
                Some(id) => id,
                None => continue,
            };
            let thread_id = match vcpu.props.thread_id {
                Some(id) => id,
                None => continue,
            };

            let mut cpu_args = Dictionary::new();
            cpu_args.insert("socket-id".to_owned(), socket_id.into());
            cpu_args.insert("core-id".to_owned(), core_id.into());
            cpu_args.insert("thread-id".to_owned(), thread_id.into());
            self.qmp.execute(&qmp::device_add {
                bus: None,
                id: Some(vcpu_id_from_core_id(core_id)),
                driver: hotpluggable_cpus[0].type_.clone(),
                arguments: cpu_args,
            })?;

            hotplugged += 1;
        }

        info!(
            sl!(),
            "Qmp::hotplug_vcpus(): hotplugged {}/{} vcpus", hotplugged, vcpu_cnt
        );

        Ok(hotplugged)
    }

    pub fn hotunplug_vcpus(&mut self, vcpu_cnt: u32) -> Result<u32> {
        let hotpluggable_cpus = self.qmp.execute(&qmp::query_hotpluggable_cpus {})?;

        let mut hotunplugged = 0;
        for vcpu in &hotpluggable_cpus {
            if hotunplugged >= vcpu_cnt {
                break;
            }
            let core_id = match vcpu.props.core_id {
                Some(id) => id,
                None => continue,
            };
            if vcpu.qom_path.is_none() {
                info!(sl!(), "hotpluggable vcpu {} not hotplugged yet", core_id);
                continue;
            }
            self.qmp.execute(&qmp::device_del {
                id: vcpu_id_from_core_id(core_id),
            })?;
            hotunplugged += 1;
        }

        info!(
            sl!(),
            "Qmp::hotunplug_vcpus(): hotunplugged {}/{} vcpus", hotunplugged, vcpu_cnt
        );

        Ok(hotunplugged)
    }

    pub fn set_guest_memory_block_size(&mut self, size: u64) {
        self.guest_memory_block_size = size;
    }

    pub fn guest_memory_block_size(&self) -> u64 {
        self.guest_memory_block_size
    }
}

fn vcpu_id_from_core_id(core_id: i64) -> String {
    format!("cpu-{}", core_id)
}
