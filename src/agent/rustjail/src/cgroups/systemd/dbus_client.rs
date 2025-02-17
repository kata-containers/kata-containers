// Copyright 2021-2023 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::vec;

use super::common::{
    CgroupHierarchy, Properties, NO_SUCH_UNIT_ERROR, SIGNAL_KILL, SLICE_SUFFIX, UNIT_MODE_REPLACE,
    WHO_ENUM_ALL,
};
use super::interface::system::ManagerProxyBlocking as SystemManager;
use anyhow::{anyhow, Context, Result};
use zbus::zvariant::Value;

pub trait SystemdInterface {
    fn start_unit(&self, pid: i32, parent: &str, cg_hierarchy: &CgroupHierarchy) -> Result<()>;
    fn set_properties(&self, properties: &Properties) -> Result<()>;
    fn kill_unit(&self) -> Result<()>;
    fn freeze_unit(&self) -> Result<()>;
    fn thaw_unit(&self) -> Result<()>;
    fn add_process(&self, pid: i32, subcgroup: &str) -> Result<()>;
    fn get_version(&self) -> Result<String>;
    fn unit_exists(&self) -> Result<bool>;
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DBusClient {
    unit_name: String,
}

impl DBusClient {
    pub fn new(unit_name: String) -> Self {
        Self { unit_name }
    }

    fn build_proxy(&self) -> Result<SystemManager<'static>> {
        let connection =
            zbus::blocking::Connection::system().context("Establishing a D-Bus connection")?;
        let proxy = SystemManager::new(&connection).context("Building a D-Bus proxy manager")?;

        Ok(proxy)
    }
}

impl SystemdInterface for DBusClient {
    fn start_unit(&self, pid: i32, parent: &str, cg_hierarchy: &CgroupHierarchy) -> Result<()> {
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

        if self.unit_name.ends_with(SLICE_SUFFIX) {
            properties.push(("Wants", Value::Str(parent.into())));
        } else {
            properties.push(("Slice", Value::Str(parent.into())));
            properties.push(("Delegate", Value::Bool(true)));
        }

        proxy
            .start_transient_unit(&self.unit_name, UNIT_MODE_REPLACE, &properties, &[])
            .context(format!("failed to start transient unit {}", self.unit_name))?;

        Ok(())
    }

    fn set_properties(&self, properties: &Properties) -> Result<()> {
        let proxy = self.build_proxy()?;

        proxy
            .set_unit_properties(&self.unit_name, true, properties)
            .context(format!("failed to set unit {} properties", self.unit_name))?;

        Ok(())
    }

    fn kill_unit(&self) -> Result<()> {
        let proxy = self.build_proxy()?;

        proxy
            .kill_unit(&self.unit_name, WHO_ENUM_ALL, SIGNAL_KILL)
            .or_else(|e| match e {
                zbus::Error::MethodError(error_name, _, _)
                    if error_name.as_str() == NO_SUCH_UNIT_ERROR =>
                {
                    Ok(())
                }
                _ => Err(e),
            })
            .context(format!("failed to kill unit {}", self.unit_name))?;

        Ok(())
    }

    fn freeze_unit(&self) -> Result<()> {
        let proxy = self.build_proxy()?;

        proxy
            .freeze_unit(&self.unit_name)
            .context(format!("failed to freeze unit {}", self.unit_name))?;

        Ok(())
    }

    fn thaw_unit(&self) -> Result<()> {
        let proxy = self.build_proxy()?;

        proxy
            .thaw_unit(&self.unit_name)
            .context(format!("failed to thaw unit {}", self.unit_name))?;

        Ok(())
    }

    fn get_version(&self) -> Result<String> {
        let proxy = self.build_proxy()?;

        let systemd_version = proxy
            .version()
            .context("failed to get systemd version".to_string())?;

        Ok(systemd_version)
    }

    fn unit_exists(&self) -> Result<bool> {
        let proxy = self.build_proxy()?;

        match proxy.get_unit(&self.unit_name) {
            Ok(_) => Ok(true),
            Err(zbus::Error::MethodError(error_name, _, _))
                if error_name.as_str() == NO_SUCH_UNIT_ERROR =>
            {
                Ok(false)
            }
            Err(e) => Err(anyhow!(format!(
                "failed to check if unit {} exists: {:?}",
                self.unit_name, e
            ))),
        }
    }

    fn add_process(&self, pid: i32, subcgroup: &str) -> Result<()> {
        let proxy = self.build_proxy()?;
        proxy
            .attach_processes_to_unit(&self.unit_name, subcgroup, &[pid as u32])
            .context(format!(
                "failed to add process into unit {}",
                self.unit_name
            ))?;

        Ok(())
    }
}
