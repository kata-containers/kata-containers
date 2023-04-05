// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::vec;

use super::common::CgroupHierarchy;
use super::common::{Properties, SLICE_SUFFIX, UNIT_MODE};
use super::interface::system::ManagerProxyBlocking as SystemManager;
use anyhow::{Context, Result};
use zbus::zvariant::Value;

pub trait SystemdInterface {
    fn start_unit(
        &self,
        pid: i32,
        parent: &str,
        unit_name: &str,
        cg_hierarchy: &CgroupHierarchy,
    ) -> Result<()>;

    fn set_properties(&self, unit_name: &str, properties: &Properties) -> Result<()>;

    fn stop_unit(&self, unit_name: &str) -> Result<()>;

    fn get_version(&self) -> Result<String>;

    fn unit_exists(&self, unit_name: &str) -> Result<bool>;

    fn add_process(&self, pid: i32, unit_name: &str) -> Result<()>;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DBusClient {}

impl DBusClient {
    fn build_proxy(&self) -> Result<SystemManager<'static>> {
        let connection =
            zbus::blocking::Connection::system().context("Establishing a D-Bus connection")?;
        let proxy = SystemManager::new(&connection).context("Building a D-Bus proxy manager")?;
        Ok(proxy)
    }
}

impl SystemdInterface for DBusClient {
    fn start_unit(
        &self,
        pid: i32,
        parent: &str,
        unit_name: &str,
        cg_hierarchy: &CgroupHierarchy,
    ) -> Result<()> {
        let proxy = self.build_proxy()?;

        // enable CPUAccounting & MemoryAccounting & (Block)IOAccounting by default
        let mut properties: Properties = vec![
            ("CPUAccounting", Value::Bool(true)),
            ("DefaultDependencies", Value::Bool(false)),
            ("MemoryAccounting", Value::Bool(true)),
            ("TasksAccounting", Value::Bool(true)),
            ("Description", Value::Str("kata-agent container".into())),
            ("PIDs", Value::Array(vec![pid as u32].into())),
        ];

        match *cg_hierarchy {
            CgroupHierarchy::Legacy => properties.push(("IOAccounting", Value::Bool(true))),
            CgroupHierarchy::Unified => properties.push(("BlockIOAccounting", Value::Bool(true))),
        }

        if unit_name.ends_with(SLICE_SUFFIX) {
            properties.push(("Wants", Value::Str(parent.into())));
        } else {
            properties.push(("Slice", Value::Str(parent.into())));
            properties.push(("Delegate", Value::Bool(true)));
        }

        proxy
            .start_transient_unit(unit_name, UNIT_MODE, &properties, &[])
            .with_context(|| format!("failed to start transient unit {}", unit_name))?;
        Ok(())
    }

    fn set_properties(&self, unit_name: &str, properties: &Properties) -> Result<()> {
        let proxy = self.build_proxy()?;

        proxy
            .set_unit_properties(unit_name, true, properties)
            .with_context(|| format!("failed to set unit properties {}", unit_name))?;

        Ok(())
    }

    fn stop_unit(&self, unit_name: &str) -> Result<()> {
        let proxy = self.build_proxy()?;

        proxy
            .stop_unit(unit_name, UNIT_MODE)
            .with_context(|| format!("failed to stop unit {}", unit_name))?;
        Ok(())
    }

    fn get_version(&self) -> Result<String> {
        let proxy = self.build_proxy()?;

        let systemd_version = proxy
            .version()
            .with_context(|| "failed to get systemd version".to_string())?;
        Ok(systemd_version)
    }

    fn unit_exists(&self, unit_name: &str) -> Result<bool> {
        let proxy = self
            .build_proxy()
            .with_context(|| format!("Checking if systemd unit {} exists", unit_name))?;

        Ok(proxy.get_unit(unit_name).is_ok())
    }

    fn add_process(&self, pid: i32, unit_name: &str) -> Result<()> {
        let proxy = self.build_proxy()?;

        proxy
            .attach_processes_to_unit(unit_name, "/", &[pid as u32])
            .with_context(|| format!("failed to add process {}", unit_name))?;

        Ok(())
    }
}
