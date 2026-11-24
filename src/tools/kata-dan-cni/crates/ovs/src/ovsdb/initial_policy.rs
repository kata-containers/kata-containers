// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

//! Initial host policies, published atomically with their new endpoint.

use dan_core::HostPortConfig;

use super::{
    includes_external_id, json, operation_failed, operation_rows, ovs_map, ovs_uuid, ovs_uuid_set,
    required_row, required_string, required_string_map, required_uuid, required_uuid_set,
    select_operation, wait_rows_operation, where_name, where_uuid, BTreeMap, OvsDbClient, OvsError,
    OvsPortSpec, Value, IFACE_ID_EXTERNAL_ID,
};

const NEW_QOS_UUID: &str = "kata_dan_qos";
const NEW_QUEUE_UUID: &str = "kata_dan_queue";
const DEFAULT_MIRROR: &str = "m0";
const QOS_COLUMNS: &[&str] = &["_uuid", "external_ids", "type", "queues", "other_config"];
const QUEUE_COLUMNS: &[&str] = &["_uuid", "external_ids", "other_config"];

pub(super) struct InitialPolicy {
    pub(super) bridge_uuid: String,
    pub(super) guards: Vec<Value>,
    pub(super) operations: Vec<Value>,
    pub(super) port_qos: Option<Value>,
    pub(super) mirror_uuid: Option<String>,
}

impl InitialPolicy {
    pub(super) fn read(
        client: &impl OvsDbClient,
        spec: &OvsPortSpec,
        config: &HostPortConfig,
    ) -> Result<Self, OvsError> {
        let columns = ["_uuid", "datapath_type", "mirrors"];
        let snapshot = client.transact(vec![select_operation(
            "Bridge",
            where_name(&spec.bridge_name),
            &columns,
        )])?;
        let rows = operation_rows(&snapshot[0], "select policy Bridge")?;
        let [value] = rows else {
            return Err(operation_failed(format!(
                "OVS bridge `{}` does not exist or is ambiguous",
                spec.bridge_name
            )));
        };
        let bridge = required_row(value, "Bridge")?;
        let datapath = required_string(bridge, "datapath_type")?;
        if config.has_qos() && !matches!(datapath.as_str(), "" | "system") {
            return Err(OvsError::InvalidInput(format!(
                "Linux host QoS requires a system OVS datapath; bridge `{}` uses `{datapath}`",
                spec.bridge_name,
            )));
        }
        let mut policy = Self {
            bridge_uuid: required_uuid(bridge, "_uuid")?,
            guards: vec![wait_rows_operation(
                "Bridge",
                where_name(&spec.bridge_name),
                &columns,
                rows.to_vec(),
            )],
            operations: Vec::new(),
            port_qos: None,
            mirror_uuid: None,
        };
        if config.has_qos() {
            policy.qos(client, spec, config)?;
        }
        if config.mirror {
            policy.mirror(client, &required_uuid_set(bridge, "mirrors")?)?;
        }
        Ok(policy)
    }

    fn mirror(
        &mut self,
        client: &impl OvsDbClient,
        bridge_mirrors: &std::collections::BTreeSet<String>,
    ) -> Result<(), OvsError> {
        let columns = ["_uuid", "name", "select_all"];
        let snapshot = client.transact(vec![select_operation("Mirror", Vec::new(), &columns)])?;
        let mut candidates = Vec::new();
        for value in operation_rows(&snapshot[0], "select Mirror")? {
            let row = required_row(value, "Mirror")?;
            let uuid = required_uuid(row, "_uuid")?;
            if !bridge_mirrors.contains(&uuid) {
                continue;
            }
            if required_string(row, "name")? != DEFAULT_MIRROR {
                continue;
            }
            candidates.push((uuid, value));
        }
        match candidates.as_slice() {
            [(uuid, value)] => {
                self.guards.push(wait_rows_operation(
                    "Mirror",
                    where_uuid(uuid),
                    &columns,
                    vec![(*value).clone()],
                ));
                // Only Kube-OVN's m0 represents its global mirror setting.
                // Respect that selection rather than narrowing it to this Pod.
                if value.get("select_all").and_then(Value::as_bool) != Some(true) {
                    self.mirror_uuid = Some(uuid.clone());
                }
            }
            _ => return Err(operation_failed(
                "Pod mirroring requires one existing `m0` Mirror attached to the endpoint bridge",
            )),
        }
        Ok(())
    }

    fn qos(
        &mut self,
        client: &impl OvsDbClient,
        spec: &OvsPortSpec,
        config: &HostPortConfig,
    ) -> Result<(), OvsError> {
        if config.pod_name.is_empty() || config.pod_namespace.is_empty() {
            return Err(OvsError::InvalidInput(
                "host QoS requires Pod name and namespace".into(),
            ));
        }
        let conditions = vec![includes_external_id(IFACE_ID_EXTERNAL_ID, &spec.iface_id)];
        let snapshot = client.transact(vec![
            select_operation("QoS", conditions.clone(), QOS_COLUMNS),
            select_operation("Queue", conditions.clone(), QUEUE_COLUMNS),
            select_operation("Port", Vec::new(), &["_uuid", "qos"]),
            select_operation("QoS", Vec::new(), &["_uuid", "queues"]),
        ])?;
        let qos_rows = operation_rows(&snapshot[0], "select initial QoS")?;
        let queue_rows = operation_rows(&snapshot[1], "select initial Queue")?;
        let qos = single_policy(qos_rows, "QoS", spec, config)?;
        let queue = single_policy(queue_rows, "Queue", spec, config)?;
        self.guards.extend([
            wait_rows_operation("QoS", conditions.clone(), QOS_COLUMNS, qos_rows.to_vec()),
            wait_rows_operation("Queue", conditions, QUEUE_COLUMNS, queue_rows.to_vec()),
        ]);
        if let Some(row) = qos {
            let uuid = required_uuid(row, "_uuid")?;
            // Existing native records may outlive a previous endpoint. Reuse
            // them only while no other Port still owns their QoS binding.
            let refs = vec![json!(["qos", "includes", ovs_uuid_set("uuid", &uuid)])];
            for value in operation_rows(&snapshot[2], "select Port QoS references")? {
                if required_uuid_set(required_row(value, "Port")?, "qos")?.contains(&uuid) {
                    return Err(operation_failed(
                        "initial QoS is still referenced by another Port",
                    ));
                }
            }
            self.guards
                .push(wait_rows_operation("Port", refs, &["_uuid"], Vec::new()));
            let queues = super::qos::queue_uuids(row.get("queues"))?;
            let expected_queue = queue
                .map(|queue| required_uuid(queue, "_uuid"))
                .transpose()?;
            if queues
                .iter()
                .any(|uuid| Some(uuid) != expected_queue.as_ref())
            {
                return Err(operation_failed(
                    "initial QoS references a Queue belonging to another policy",
                ));
            }
        }
        if let Some(row) = queue {
            let queue_uuid = required_uuid(row, "_uuid")?;
            let qos_uuid = qos.map(|qos| required_uuid(qos, "_uuid")).transpose()?;
            let references = operation_rows(&snapshot[3], "select QoS Queue references")?;
            for value in references {
                let row = required_row(value, "QoS")?;
                if super::qos::queue_uuids(row.get("queues"))?.contains(&queue_uuid)
                    && Some(required_uuid(row, "_uuid")?) != qos_uuid
                {
                    return Err(operation_failed(
                        "initial Queue is shared with another QoS policy",
                    ));
                }
            }
            // Queue references are map values at arbitrary IDs. Guard just
            // those reference columns, permitting unrelated rate/stat updates.
            self.guards.push(wait_rows_operation(
                "QoS",
                Vec::new(),
                &["_uuid", "queues"],
                references.to_vec(),
            ));
        }
        let mut queue_config = queue
            .map(|row| required_string_map(row, "other_config"))
            .transpose()?
            .unwrap_or_default();
        queue_config.remove("max-rate");
        if config.ingress_rate_mbps > 0 {
            queue_config.insert(
                "max-rate".into(),
                (config.ingress_rate_mbps * 1_000_000).to_string(),
            );
        }
        let htb = !queue_config.is_empty();
        let tags = BTreeMap::from([
            (IFACE_ID_EXTERNAL_ID.to_string(), spec.iface_id.clone()),
            (
                "pod".into(),
                format!("{}/{}", config.pod_namespace, config.pod_name),
            ),
        ]);
        if htb {
            let queue_ref = self.put(
                "Queue",
                queue,
                NEW_QUEUE_UUID,
                json!({"external_ids": ovs_map(&tags), "other_config": ovs_map(&queue_config)}),
            )?;
            let qos_ref = self.put(
                "QoS",
                qos,
                NEW_QOS_UUID,
                json!({
                    "type": "linux-htb", "external_ids": ovs_map(&tags),
                    "queues": ["map", [[0, queue_ref]]],
                }),
            )?;
            self.port_qos = Some(qos_ref);
        } else if !config.netem.is_empty() {
            let mut options = BTreeMap::new();
            for (name, value) in [
                ("latency", config.netem.latency_ms * 1_000),
                ("jitter", config.netem.jitter_ms * 1_000),
                ("limit", config.netem.limit_packets),
            ] {
                if value != 0 {
                    options.insert(name.to_string(), value.to_string());
                }
            }
            if config.netem.loss_percent > 0.0 {
                options.insert("loss".into(), config.netem.loss_percent.to_string());
            }
            let qos_ref = self.put(
                "QoS",
                qos,
                NEW_QOS_UUID,
                json!({
                    "type": "linux-netem", "external_ids": ovs_map(&tags),
                    "queues": ["map", []], "other_config": ovs_map(&options),
                }),
            )?;
            self.port_qos = Some(qos_ref);
        }
        // Disconnected root records otherwise survive endpoint deletion. Retire
        // only the unshared records covered by the guards above.
        if !htb {
            if let Some(queue) = queue {
                self.remove("Queue", queue)?;
            }
            if config.netem.is_empty() {
                if let Some(qos) = qos {
                    self.remove("QoS", qos)?;
                }
            }
        }
        Ok(())
    }

    fn put(
        &mut self,
        table: &str,
        existing: Option<&serde_json::Map<String, Value>>,
        name: &str,
        row: Value,
    ) -> Result<Value, OvsError> {
        if let Some(existing) = existing {
            let uuid = required_uuid(existing, "_uuid")?;
            self.operations.push(
                json!({"op": "update", "table": table, "where": where_uuid(&uuid), "row": row}),
            );
            Ok(ovs_uuid("uuid", &uuid))
        } else {
            self.operations
                .push(json!({"op": "insert", "table": table, "uuid-name": name, "row": row}));
            Ok(ovs_uuid("named-uuid", name))
        }
    }

    fn remove(
        &mut self,
        table: &str,
        row: &serde_json::Map<String, Value>,
    ) -> Result<(), OvsError> {
        self.operations.push(json!({"op": "delete", "table": table, "where": where_uuid(&required_uuid(row, "_uuid")?)}));
        Ok(())
    }
}

fn single_policy<'a>(
    rows: &'a [Value],
    table: &str,
    spec: &OvsPortSpec,
    config: &HostPortConfig,
) -> Result<Option<&'a serde_json::Map<String, Value>>, OvsError> {
    let [value] = rows else {
        return if rows.is_empty() {
            Ok(None)
        } else {
            Err(operation_failed(format!(
                "multiple {table} records claim iface-id `{}`",
                spec.iface_id
            )))
        };
    };
    let row = required_row(value, table)?;
    let ids = required_string_map(row, "external_ids")?;
    let pod = format!("{}/{}", config.pod_namespace, config.pod_name);
    if ids.get("pod") != Some(&pod) {
        return Err(operation_failed(format!(
            "{table} for iface-id `{}` belongs to another Pod",
            spec.iface_id
        )));
    }
    Ok(Some(row))
}
