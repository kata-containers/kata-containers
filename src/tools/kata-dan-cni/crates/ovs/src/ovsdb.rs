// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

//! OVSDB table operations.

mod initial_policy;
mod qos;

use std::collections::{BTreeMap, BTreeSet};
use std::net::IpAddr;

use dan_core::HostPortConfig;
use serde_json::{json, Map, Value};

use crate::binding::{
    ENCAP_IP_EXTERNAL_ID, IFACE_ID_EXTERNAL_ID, OVN_INSTALLED_EXTERNAL_ID, OWNER_EXTERNAL_ID,
};
use crate::client::{OvsDbClient, UnixOvsDbClient};
use crate::{OvsControl, OvsError, OvsPortInfo, OvsPortSpec};

const NEW_INTERFACE_UUID: &str = "kata_dan_interface";
const NEW_PORT_UUID: &str = "kata_dan_port";

/// Synchronous, atomic OVSDB operations used by endpoint backends.
#[derive(Clone, Debug)]
pub struct OvsDbControl<C = UnixOvsDbClient> {
    client: C,
}

impl Default for OvsDbControl<UnixOvsDbClient> {
    fn default() -> Self {
        Self::new(UnixOvsDbClient::default())
    }
}

impl<C> OvsDbControl<C> {
    pub fn new(client: C) -> Self {
        Self { client }
    }
}

impl<C: OvsDbClient> OvsDbControl<C> {
    fn create_port(&self, spec: &OvsPortSpec, config: &HostPortConfig) -> Result<(), OvsError> {
        validate_name(&spec.bridge_name)?;
        validate_name(&spec.port_name)?;
        config
            .validate()
            .map_err(|error| OvsError::InvalidInput(error.to_string()))?;
        let policy = if config.is_empty() {
            None
        } else {
            Some(initial_policy::InitialPolicy::read(
                &self.client,
                spec,
                config,
            )?)
        };
        let bridge_uuid = match &policy {
            Some(policy) => policy.bridge_uuid.clone(),
            None => self.bridge_uuid(&spec.bridge_name)?,
        };
        let mut interface_ids = BTreeMap::from([
            (IFACE_ID_EXTERNAL_ID.to_string(), spec.iface_id.clone()),
            (OWNER_EXTERNAL_ID.to_string(), spec.owner_id.clone()),
        ]);
        if let Some(ip) = spec.encapsulation_ip {
            interface_ids.insert(ENCAP_IP_EXTERNAL_ID.to_string(), ip.to_string());
        }
        let port_ids = BTreeMap::from([(OWNER_EXTERNAL_ID.to_string(), spec.owner_id.clone())]);
        let mut interface_row = json!({
            "name": spec.port_name,
            "type": spec.interface_type,
            "options": ovs_map(&spec.options),
            "external_ids": ovs_map(&interface_ids),
        });
        if let Some(mtu) = spec.mtu_request {
            interface_row["mtu_request"] = json!(mtu);
        }
        if config.has_qos() {
            // Annotation directions are from the Pod's perspective.
            let rate = config.egress_rate_mbps * 1_000;
            interface_row["ingress_policing_rate"] = json!(rate);
            interface_row["ingress_policing_burst"] = json!(rate * 8 / 10);
        }
        let mut port_row = json!({
            "name": spec.port_name,
            "interfaces": ovs_uuid_set("named-uuid", NEW_INTERFACE_UUID),
            "external_ids": ovs_map(&port_ids),
        });
        if let Some(qos) = policy.as_ref().and_then(|policy| policy.port_qos.as_ref()) {
            port_row["qos"] = qos.clone();
        }
        let mut operations = vec![
            wait_rows_operation(
                "Bridge",
                where_name(&spec.bridge_name),
                &["_uuid"],
                vec![json!({"_uuid": ovs_uuid("uuid", &bridge_uuid)})],
            ),
            wait_empty_operation(
                "Interface",
                vec![includes_external_id(IFACE_ID_EXTERNAL_ID, &spec.iface_id)],
            ),
            wait_empty_operation("Interface", where_name(&spec.port_name)),
            wait_empty_operation("Port", where_name(&spec.port_name)),
        ];
        if let Some(policy) = &policy {
            operations.extend(policy.guards.clone());
            operations.extend(policy.operations.clone());
        }
        operations.extend([
            json!({"op": "insert", "table": "Interface", "uuid-name": NEW_INTERFACE_UUID, "row": interface_row}),
            json!({"op": "insert", "table": "Port", "uuid-name": NEW_PORT_UUID, "row": port_row}),
        ]);
        if let Some(mirror_uuid) = policy
            .as_ref()
            .and_then(|policy| policy.mirror_uuid.as_ref())
        {
            operations.push(json!({
                "op": "mutate", "table": "Mirror", "where": where_uuid(mirror_uuid),
                "mutations": [["select_dst_port", "insert", ovs_uuid_set("named-uuid", NEW_PORT_UUID)]],
            }));
        }
        // Publish only the fully configured endpoint; daemon updates cannot see
        // an Interface without its initial policing, QoS and mirror selection.
        let attach_index = operations.len();
        operations.push(json!({
            "op": "mutate", "table": "Bridge", "where": where_uuid(&bridge_uuid),
            "mutations": [["ports", "insert", ovs_uuid_set("named-uuid", NEW_PORT_UUID)]],
        }));
        let results = self.client.transact(operations)?;
        require_count(&results[attach_index], 1, "attach Port to Bridge")
    }

    fn inspect_port(
        &self,
        port_name: &str,
        include_configuration: bool,
    ) -> Result<Option<OvsPortInfo>, OvsError> {
        validate_name(port_name)?;
        let interface_columns: &[&str] = if include_configuration {
            &[
                "_uuid",
                "name",
                "type",
                "options",
                "external_ids",
                "ofport",
                "error",
                "mtu_request",
            ]
        } else {
            &["_uuid", "name", "external_ids"]
        };
        // All tables are read from the same transaction snapshot.
        let results = self.client.transact(vec![
            select_operation("Interface", where_name(port_name), interface_columns),
            select_operation(
                "Port",
                where_name(port_name),
                &["_uuid", "name", "interfaces", "external_ids"],
            ),
            select_operation("Bridge", Vec::new(), &["_uuid", "name", "ports"]),
        ])?;
        let interfaces = operation_rows(&results[0], "select Interface")?;
        let ports = operation_rows(&results[1], "select Port")?;
        if interfaces.is_empty() && ports.is_empty() {
            return Ok(None);
        }
        if interfaces.len() != 1 || ports.len() != 1 {
            return Err(operation_failed(format!(
                "OVS records for `{port_name}` are incomplete or ambiguous"
            )));
        }
        let interface = required_row(&interfaces[0], "Interface")?;
        let port = required_row(&ports[0], "Port")?;
        let interface_name = required_string(interface, "name")?;
        if interface_name != port_name || required_string(port, "name")? != port_name {
            return Err(operation_failed(format!(
                "OVS returned unexpected records for `{port_name}`"
            )));
        }
        let interface_uuid = required_uuid(interface, "_uuid")?;
        let port_uuid = required_uuid(port, "_uuid")?;
        if required_uuid_set(port, "interfaces")? != BTreeSet::from([interface_uuid.clone()]) {
            return Err(operation_failed(format!(
                "OVS port `{port_name}` does not contain exactly the expected interface"
            )));
        }
        let mut bridges = Vec::new();
        for value in operation_rows(&results[2], "select Bridge")? {
            let bridge = required_row(value, "Bridge")?;
            if required_uuid_set(bridge, "ports")?.contains(&port_uuid) {
                bridges.push((
                    required_uuid(bridge, "_uuid")?,
                    required_string(bridge, "name")?,
                ));
            }
        }
        if bridges.len() != 1 {
            return Err(operation_failed(format!(
                "OVS port `{port_name}` must belong to exactly one bridge, found {}",
                bridges.len()
            )));
        }
        let interface_ids = required_string_map(interface, "external_ids")?;
        let port_ids = required_string_map(port, "external_ids")?;
        let (bridge_uuid, bridge_name) = bridges.remove(0);
        let mut info = OvsPortInfo {
            port_uuid,
            interface_uuid,
            bridge_uuid,
            bridge_name,
            port_name: interface_name,
            port_owner: port_ids.get(OWNER_EXTERNAL_ID).cloned(),
            interface_owner: interface_ids.get(OWNER_EXTERNAL_ID).cloned(),
            iface_id: None,
            encapsulation_ip: None,
            ovn_installed: false,
            interface_type: String::new(),
            options: BTreeMap::new(),
            mtu_request: None,
            ofport: None,
            interface_error: None,
        };
        if include_configuration {
            info.iface_id = interface_ids.get(IFACE_ID_EXTERNAL_ID).cloned();
            info.encapsulation_ip = optional_ip(&interface_ids, ENCAP_IP_EXTERNAL_ID)?;
            info.ovn_installed = interface_ids
                .get(OVN_INSTALLED_EXTERNAL_ID)
                .is_some_and(|value| value == "true");
            info.interface_type = required_string(interface, "type")?;
            info.options = required_string_map(interface, "options")?;
            info.mtu_request = optional_i64(interface.get("mtu_request"), "mtu_request")?
                .map(|mtu| {
                    u32::try_from(mtu).map_err(|error| {
                        operation_failed(format!("invalid OVS MTU request `{mtu}`: {error}"))
                    })
                })
                .transpose()?;
            info.ofport = optional_i64(interface.get("ofport"), "ofport")?;
            info.interface_error = optional_string(interface.get("error"), "error")?;
        }
        Ok(Some(info))
    }

    fn bridge_uuid(&self, bridge_name: &str) -> Result<String, OvsError> {
        let results = self.client.transact(vec![select_operation(
            "Bridge",
            where_name(bridge_name),
            &["_uuid", "name"],
        )])?;
        let rows = operation_rows(&results[0], "select Bridge")?;
        match rows {
            [value] => {
                let row = required_row(value, "Bridge")?;
                if required_string(row, "name")? != bridge_name {
                    return Err(operation_failed("OVS returned an unexpected bridge"));
                }
                required_uuid(row, "_uuid")
            }
            [] => Err(operation_failed(format!(
                "OVS bridge `{bridge_name}` does not exist"
            ))),
            _ => Err(operation_failed(format!(
                "OVS bridge `{bridge_name}` is ambiguous"
            ))),
        }
    }
}

impl<C: OvsDbClient> OvsControl for OvsDbControl<C> {
    fn inspect(&self, port_name: &str) -> Result<Option<OvsPortInfo>, OvsError> {
        self.inspect_port(port_name, true)
    }

    fn interface_ready(&self, port_name: &str) -> Result<bool, OvsError> {
        validate_name(port_name)?;
        let results = self.client.transact(vec![select_operation(
            "Interface",
            where_name(port_name),
            &["ofport", "error", "external_ids"],
        )])?;
        let rows = operation_rows(&results[0], "select Interface readiness")?;
        let [value] = rows else {
            return Err(operation_failed(format!(
                "OVS interface `{port_name}` does not exist or is ambiguous"
            )));
        };
        let row = required_row(value, "Interface")?;
        if let Some(error) =
            optional_string(row.get("error"), "error")?.filter(|error| !error.is_empty())
        {
            return Err(operation_failed(format!(
                "OVS interface `{port_name}` reports an error: {error}"
            )));
        }
        let ofport = optional_i64(row.get("ofport"), "ofport")?;
        let ids = required_string_map(row, "external_ids")?;
        Ok(ofport.is_some_and(|value| value > 0)
            && ids
                .get(OVN_INSTALLED_EXTERNAL_ID)
                .is_some_and(|value| value == "true"))
    }

    fn inspect_owned(&self, port_name: &str) -> Result<Option<OvsPortInfo>, OvsError> {
        self.inspect_port(port_name, false)
    }

    fn interfaces_with_iface_id(&self, iface_id: &str) -> Result<Vec<String>, OvsError> {
        let results = self.client.transact(vec![select_operation(
            "Interface",
            vec![includes_external_id(IFACE_ID_EXTERNAL_ID, iface_id)],
            &["name"],
        )])?;
        operation_rows(&results[0], "select Interface")?
            .iter()
            .map(|value| {
                required_row(value, "Interface").and_then(|row| required_string(row, "name"))
            })
            .collect()
    }

    fn add_port(&self, spec: &OvsPortSpec) -> Result<(), OvsError> {
        self.create_port(spec, &HostPortConfig::default())
    }

    fn add_port_with_config(
        &self,
        spec: &OvsPortSpec,
        config: &HostPortConfig,
    ) -> Result<(), OvsError> {
        self.create_port(spec, config)
    }

    fn delete_owned_port(&self, info: &OvsPortInfo, owner_id: &str) -> Result<(), OvsError> {
        validate_name(&info.port_name)?;
        validate_name(&info.bridge_name)?;
        for uuid in [&info.port_uuid, &info.interface_uuid, &info.bridge_uuid] {
            validate_uuid(uuid)?;
        }
        if info.port_owner.as_deref() != Some(owner_id)
            || info.interface_owner.as_deref() != Some(owner_id)
        {
            return Err(operation_failed("refusing to delete an unowned OVS port"));
        }
        let mut port_conditions = where_uuid(&info.port_uuid);
        port_conditions.extend([
            json!(["name", "==", info.port_name]),
            json!([
                "interfaces",
                "==",
                ovs_uuid_set("uuid", &info.interface_uuid)
            ]),
            includes_external_id(OWNER_EXTERNAL_ID, owner_id),
        ]);
        let mut interface_conditions = where_uuid(&info.interface_uuid);
        interface_conditions.extend([
            json!(["name", "==", info.port_name]),
            includes_external_id(OWNER_EXTERNAL_ID, owner_id),
        ]);
        let cleanup = qos::Cleanup::read(&self.client, &info.port_uuid)?;
        let mut operations = vec![
            wait_rows_operation(
                "Port",
                port_conditions,
                &["_uuid"],
                vec![json!({"_uuid": ovs_uuid("uuid", &info.port_uuid)})],
            ),
            wait_rows_operation(
                "Interface",
                interface_conditions,
                &["_uuid"],
                vec![json!({"_uuid": ovs_uuid("uuid", &info.interface_uuid)})],
            ),
            // Endpoint deletion requires exclusive use of this Interface.
            wait_rows_operation(
                "Port",
                vec![json!([
                    "interfaces",
                    "includes",
                    ovs_uuid_set("uuid", &info.interface_uuid)
                ])],
                &["_uuid"],
                vec![json!({"_uuid": ovs_uuid("uuid", &info.port_uuid)})],
            ),
            // Bind deletion to the inspected bridge UUID and membership.
            wait_rows_operation(
                "Bridge",
                vec![json!([
                    "ports",
                    "includes",
                    ovs_uuid_set("uuid", &info.port_uuid)
                ])],
                &["_uuid", "name"],
                vec![json!({
                    "_uuid": ovs_uuid("uuid", &info.bridge_uuid), "name": info.bridge_name,
                })],
            ),
        ];
        operations.extend(cleanup.guards);
        // Clear the strong QoS reference; root QoS/Queue rows need explicit deletion.
        let clear_index = operations.len();
        operations.push(json!({
            "op": "update", "table": "Port", "where": where_uuid(&info.port_uuid),
            "row": {"qos": ["set", []]},
        }));
        let detach_index = operations.len();
        operations.push(json!({
            "op": "mutate", "table": "Bridge", "where": where_uuid(&info.bridge_uuid),
            "mutations": [["ports", "delete", ovs_uuid_set("uuid", &info.port_uuid)]],
        }));
        let destroy_index = operations.len();
        operations.extend(cleanup.deletes);
        let results = self.client.transact(operations)?;
        require_count(&results[clear_index], 1, "clear owned Port QoS reference")?;
        require_count(&results[detach_index], 1, "detach owned Port from Bridge")?;
        for result in &results[destroy_index..] {
            require_count(result, 1, "destroy unshared QoS or Queue")?;
        }
        Ok(())
    }

    fn bridge_datapath_type(&self, bridge_name: &str) -> Result<Option<String>, OvsError> {
        validate_name(bridge_name)?;
        let results = self.client.transact(vec![select_operation(
            "Bridge",
            where_name(bridge_name),
            &["name", "datapath_type"],
        )])?;
        match operation_rows(&results[0], "select Bridge")? {
            [] => Ok(None),
            [value] => {
                let row = required_row(value, "Bridge")?;
                if required_string(row, "name")? != bridge_name {
                    return Err(operation_failed("OVS returned an unexpected bridge"));
                }
                required_string(row, "datapath_type").map(Some)
            }
            _ => Err(operation_failed(format!(
                "OVS bridge `{bridge_name}` is ambiguous"
            ))),
        }
    }
}

fn validate_name(name: &str) -> Result<(), OvsError> {
    if name.is_empty() || name.contains('\0') {
        return Err(OvsError::InvalidInput(
            "OVS record name is empty or contains NUL".to_string(),
        ));
    }
    Ok(())
}

fn validate_uuid(uuid: &str) -> Result<(), OvsError> {
    if uuid.len() != 36
        || !uuid.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
    {
        return Err(OvsError::InvalidInput("OVS UUID is malformed".to_string()));
    }
    Ok(())
}

fn require_count(result: &Value, expected: u64, operation: &str) -> Result<(), OvsError> {
    if result.get("count").and_then(Value::as_u64) != Some(expected) {
        return Err(operation_failed(format!(
            "OVSDB {operation} did not affect {expected} row(s)"
        )));
    }
    Ok(())
}

fn where_uuid(uuid: &str) -> Vec<Value> {
    vec![json!(["_uuid", "==", ovs_uuid("uuid", uuid)])]
}

fn includes_external_id(key: &str, value: &str) -> Value {
    json!([
        "external_ids",
        "includes",
        ovs_map(&BTreeMap::from([(key.to_string(), value.to_string())]))
    ])
}

fn wait_rows_operation(
    table: &str,
    conditions: Vec<Value>,
    columns: &[&str],
    rows: Vec<Value>,
) -> Value {
    json!({
        "op": "wait", "timeout": 0, "table": table, "where": conditions,
        "columns": columns, "until": "==", "rows": rows,
    })
}

fn select_operation(table: &str, conditions: Vec<Value>, columns: &[&str]) -> Value {
    json!({
        "op": "select",
        "table": table,
        "where": conditions,
        "columns": columns,
    })
}

fn wait_empty_operation(table: &str, conditions: Vec<Value>) -> Value {
    wait_rows_operation(table, conditions, &["_uuid"], Vec::new())
}

fn where_name(name: &str) -> Vec<Value> {
    vec![json!(["name", "==", name])]
}

fn ovs_uuid(kind: &str, uuid: &str) -> Value {
    json!([kind, uuid])
}

fn ovs_uuid_set(kind: &str, uuid: &str) -> Value {
    json!(["set", [ovs_uuid(kind, uuid)]])
}

fn ovs_map(values: &BTreeMap<String, String>) -> Value {
    let entries = values
        .iter()
        .map(|(key, value)| json!([key, value]))
        .collect::<Vec<_>>();
    json!(["map", entries])
}

fn operation_rows<'a>(result: &'a Value, operation: &str) -> Result<&'a [Value], OvsError> {
    result
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .ok_or_else(|| operation_failed(format!("OVSDB {operation} result has no rows array")))
}

fn required_row<'a>(value: &'a Value, table: &str) -> Result<&'a Map<String, Value>, OvsError> {
    value
        .as_object()
        .ok_or_else(|| operation_failed(format!("OVSDB {table} row is not an object")))
}

fn required_string(record: &Map<String, Value>, field: &str) -> Result<String, OvsError> {
    record
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| {
            operation_failed(format!(
                "OVSDB field `{field}` is missing or is not a string"
            ))
        })
}

fn required_uuid(record: &Map<String, Value>, field: &str) -> Result<String, OvsError> {
    record
        .get(field)
        .and_then(parse_uuid)
        .ok_or_else(|| operation_failed(format!("OVSDB field `{field}` is not a UUID")))
}

fn parse_uuid(value: &Value) -> Option<String> {
    let encoded = value.as_array()?;
    if encoded.len() != 2
        || !matches!(
            encoded.first().and_then(Value::as_str),
            Some("uuid" | "named-uuid")
        )
    {
        return None;
    }
    encoded.get(1)?.as_str().map(ToString::to_string)
}

fn required_uuid_set(
    record: &Map<String, Value>,
    field: &str,
) -> Result<BTreeSet<String>, OvsError> {
    let value = record
        .get(field)
        .ok_or_else(|| operation_failed(format!("OVSDB field `{field}` is missing")))?;
    if let Some(uuid) = parse_uuid(value) {
        return Ok(BTreeSet::from([uuid]));
    }

    let encoded = value
        .as_array()
        .filter(|encoded| {
            encoded.len() == 2 && encoded.first().and_then(Value::as_str) == Some("set")
        })
        .ok_or_else(|| operation_failed(format!("OVSDB field `{field}` is not a UUID set")))?;
    encoded
        .get(1)
        .and_then(Value::as_array)
        .ok_or_else(|| operation_failed(format!("OVSDB field `{field}` has an invalid set")))?
        .iter()
        .map(|value| {
            parse_uuid(value).ok_or_else(|| {
                operation_failed(format!("OVSDB field `{field}` contains a non-UUID value"))
            })
        })
        .collect()
}

fn required_string_map(
    record: &Map<String, Value>,
    field: &str,
) -> Result<BTreeMap<String, String>, OvsError> {
    let encoded = record
        .get(field)
        .and_then(Value::as_array)
        .filter(|encoded| {
            encoded.len() == 2 && encoded.first().and_then(Value::as_str) == Some("map")
        })
        .ok_or_else(|| operation_failed(format!("OVSDB field `{field}` is not a map")))?;
    let entries = encoded
        .get(1)
        .and_then(Value::as_array)
        .ok_or_else(|| operation_failed(format!("OVSDB field `{field}` has invalid entries")))?;
    entries
        .iter()
        .map(|entry| {
            let pair = entry
                .as_array()
                .filter(|pair| pair.len() == 2)
                .ok_or_else(|| {
                    operation_failed(format!("OVSDB field `{field}` has an invalid map entry"))
                })?;
            let key = pair.first().and_then(Value::as_str).ok_or_else(|| {
                operation_failed(format!("OVSDB field `{field}` has an invalid map key"))
            })?;
            let value = pair.get(1).and_then(Value::as_str).ok_or_else(|| {
                operation_failed(format!("OVSDB field `{field}` has an invalid map value"))
            })?;
            Ok((key.to_string(), value.to_string()))
        })
        .collect()
}

fn optional_ip(values: &BTreeMap<String, String>, field: &str) -> Result<Option<IpAddr>, OvsError> {
    values
        .get(field)
        .map(|value| {
            value.parse::<IpAddr>().map_err(|error| {
                operation_failed(format!(
                    "OVS external ID `{field}` contains invalid IP `{value}`: {error}"
                ))
            })
        })
        .transpose()
}

fn optional_i64(value: Option<&Value>, field: &str) -> Result<Option<i64>, OvsError> {
    let Some(value) = optional_atom(value, field)? else {
        return Ok(None);
    };
    value
        .as_i64()
        .map(Some)
        .ok_or_else(|| operation_failed(format!("OVSDB field `{field}` is not an integer")))
}

fn optional_string(value: Option<&Value>, field: &str) -> Result<Option<String>, OvsError> {
    let Some(value) = optional_atom(value, field)? else {
        return Ok(None);
    };
    value
        .as_str()
        .map(|value| Some(value.to_string()))
        .ok_or_else(|| operation_failed(format!("OVSDB field `{field}` is not a string")))
}

fn optional_atom<'a>(value: Option<&'a Value>, field: &str) -> Result<Option<&'a Value>, OvsError> {
    let Some(value) = value else {
        return Ok(None);
    };
    let Some(encoded) = value.as_array() else {
        return Ok(Some(value));
    };
    if encoded.first().and_then(Value::as_str) != Some("set") || encoded.len() != 2 {
        return Ok(Some(value));
    }
    let members = encoded
        .get(1)
        .and_then(Value::as_array)
        .ok_or_else(|| operation_failed(format!("OVSDB field `{field}` has an invalid set")))?;
    match members.as_slice() {
        [] => Ok(None),
        [member] => Ok(Some(member)),
        _ => Err(operation_failed(format!(
            "OVSDB field `{field}` contains more than one value"
        ))),
    }
}

fn operation_failed(message: impl Into<String>) -> OvsError {
    OvsError::OperationFailed(message.into())
}

#[cfg(test)]
mod tests {
    //! Transaction safety contracts at the OVSDB boundary.

    use std::collections::{BTreeMap, VecDeque};
    use std::sync::{Arc, Mutex};

    use crate::{OvsControl, OvsDbClient, OvsDbControl, OvsError, OvsPortInfo, OvsPortSpec};
    use serde_json::{json, Value};

    const PORT: &str = "00000000-0000-0000-0000-000000000001";
    const INTERFACE: &str = "00000000-0000-0000-0000-000000000002";
    const BRIDGE: &str = "00000000-0000-0000-0000-000000000003";
    const QOS: &str = "00000000-0000-0000-0000-000000000004";
    const QUEUE: &str = "00000000-0000-0000-0000-000000000005";
    const OTHER: &str = "00000000-0000-0000-0000-000000000006";

    #[derive(Clone)]
    struct Database {
        snapshots: Arc<Mutex<VecDeque<Vec<Value>>>>,
        transactions: Arc<Mutex<Vec<Vec<Value>>>>,
    }

    impl Database {
        fn new(snapshots: Vec<Vec<Value>>) -> Self {
            Self {
                snapshots: Arc::new(Mutex::new(snapshots.into())),
                transactions: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn mutations(&self) -> Vec<Vec<Value>> {
            self.transactions
                .lock()
                .expect("transactions")
                .iter()
                .filter(|operations| operations.iter().any(|op| op["op"] != "select"))
                .cloned()
                .collect()
        }
    }

    impl OvsDbClient for Database {
        fn transact(&self, operations: Vec<Value>) -> Result<Vec<Value>, OvsError> {
            self.transactions
                .lock()
                .expect("transactions")
                .push(operations.clone());
            if operations.iter().all(|op| op["op"] == "select") {
                let snapshot = self
                    .snapshots
                    .lock()
                    .expect("snapshots")
                    .pop_front()
                    .expect("expected read");
                assert_eq!(snapshot.len(), operations.len());
                Ok(snapshot)
            } else {
                Ok(operations.iter().map(|_| json!({"count": 1})).collect())
            }
        }
    }

    fn spec() -> OvsPortSpec {
        OvsPortSpec {
            bridge_name: "br-int".into(),
            port_name: "kdt-contract".into(),
            interface_type: String::new(),
            options: BTreeMap::new(),
            mtu_request: Some(1500),
            iface_id: "pod.namespace.ovn".into(),
            encapsulation_ip: Some("192.0.2.10".parse().expect("IP")),
            owner_id: "owner".into(),
        }
    }

    fn owned() -> OvsPortInfo {
        OvsPortInfo {
            port_uuid: PORT.into(),
            interface_uuid: INTERFACE.into(),
            bridge_uuid: BRIDGE.into(),
            bridge_name: "br-int".into(),
            port_name: "kdt-contract".into(),
            port_owner: Some("owner".into()),
            interface_owner: Some("owner".into()),
            iface_id: Some(spec().iface_id),
            encapsulation_ip: None,
            ovn_installed: true,
            interface_type: String::new(),
            options: BTreeMap::new(),
            mtu_request: Some(1500),
            ofport: Some(12),
            interface_error: None,
        }
    }

    fn rows(values: Vec<Value>) -> Value {
        json!({"rows": values})
    }
    fn uuid(value: &str) -> Value {
        json!(["uuid", value])
    }
    fn uuid_set(value: &str) -> Value {
        json!(["set", [uuid(value)]])
    }

    fn string_map(value: &Value) -> BTreeMap<&str, &str> {
        assert_eq!(value[0], "map");
        value[1]
            .as_array()
            .expect("OVSDB map entries")
            .iter()
            .map(|entry| {
                (
                    entry[0].as_str().expect("string map key"),
                    entry[1].as_str().expect("string map value"),
                )
            })
            .collect()
    }

    fn has_condition(operation: &Value, expected: Value) -> bool {
        operation["where"]
            .as_array()
            .expect("OVSDB conditions")
            .contains(&expected)
    }

    #[test]
    fn publish_atomic() {
        let database = Database::new(vec![vec![rows(vec![
            json!({"_uuid": uuid(BRIDGE), "name": "br-int"}),
        ])]]);
        OvsDbControl::new(database.clone())
            .add_port(&spec())
            .expect("create");
        let writes = database.mutations();
        assert_eq!(writes.len(), 1, "endpoint publication must be atomic");
        let operations = &writes[0];
        let first_write = operations
            .iter()
            .position(|op| op["op"] != "wait")
            .expect("write");
        assert!(first_write >= 4);
        assert!(operations[..first_write]
            .iter()
            .all(|op| op["timeout"] == 0));
        let guard = operations
            .iter()
            .find(|op| {
                op["op"] == "wait"
                    && op["table"] == "Interface"
                    && has_condition(
                        op,
                        json!([
                            "external_ids",
                            "includes",
                            ["map", [["iface-id", spec().iface_id]]]
                        ]),
                    )
            })
            .expect("exclusive iface-id guard");
        assert_eq!(guard["rows"], json!([]));
        for table in ["Interface", "Port"] {
            assert!(operations.iter().any(|operation| {
                operation["op"] == "wait"
                    && operation["table"] == table
                    && has_condition(operation, json!(["name", "==", spec().port_name]))
                    && operation["rows"] == json!([])
            }));
        }
        for table in ["Port", "Interface"] {
            let insert = operations
                .iter()
                .find(|op| op["op"] == "insert" && op["table"] == table)
                .expect("owned row");
            assert_eq!(
                string_map(&insert["row"]["external_ids"]).get("kata-dan-owner"),
                Some(&"owner")
            );
        }
        let interface = operations
            .iter()
            .find(|op| op["op"] == "insert" && op["table"] == "Interface")
            .expect("interface");
        assert_eq!(interface["row"]["mtu_request"], 1500);
        let ids = string_map(&interface["row"]["external_ids"]);
        assert_eq!(ids.get("encap-ip"), Some(&"192.0.2.10"));
        assert_eq!(ids.get("iface-id"), Some(&"pod.namespace.ovn"));
        assert_eq!(operations.last().expect("publication")["table"], "Bridge");
        assert_eq!(operations.last().expect("publication")["op"], "mutate");
    }

    #[test]
    fn delete_owned_rows() {
        for (shared_qos, shared_queue, expected_deletes) in [
            (false, false, vec!["QoS", "Queue"]),
            (true, false, vec![]),
            (false, true, vec!["QoS"]),
        ] {
            let mut ports = vec![json!({"_uuid": uuid(PORT), "qos": uuid_set(QOS)})];
            if shared_qos {
                ports.push(json!({"_uuid": uuid(OTHER), "qos": uuid_set(QOS)}));
            }
            let mut policies =
                vec![json!({"_uuid": uuid(QOS), "queues": ["map", [[0, uuid(QUEUE)]]]})];
            if shared_queue {
                policies
                    .push(json!({"_uuid": uuid(OTHER), "queues": ["map", [[12, uuid(QUEUE)]]]}));
            }
            let database = Database::new(vec![vec![rows(ports), rows(policies)]]);
            OvsDbControl::new(database.clone())
                .delete_owned_port(&owned(), "owner")
                .expect("delete");
            let transactions = database.mutations();
            assert_eq!(transactions.len(), 1);
            let operations = &transactions[0];
            let deletes: Vec<_> = operations
                .iter()
                .filter(|op| op["op"] == "delete")
                .map(|op| op["table"].as_str().expect("table"))
                .collect();
            assert_eq!(deletes, expected_deletes);
            let guards: Vec<_> = operations.iter().filter(|op| op["op"] == "wait").collect();
            assert!(guards.iter().all(|guard| guard["timeout"] == 0));
            for (table, id) in [("Port", PORT), ("Interface", INTERFACE)] {
                assert!(guards.iter().any(|guard| {
                    guard["table"] == table
                        && has_condition(guard, json!(["_uuid", "==", uuid(id)]))
                        && has_condition(guard, json!(["name", "==", "kdt-contract"]))
                        && has_condition(
                            guard,
                            json!([
                                "external_ids",
                                "includes",
                                ["map", [["kata-dan-owner", "owner"]]]
                            ]),
                        )
                }));
            }
            assert!(guards.iter().any(|guard| {
                guard["table"] == "Port"
                    && has_condition(
                        guard,
                        json!(["interfaces", "includes", uuid_set(INTERFACE)]),
                    )
                    && guard["rows"] == json!([{"_uuid": uuid(PORT)}])
            }));
            assert!(guards.iter().any(|guard| {
                guard["table"] == "Bridge"
                    && has_condition(guard, json!(["ports", "includes", uuid_set(PORT)]))
                    && guard["rows"] == json!([{"_uuid": uuid(BRIDGE), "name": "br-int"}])
            }));
        }
    }
}
