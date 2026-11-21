// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

//! Cleanup of unshared QoS/Queue rows attached to an owned Port.
//!
//! Reference checks and deletion share a transaction so a failed cleanup
//! leaves the Port attached for a later retry.

use super::{
    json, operation_failed, operation_rows, ovs_uuid_set, parse_uuid, required_row, required_uuid,
    required_uuid_set, select_operation, validate_uuid, wait_rows_operation, where_uuid, BTreeMap,
    BTreeSet, OvsDbClient, OvsError, Value,
};

pub(super) struct Cleanup {
    pub(super) guards: Vec<Value>,
    pub(super) deletes: Vec<Value>,
}

impl Cleanup {
    pub(super) fn read(client: &impl OvsDbClient, port_uuid: &str) -> Result<Self, OvsError> {
        // Snapshot references only; unrelated rate and statistics updates are allowed.
        let snapshot = client.transact(vec![
            select_operation("Port", Vec::new(), &["_uuid", "qos"]),
            select_operation("QoS", Vec::new(), &["_uuid", "queues"]),
        ])?;
        Self::from_snapshot(port_uuid, &snapshot)
    }

    fn from_snapshot(port_uuid: &str, snapshot: &[Value]) -> Result<Self, OvsError> {
        let ports = operation_rows(&snapshot[0], "select Port QoS references")?;
        let qos_rows = operation_rows(&snapshot[1], "select QoS Queue references")?;
        let mut port_qos = BTreeMap::new();
        for value in ports {
            let port = required_row(value, "Port")?;
            let uuid = required_uuid(port, "_uuid")?;
            let qos = required_uuid_set(port, "qos")?;
            if qos.len() > 1 || port_qos.insert(uuid, qos).is_some() {
                return Err(operation_failed("invalid Port QoS reference snapshot"));
            }
        }
        let attached = port_qos.get(port_uuid).ok_or_else(|| {
            OvsError::TransactionConflict(
                "owned Port disappeared while preparing QoS cleanup".into(),
            )
        })?;
        let qos_value = attached
            .iter()
            .next()
            .map_or_else(|| json!(["set", []]), |uuid| ovs_uuid_set("uuid", uuid));
        let mut cleanup = Self {
            guards: vec![wait_rows_operation(
                "Port",
                where_uuid(port_uuid),
                &["qos"],
                vec![json!({"qos": qos_value})],
            )],
            deletes: Vec::new(),
        };
        let Some(qos_uuid) = attached.iter().next() else {
            // This wait still detects a daemon attaching QoS after our read.
            return Ok(cleanup);
        };
        validate_uuid(qos_uuid)?;
        if port_qos
            .iter()
            .any(|(uuid, refs)| uuid != port_uuid && refs.contains(qos_uuid))
        {
            // A shared policy, and all of its queues, must outlive this Port.
            return Ok(cleanup);
        }

        let mut qos_queues = BTreeMap::new();
        for value in qos_rows {
            let qos = required_row(value, "QoS")?;
            let uuid = required_uuid(qos, "_uuid")?;
            let queues = queue_uuids(qos.get("queues"))?;
            if qos_queues.insert(uuid, (queues, value)).is_some() {
                return Err(operation_failed("duplicate QoS UUID in reference snapshot"));
            }
        }
        let (queues, qos_row) = qos_queues
            .get(qos_uuid)
            .ok_or_else(|| operation_failed("attached QoS is missing from reference snapshot"))?;
        cleanup.guards.push(wait_rows_operation(
            "QoS",
            where_uuid(qos_uuid),
            &["queues"],
            vec![json!({"queues": qos_row["queues"]})],
        ));
        cleanup.deletes.push(destroy("QoS", qos_uuid));

        for queue_uuid in queues {
            if !qos_queues
                .iter()
                .any(|(uuid, (refs, _))| uuid != qos_uuid && refs.contains(queue_uuid))
            {
                cleanup.deletes.push(destroy("Queue", queue_uuid));
            }
        }
        // Preserve rows shared in the snapshot; OVSDB rejects newly shared deletions.
        Ok(cleanup)
    }
}

fn destroy(table: &str, uuid: &str) -> Value {
    json!({"op": "delete", "table": table, "where": where_uuid(uuid)})
}

pub(super) fn queue_uuids(value: Option<&Value>) -> Result<BTreeSet<String>, OvsError> {
    let encoded = value
        .and_then(Value::as_array)
        .filter(|encoded| encoded.len() == 2 && encoded[0] == "map")
        .ok_or_else(|| operation_failed("QoS queues is not a UUID map"))?;
    let entries = encoded[1]
        .as_array()
        .ok_or_else(|| operation_failed("QoS queues has invalid map entries"))?;
    let mut keys = BTreeSet::new();
    let mut queues = BTreeSet::new();
    for entry in entries {
        let pair = entry
            .as_array()
            .filter(|pair| pair.len() == 2)
            .ok_or_else(|| operation_failed("QoS queues has an invalid map entry"))?;
        let key = pair[0]
            .as_u64()
            .and_then(|key| u32::try_from(key).ok())
            .ok_or_else(|| operation_failed("QoS queues has an invalid queue ID"))?;
        if !keys.insert(key) {
            return Err(operation_failed("QoS queues has duplicate queue IDs"));
        }
        let uuid = parse_uuid(&pair[1])
            .ok_or_else(|| operation_failed("QoS queues contains a non-UUID value"))?;
        validate_uuid(&uuid)?;
        queues.insert(uuid);
    }
    Ok(queues)
}
