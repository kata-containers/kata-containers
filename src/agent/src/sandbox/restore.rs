// Copyright (c) 2026 Ant Group
//
// SPDX-License-Identifier: Apache-2.0

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Context, Result};
use kata_sys_util::validate;
use protocols::agent::ContainerIDMapping;

use super::Sandbox;
use crate::network::update_guest_dns;

impl Sandbox {
    pub fn rebind_ids(&mut self, sandbox_id: &str, mappings: &[ContainerIDMapping]) -> Result<()> {
        validate::verify_id(sandbox_id).context("invalid restored sandbox id")?;
        if mappings.len() != self.containers.len() {
            return Err(anyhow!(
                "restore mapping has {} tasks, restored guest has {}",
                mappings.len(),
                self.containers.len()
            ));
        }

        let mut old_ids = HashSet::new();
        let mut new_ids = HashSet::new();
        let mut aliases = self
            .container_id_remappings
            .write()
            .map_err(|_| anyhow!("container ID alias lock is poisoned"))?;

        for mapping in mappings {
            let old_id = &mapping.old_id;
            let new_id = &mapping.new_id;
            validate::verify_id(old_id).context("invalid checkpoint task id")?;
            validate::verify_id(new_id).context("invalid restored task id")?;
            if !old_ids.insert(old_id.clone()) || !self.containers.contains_key(old_id) {
                return Err(anyhow!(
                    "checkpoint task {old_id} is duplicate or absent in restored guest"
                ));
            }
            if !new_ids.insert(new_id.clone()) {
                return Err(anyhow!("duplicate restored task id {new_id}"));
            }
            if old_id != new_id && self.containers.contains_key(new_id) {
                return Err(anyhow!("restored task id {new_id} already exists"));
            }
        }

        // Validate and reassert the Guest freezer state before changing files,
        // output-reader generations, IDs, aliases, or any container-indexed maps.
        for mapping in mappings {
            self.containers
                .get(&mapping.old_id)
                .ok_or_else(|| anyhow!("checkpoint task {} disappeared", mapping.old_id))?
                .ensure_restored_frozen()
                .context(format!(
                    "enforce frozen state for restored task {}",
                    mapping.old_id
                ))?;
        }

        // The VM snapshot contains the source shim's in-flight output RPC handlers.
        for mapping in mappings {
            let container = self
                .containers
                .get(&mapping.old_id)
                .ok_or_else(|| anyhow!("checkpoint task {} disappeared", mapping.old_id))?;
            for process in container.processes.values() {
                process.invalidate_output_readers();
            }
        }

        for mapping in mappings {
            let old_id = &mapping.old_id;
            let new_id = &mapping.new_id;
            let mut container = self
                .containers
                .remove(old_id)
                .ok_or_else(|| anyhow!("checkpoint task {old_id} disappeared"))?;

            container.id = new_id.clone();
            self.containers.insert(new_id.clone(), container);
            move_map_entry(&mut self.container_mounts, old_id, new_id);
            move_map_entry(&mut self.container_verity_devices, old_id, new_id);
            move_map_entry(&mut self.pcimap, old_id, new_id);
            record_id_mapping(&mut aliases, old_id, new_id);
        }

        self.id = sandbox_id.to_string();
        info!(self.logger, "restored sandbox IDs rebound"; "container_count" => mappings.len());

        Ok(())
    }

    pub fn refresh_restored_config(&mut self, hostname: &str, dns: Vec<String>) -> Result<()> {
        if hostname.is_empty() {
            return Err(anyhow::anyhow!(
                "restored sandbox hostname must not be empty"
            ));
        }

        self.shared_utsns
            .set_hostname(hostname)
            .context("set restored sandbox hostname")?;

        update_guest_dns(&dns).context("update restored sandbox DNS")?;

        self.hostname = hostname.to_string();

        self.network.reset_dns(dns);

        Ok(())
    }
}

fn move_map_entry<T>(map: &mut HashMap<String, T>, old_id: &str, new_id: &str) {
    if let Some(value) = map.remove(old_id) {
        map.insert(new_id.to_string(), value);
    }
}

fn record_id_mapping(aliases: &mut HashMap<String, String>, old_id: &str, new_id: &str) {
    for current_id in aliases.values_mut() {
        if current_id == old_id {
            *current_id = new_id.to_string();
        }
    }
    aliases.insert(old_id.to_string(), new_id.to_string());
}

#[cfg(test)]
mod tests {
    use super::record_id_mapping;
    use std::collections::HashMap;

    #[test]
    fn container_id_remapping_flattens_restore_chain() {
        let mut aliases = HashMap::new();

        record_id_mapping(&mut aliases, "source", "restore-1");
        record_id_mapping(&mut aliases, "restore-1", "restore-2");

        assert_eq!(aliases.get("source").map(String::as_str), Some("restore-2"));
        assert_eq!(
            aliases.get("restore-1").map(String::as_str),
            Some("restore-2")
        );
    }
}
