// Copyright (c) 2019-2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io;

use anyhow::{anyhow, Context, Error, Result};
use cgroups_rs::Cgroup;

use crate::cgroups::CgroupsResource;

pub(crate) fn delete_v1_cgroups(resource: &CgroupsResource) -> Result<()> {
    move_tasks_to_root(&resource.cgroup_manager).context("move tasks in sandbox cgroup to root")?;
    resource
        .cgroup_manager
        .delete()
        .context("delete sandbox cgroup")?;

    if let Some(overhead) = resource.overhead_cgroup_manager.as_ref() {
        move_tasks_to_root(overhead).context("move tasks in overhead cgroup to root")?;
        overhead.delete().context("delete overhead cgroup")?;
    }

    Ok(())
}

pub(crate) fn delete_v2_cgroups(resource: &CgroupsResource) -> Result<()> {
    let mut cgroup = resource.cgroup_manager.clone();

    // Move all threads from the sandbox and overhead to their parent,
    // then delete them all, and back to their parent cgroup.
    if resource.cgroup_config.threaded_mode() {
        move_tasks_to_parent(&cgroup).context("move tasks in sandbox cgroup to parent")?;
        cgroup.delete().context("delete sandbox cgroup")?;
        if let Some(overhead) = resource.overhead_cgroup_manager.as_ref() {
            move_tasks_to_parent(overhead).context("move tasks in overhead cgroup to parent")?;
            overhead.delete().context("delete overhead cgroup")?;
        }
        // Go back to the parent
        cgroup = cgroup.parent_control_group();
    }

    move_procs_to_root(&cgroup).context("move procs to root")?;
    cgroup.delete().context("delete sandbox cgroup")?;

    Ok(())
}

fn move_tasks_to_root(cgroup: &Cgroup) -> Result<()> {
    for pid in cgroup.tasks() {
        let pid_raw = pid.pid;
        if let Err(err) = cgroup
            .remove_task(pid)
            .with_context(|| anyhow!("remove task {}", pid_raw))
        {
            ignore_esrch_error(err)?;
        }
    }
    Ok(())
}

fn move_procs_to_root(cgroup: &Cgroup) -> Result<()> {
    for tgid in cgroup.procs() {
        let tgid_raw = tgid.pid;
        if let Err(err) = cgroup
            .remove_task_by_tgid(tgid)
            .with_context(|| anyhow!("remove proc {}", tgid_raw))
        {
            ignore_esrch_error(err)?;
        }
    }
    Ok(())
}

fn move_tasks_to_parent(cgroup: &Cgroup) -> Result<()> {
    for pid in cgroup.tasks() {
        let pid_raw = pid.pid;
        if let Err(err) = cgroup
            .move_task_to_parent(pid)
            .with_context(|| anyhow!("remove task {} to parent", pid_raw))
        {
            ignore_esrch_error(err)?;
        }
    }
    Ok(())
}

fn ignore_esrch_error(err: Error) -> Result<()> {
    if let Some(err) = err.source() {
        if let Some(io_err) = err.downcast_ref::<io::Error>() {
            if io_err.raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
        }
    }
    Err(err)
}
