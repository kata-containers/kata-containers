// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;
use std::thread;
use std::time::{Duration, Instant};

use dan_core::HostPortConfig;

use crate::{OvsControl, OvsDbControl, OvsError, OvsPortInfo, OvsPortSpec};

const READY_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(500);
const MAX_TRANSACTION_ATTEMPTS: usize = 8;
const CONFLICT_RETRY_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) const OWNER_EXTERNAL_ID: &str = "kata-dan-owner";
pub(crate) const IFACE_ID_EXTERNAL_ID: &str = "iface-id";
pub(crate) const ENCAP_IP_EXTERNAL_ID: &str = "encap-ip";
pub(crate) const OVN_INSTALLED_EXTERNAL_ID: &str = "ovn-installed";

/// Whether this call created the OVS port or reused an existing owned port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateOutcome {
    Created,
    AlreadyPresent,
}

impl OvsPortInfo {
    fn validate_owned(&self, port_name: &str, owner_id: &str) -> Result<(), OvsError> {
        if self.port_name != port_name
            || self.port_owner.as_deref() != Some(owner_id)
            || self.interface_owner.as_deref() != Some(owner_id)
        {
            return Err(OvsError::OperationFailed(format!(
                "OVS port `{port_name}` is not owned by this attachment"
            )));
        }
        Ok(())
    }

    fn validate_configured(&self, spec: &OvsPortSpec) -> Result<(), OvsError> {
        self.validate_owned(&spec.port_name, &spec.owner_id)?;
        if self.bridge_name != spec.bridge_name
            || self.interface_type != spec.interface_type
            || spec
                .options
                .iter()
                .any(|(key, value)| self.options.get(key) != Some(value))
            || self.mtu_request != spec.mtu_request
            || self.iface_id.as_deref() != Some(spec.iface_id.as_str())
            || self.encapsulation_ip != spec.encapsulation_ip
        {
            return Err(OvsError::OperationFailed(format!(
                "OVS port `{}` does not match its expected bridge, interface type, options, MTU or OVN binding",
                spec.port_name
            )));
        }
        Ok(())
    }

    pub(crate) fn is_ready(&self) -> Result<bool, OvsError> {
        if let Some(error) = self
            .interface_error
            .as_deref()
            .filter(|error| !error.is_empty())
        {
            return Err(OvsError::OperationFailed(format!(
                "OVS interface `{}` reports an error: {error}",
                self.port_name
            )));
        }
        Ok(self.ofport.is_some_and(|value| value > 0) && self.ovn_installed)
    }
}

/// Applies the project's ownership and OVN binding policy to OVS ports.
#[derive(Clone, Debug)]
pub struct OvsBinding<C = OvsDbControl> {
    control: C,
}

impl Default for OvsBinding<OvsDbControl> {
    fn default() -> Self {
        Self::new(OvsDbControl::default())
    }
}

impl<C> OvsBinding<C> {
    pub const fn new(control: C) -> Self {
        Self { control }
    }
}

impl<C: OvsControl> OvsBinding<C> {
    pub fn create_or_validate(&self, spec: &OvsPortSpec) -> Result<CreateOutcome, OvsError> {
        create_or_validate_with(&self.control, spec, READY_TIMEOUT, POLL_INTERVAL)
    }

    pub fn create_or_validate_with_config(
        &self,
        spec: &OvsPortSpec,
        config: &HostPortConfig,
    ) -> Result<CreateOutcome, OvsError> {
        config
            .validate()
            .map_err(|error| OvsError::InvalidInput(error.to_string()))?;
        create_or_validate_configured(&self.control, spec, config, READY_TIMEOUT, POLL_INTERVAL)
    }

    /// Validate local ownership and configuration against the current OVS binding.
    pub fn check_owned(
        &self,
        bridge_name: &str,
        port_name: &str,
        owner_id: &str,
        interface_type: &str,
        options: &BTreeMap<String, String>,
        mtu_request: Option<u32>,
    ) -> Result<(), OvsError> {
        let info = self.control.inspect(port_name)?.ok_or_else(|| {
            OvsError::OperationFailed(format!("OVS port `{port_name}` does not exist"))
        })?;
        let iface_id = info
            .iface_id
            .clone()
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                OvsError::OperationFailed(format!("OVS port `{port_name}` has no iface-id"))
            })?;
        let spec = OvsPortSpec {
            bridge_name: bridge_name.to_string(),
            port_name: port_name.to_string(),
            interface_type: interface_type.to_string(),
            options: options.clone(),
            mtu_request,
            owner_id: owner_id.to_string(),
            iface_id,
            encapsulation_ip: info.encapsulation_ip,
        };
        info.validate_configured(&spec)?;
        ensure_iface_id_unique(&self.control, &spec)?;
        if !info.is_ready()? {
            return Err(OvsError::OperationFailed(format!(
                "OVS interface `{port_name}` is not ready: ofport {:?}, ovn-installed {}",
                info.ofport, info.ovn_installed
            )));
        }
        Ok(())
    }

    /// Remove an owned port regardless of its current configuration or readiness.
    pub fn delete_owned(&self, port_name: &str, owner_id: &str) -> Result<(), OvsError> {
        delete_owned_with(&self.control, port_name, owner_id)
    }
}

fn create_or_validate_with<C: OvsControl>(
    control: &C,
    spec: &OvsPortSpec,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<CreateOutcome, OvsError> {
    create_or_validate_configured(
        control,
        spec,
        &HostPortConfig::default(),
        timeout,
        poll_interval,
    )
}

fn create_or_validate_configured<C: OvsControl>(
    control: &C,
    spec: &OvsPortSpec,
    config: &HostPortConfig,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<CreateOutcome, OvsError> {
    let mut retry = ConflictRetry::new();
    let created = loop {
        if let Some(info) = control.inspect(&spec.port_name)? {
            info.validate_configured(spec)?;
            break false;
        }
        let names = control.interfaces_with_iface_id(&spec.iface_id)?;
        if names.iter().any(|name| name != &spec.port_name) {
            return Err(OvsError::OperationFailed(format!(
                "OVS iface-id `{}` is already in use",
                spec.iface_id
            )));
        }
        if !names.is_empty() {
            retry.after_conflict(OvsError::TransactionConflict(
                "OVS port appeared after the ADD snapshot".into(),
            ))?;
            continue;
        }
        match control.add_port_with_config(spec, config) {
            Ok(()) => break true,
            Err(error @ OvsError::TransactionConflict(_)) => retry.after_conflict(error)?,
            Err(error) => {
                // An I/O failure may follow a commit; clean up owned rows without retrying.
                return Err(cleanup_failed_create(control, spec, error));
            }
        }
    };
    let ready = ensure_iface_id_unique(control, spec)
        .and_then(|()| wait_until_ready(control, &spec.port_name, timeout, poll_interval));
    if let Err(error) = ready {
        return Err(if created {
            cleanup_failed_create(control, spec, error)
        } else {
            error
        });
    }
    Ok(if created {
        CreateOutcome::Created
    } else {
        CreateOutcome::AlreadyPresent
    })
}

fn wait_until_ready<C: OvsControl>(
    control: &C,
    port_name: &str,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<(), OvsError> {
    let deadline = Instant::now() + timeout;
    loop {
        if control.interface_ready(port_name)? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(OvsError::OperationFailed(format!(
                "OVS interface `{port_name}` did not become ready within {} seconds",
                timeout.as_secs()
            )));
        }
        thread::sleep(poll_interval.min(deadline.saturating_duration_since(Instant::now())));
    }
}

fn ensure_iface_id_unique<C: OvsControl>(control: &C, spec: &OvsPortSpec) -> Result<(), OvsError> {
    let names = control.interfaces_with_iface_id(&spec.iface_id)?;
    if names.len() == 1 && names[0] == spec.port_name {
        return Ok(());
    }
    Err(OvsError::OperationFailed(format!(
        "OVS iface-id `{}` must be used only by `{}`, found: {}",
        spec.iface_id,
        spec.port_name,
        names.join(", ")
    )))
}

fn delete_owned_with<C: OvsControl>(
    control: &C,
    port_name: &str,
    owner_id: &str,
) -> Result<(), OvsError> {
    let mut retry = ConflictRetry::new();
    loop {
        let Some(info) = control.inspect_owned(port_name)? else {
            return Ok(());
        };
        info.validate_owned(port_name, owner_id)?;
        match control.delete_owned_port(&info, owner_id) {
            Ok(()) => return Ok(()),
            Err(error @ OvsError::TransactionConflict(_)) => retry.after_conflict(error)?,
            Err(error) => return Err(error),
        }
    }
}

fn cleanup_failed_create<C: OvsControl>(
    control: &C,
    spec: &OvsPortSpec,
    error: OvsError,
) -> OvsError {
    match delete_owned_with(control, &spec.port_name, &spec.owner_id) {
        Ok(()) => error,
        Err(cleanup_error) => OvsError::OperationFailed(format!(
            "failed to create OVS port `{}` ({error}); cleanup also failed ({cleanup_error})",
            spec.port_name
        )),
    }
}

/// Retries only zero-timeout guard conflicts, whose transactions did not commit.
struct ConflictRetry {
    attempts: usize,
    deadline: Instant,
}

impl ConflictRetry {
    fn new() -> Self {
        Self {
            attempts: 1,
            deadline: Instant::now() + CONFLICT_RETRY_TIMEOUT,
        }
    }

    fn after_conflict(&mut self, error: OvsError) -> Result<(), OvsError> {
        if self.attempts >= MAX_TRANSACTION_ATTEMPTS || Instant::now() >= self.deadline {
            return Err(OvsError::TransactionConflict(format!(
                "retry budget exhausted after {} attempt(s): {error}",
                self.attempts
            )));
        }
        let delay = Duration::from_millis(2 << (self.attempts - 1).min(5));
        thread::sleep(delay.min(self.deadline.saturating_duration_since(Instant::now())));
        self.attempts += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    //! Ownership and recovery contracts, exercised without a host OVS daemon.

    use std::collections::{BTreeMap, VecDeque};
    use std::sync::{Arc, Mutex};

    use crate::{CreateOutcome, OvsBinding, OvsControl, OvsError, OvsPortInfo, OvsPortSpec};

    #[derive(Clone, Default)]
    struct Switch(Arc<Mutex<SwitchState>>);

    #[derive(Default)]
    struct SwitchState {
        port: Option<OvsPortInfo>,
        rival: Option<String>,
        adds: usize,
        deletes: usize,
        add_errors: VecDeque<OvsError>,
        delete_errors: VecDeque<OvsError>,
        commit_before_error: bool,
        readiness_error: bool,
    }

    fn spec() -> OvsPortSpec {
        OvsPortSpec {
            bridge_name: "br-int".into(),
            port_name: "kdt-attachment".into(),
            interface_type: String::new(),
            options: BTreeMap::from([("managed-option".into(), "value".into())]),
            mtu_request: Some(1500),
            iface_id: "pod.namespace.ovn".into(),
            encapsulation_ip: Some("192.0.2.10".parse().expect("IP")),
            owner_id: "attachment-owner".into(),
        }
    }

    fn port(spec: &OvsPortSpec) -> OvsPortInfo {
        OvsPortInfo {
            port_uuid: "00000000-0000-0000-0000-000000000001".into(),
            interface_uuid: "00000000-0000-0000-0000-000000000002".into(),
            bridge_uuid: "00000000-0000-0000-0000-000000000003".into(),
            bridge_name: spec.bridge_name.clone(),
            port_name: spec.port_name.clone(),
            port_owner: Some(spec.owner_id.clone()),
            interface_owner: Some(spec.owner_id.clone()),
            iface_id: Some(spec.iface_id.clone()),
            encapsulation_ip: spec.encapsulation_ip,
            ovn_installed: true,
            interface_type: spec.interface_type.clone(),
            options: spec.options.clone(),
            mtu_request: spec.mtu_request,
            ofport: Some(12),
            interface_error: None,
        }
    }

    impl OvsControl for Switch {
        fn inspect(&self, _: &str) -> Result<Option<OvsPortInfo>, OvsError> {
            Ok(self.0.lock().expect("switch lock").port.clone())
        }

        fn interface_ready(&self, _: &str) -> Result<bool, OvsError> {
            if self.0.lock().expect("switch lock").readiness_error {
                Err(OvsError::OperationFailed("OVN rejected the binding".into()))
            } else {
                Ok(true)
            }
        }

        fn interfaces_with_iface_id(&self, _: &str) -> Result<Vec<String>, OvsError> {
            let state = self.0.lock().expect("switch lock");
            Ok(state
                .port
                .iter()
                .map(|port| port.port_name.clone())
                .chain(state.rival.clone())
                .collect())
        }

        fn add_port(&self, spec: &OvsPortSpec) -> Result<(), OvsError> {
            let mut state = self.0.lock().expect("switch lock");
            state.adds += 1;
            let error = state.add_errors.pop_front();
            if error.is_none() || state.commit_before_error {
                state.port = Some(port(spec));
            }
            error.map_or(Ok(()), Err)
        }

        fn delete_owned_port(&self, snapshot: &OvsPortInfo, owner: &str) -> Result<(), OvsError> {
            let mut state = self.0.lock().expect("switch lock");
            state.deletes += 1;
            if let Some(error) = state.delete_errors.pop_front() {
                return Err(error);
            }
            assert_eq!(state.port.as_ref(), Some(snapshot));
            assert_eq!(snapshot.port_owner.as_deref(), Some(owner));
            assert_eq!(snapshot.interface_owner.as_deref(), Some(owner));
            state.port = None;
            Ok(())
        }
    }

    #[test]
    fn add_idempotent() {
        let switch = Switch::default();
        let binding = OvsBinding::new(switch.clone());
        let spec = spec();
        assert_eq!(
            binding.create_or_validate(&spec),
            Ok(CreateOutcome::Created)
        );
        switch
            .0
            .lock()
            .expect("switch lock")
            .port
            .as_mut()
            .expect("created port")
            .options
            .insert("unmanaged-option".into(), "preserve-me".into());
        assert_eq!(
            binding.create_or_validate(&spec),
            Ok(CreateOutcome::AlreadyPresent)
        );
        let state = switch.0.lock().expect("switch lock");
        assert_eq!(state.adds, 1);
        assert_eq!(state.deletes, 0);
        assert_eq!(state.port.as_ref().expect("port").options.len(), 2);
    }

    #[test]
    fn reject_port_mismatch() {
        let spec = spec();
        let mutations: [fn(&mut OvsPortInfo); 4] = [
            |port| port.port_owner = Some("someone-else".into()),
            |port| port.interface_owner = None,
            |port| port.mtu_request = Some(9000),
            |port| port.iface_id = Some("another-logical-port".into()),
        ];
        for mutate in mutations {
            let switch = Switch::default();
            let mut existing = port(&spec);
            mutate(&mut existing);
            switch.0.lock().expect("switch lock").port = Some(existing.clone());
            assert!(OvsBinding::new(switch.clone())
                .create_or_validate(&spec)
                .is_err());
            let state = switch.0.lock().expect("switch lock");
            assert_eq!((state.adds, state.deletes), (0, 0));
            assert_eq!(state.port, Some(existing));
        }
    }

    #[test]
    fn reject_iface_conflict() {
        let switch = Switch::default();
        switch.0.lock().expect("switch lock").rival = Some("ordinary-pod-veth".into());
        assert!(OvsBinding::new(switch.clone())
            .create_or_validate(&spec())
            .is_err());
        let state = switch.0.lock().expect("switch lock");
        assert_eq!((state.adds, state.deletes), (0, 0));
    }

    #[test]
    fn retry_transaction_conflicts() {
        let switch = Switch::default();
        switch.0.lock().expect("switch lock").add_errors = VecDeque::from([
            OvsError::TransactionConflict("concurrent snapshot".into()),
            OvsError::TransactionConflict("another concurrent snapshot".into()),
        ]);
        assert_eq!(
            OvsBinding::new(switch.clone()).create_or_validate(&spec()),
            Ok(CreateOutcome::Created)
        );
        assert_eq!(switch.0.lock().expect("switch lock").adds, 3);

        let uncertain = Switch::default();
        {
            let mut state = uncertain.0.lock().expect("switch lock");
            state
                .add_errors
                .push_back(OvsError::OperationFailed("socket disconnected".into()));
            state.commit_before_error = true;
        }
        assert!(matches!(
            OvsBinding::new(uncertain.clone()).create_or_validate(&spec()),
            Err(OvsError::OperationFailed(_))
        ));
        let state = uncertain.0.lock().expect("switch lock");
        assert_eq!((state.adds, state.deletes), (1, 1));
        assert!(state.port.is_none());
    }

    #[test]
    fn rollback_readiness_failure() {
        for existing in [false, true] {
            let switch = Switch::default();
            {
                let mut state = switch.0.lock().expect("switch lock");
                state.readiness_error = true;
                state.port = existing.then(|| port(&spec()));
            }
            assert!(OvsBinding::new(switch.clone())
                .create_or_validate(&spec())
                .is_err());
            let state = switch.0.lock().expect("switch lock");
            assert_eq!(state.deletes, usize::from(!existing));
            assert_eq!(state.port.is_some(), existing);
        }
    }

    #[test]
    fn delete_idempotent() {
        let switch = Switch::default();
        {
            let mut state = switch.0.lock().expect("switch lock");
            let mut stale = port(&spec());
            stale.interface_error = Some("datapath unavailable".into());
            stale.ofport = Some(-1);
            stale.interface_type = "unexpected".into();
            stale.iface_id = None;
            state.port = Some(stale);
            state.delete_errors.push_back(OvsError::TransactionConflict(
                "new snapshot required".into(),
            ));
        }
        let binding = OvsBinding::new(switch.clone());
        binding
            .delete_owned(&spec().port_name, &spec().owner_id)
            .expect("owned deletion");
        binding
            .delete_owned(&spec().port_name, &spec().owner_id)
            .expect("repeated deletion");
        let state = switch.0.lock().expect("switch lock");
        assert_eq!(state.deletes, 2);
        assert!(state.port.is_none());
    }

    #[test]
    fn delete_requires_ownership() {
        for port_owner in [true, false] {
            let switch = Switch::default();
            let mut existing = port(&spec());
            if port_owner {
                existing.port_owner = Some("foreign".into());
            } else {
                existing.interface_owner = Some("foreign".into());
            }
            switch.0.lock().expect("switch lock").port = Some(existing.clone());
            assert!(OvsBinding::new(switch.clone())
                .delete_owned(&spec().port_name, &spec().owner_id)
                .is_err());
            let state = switch.0.lock().expect("switch lock");
            assert_eq!(state.deletes, 0);
            assert_eq!(state.port, Some(existing));
        }
    }
}
