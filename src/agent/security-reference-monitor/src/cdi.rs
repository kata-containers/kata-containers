// Copyright (c) 2026 Kata Containers community
//
// SPDX-License-Identifier: Apache-2.0

//! FR-11 — trusted device / CDI resolution.
//!
//! Container Device Interface (CDI) resolution applies `containerEdits` (environment
//! variables, device nodes, mounts, hooks) to the OCI spec from CDI spec files found in a
//! guest directory (e.g. `/var/run/cdi`). Those edits are applied *after* the create
//! request is authorized, and the spec files themselves may be produced by host-influenced
//! entities. A host can therefore smuggle privilege into a container by injecting a CDI
//! annotation and/or a CDI spec that the policy never authorized (the GPU instance of the
//! canonical-object gap).
//!
//! This module makes CDI resolution *trusted*: every CDI spec that contributes an injected
//! device must be **measured** — its content digest must appear in an authorized set of
//! measured manifests. Resolution is closed-door by default: if a container requests CDI
//! devices but no measured manifest authorizes them, the request is rejected rather than
//! silently applying host-arbitrary edits. Each authorized device is returned as a
//! [`VerifiedCdiDevice`] so the resolved handle can be bound to the container occurrence.

use std::collections::HashSet;
use std::fmt;

/// A CDI device requested by a container, split into its kind and device name.
/// For `nvidia.com/gpu=0` the kind is `nvidia.com/gpu` and the name is `0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdiDeviceRequest {
    pub kind: String,
    pub name: String,
}

impl CdiDeviceRequest {
    /// Parse a fully-qualified CDI device string (`<kind>=<name>`).
    pub fn parse(fqdn: &str) -> Option<Self> {
        let (kind, name) = fqdn.rsplit_once('=')?;
        if kind.is_empty() || name.is_empty() {
            return None;
        }
        Some(CdiDeviceRequest {
            kind: kind.to_string(),
            name: name.to_string(),
        })
    }

    fn fqdn(&self) -> String {
        format!("{}={}", self.kind, self.name)
    }
}

/// A CDI spec file available in the guest spec directory, with its measured content digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeasuredCdiSpec {
    pub path: String,
    /// The CDI `kind` declared by the spec (e.g. `nvidia.com/gpu`).
    pub kind: String,
    /// Content digest of the spec file (e.g. `sha256:...`).
    pub digest: String,
}

/// A CDI device whose providing spec has been verified as measured/trusted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedCdiDevice {
    /// Fully-qualified device string (`<kind>=<name>`).
    pub device: String,
    /// Digest of the measured spec that provides the device (binds device→content).
    pub spec_digest: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum CdiError {
    /// A container requested CDI devices but no measured manifest is authorized (the
    /// closed-door default): applying host-arbitrary CDI edits is refused.
    HostArbitraryCdi { device: String },
    /// A spec of the requested kind exists but its content digest is not in the
    /// authorized (measured) set — an unmeasured / tampered spec.
    UnmeasuredSpec {
        device: String,
        kind: String,
        found_digest: String,
    },
    /// No spec of the requested kind is available at all.
    UnsatisfiedRequest { device: String },
}

impl fmt::Display for CdiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CdiError::HostArbitraryCdi { device } => write!(
                f,
                "CDI device {device} requested but no measured CDI manifest is authorized \
                 (host-arbitrary CDI is refused in strict mode)"
            ),
            CdiError::UnmeasuredSpec {
                device,
                kind,
                found_digest,
            } => write!(
                f,
                "CDI device {device}: spec of kind {kind} has unmeasured digest {found_digest}"
            ),
            CdiError::UnsatisfiedRequest { device } => {
                write!(f, "CDI device {device}: no spec of its kind is available")
            }
        }
    }
}

impl std::error::Error for CdiError {}

/// Authorize a set of requested CDI devices against the measured spec files available in
/// the guest, using an authorized set of measured spec digests.
///
/// Returns the verified devices (device→providing-spec-digest) in request order, or the
/// first authorization failure. Resolution is closed-door: with an empty authorized set,
/// any requested CDI device is refused.
pub fn authorize_cdi(
    requested: &[CdiDeviceRequest],
    available_specs: &[MeasuredCdiSpec],
    authorized_digests: &HashSet<String>,
) -> Result<Vec<VerifiedCdiDevice>, CdiError> {
    let mut verified = Vec::with_capacity(requested.len());

    for req in requested {
        let device = req.fqdn();

        // Closed-door default: no measured manifests => refuse host-arbitrary CDI.
        if authorized_digests.is_empty() {
            return Err(CdiError::HostArbitraryCdi { device });
        }

        // Specs that declare the requested kind.
        let of_kind: Vec<&MeasuredCdiSpec> =
            available_specs.iter().filter(|s| s.kind == req.kind).collect();
        if of_kind.is_empty() {
            return Err(CdiError::UnsatisfiedRequest { device });
        }

        // At least one spec of the kind must be measured/authorized.
        match of_kind
            .iter()
            .find(|s| authorized_digests.contains(&s.digest))
        {
            Some(spec) => verified.push(VerifiedCdiDevice {
                device,
                spec_digest: spec.digest.clone(),
            }),
            None => {
                return Err(CdiError::UnmeasuredSpec {
                    device,
                    kind: req.kind.clone(),
                    found_digest: of_kind[0].digest.clone(),
                })
            }
        }
    }

    Ok(verified)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(kind: &str, digest: &str) -> MeasuredCdiSpec {
        MeasuredCdiSpec {
            path: format!("/var/run/cdi/{kind}.json"),
            kind: kind.to_string(),
            digest: digest.to_string(),
        }
    }
    fn auth(digests: &[&str]) -> HashSet<String> {
        digests.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parse_fqdn() {
        let r = CdiDeviceRequest::parse("nvidia.com/gpu=0").unwrap();
        assert_eq!(r.kind, "nvidia.com/gpu");
        assert_eq!(r.name, "0");
        assert!(CdiDeviceRequest::parse("no-equals").is_none());
        assert!(CdiDeviceRequest::parse("kind=").is_none());
    }

    #[test]
    fn no_cdi_requested_is_noop() {
        let v = authorize_cdi(&[], &[], &HashSet::new()).unwrap();
        assert!(v.is_empty());
    }

    /// TC4.2: an unsigned/unmeasured CDI spec is rejected.
    #[test]
    fn unmeasured_spec_is_rejected() {
        let req = vec![CdiDeviceRequest::parse("nvidia.com/gpu=0").unwrap()];
        let specs = vec![spec("nvidia.com/gpu", "sha256:HOSTARBITRARY")];
        let authorized = auth(&["sha256:TRUSTED"]);
        assert!(matches!(
            authorize_cdi(&req, &specs, &authorized).unwrap_err(),
            CdiError::UnmeasuredSpec { .. }
        ));
    }

    /// TC4.2: a measured/signed CDI spec is accepted; TC4.3: the resolved device carries
    /// the providing spec digest so it can be bound to the occurrence.
    #[test]
    fn measured_spec_is_accepted_and_bound() {
        let req = vec![CdiDeviceRequest::parse("nvidia.com/gpu=0").unwrap()];
        let specs = vec![spec("nvidia.com/gpu", "sha256:TRUSTED")];
        let authorized = auth(&["sha256:TRUSTED"]);
        let v = authorize_cdi(&req, &specs, &authorized).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].device, "nvidia.com/gpu=0");
        assert_eq!(v[0].spec_digest, "sha256:TRUSTED");
    }

    /// Closed-door default: CDI requested but no measured manifest authorized => refused.
    #[test]
    fn closed_door_refuses_host_arbitrary_cdi() {
        let req = vec![CdiDeviceRequest::parse("nvidia.com/gpu=0").unwrap()];
        let specs = vec![spec("nvidia.com/gpu", "sha256:WHATEVER")];
        assert!(matches!(
            authorize_cdi(&req, &specs, &HashSet::new()).unwrap_err(),
            CdiError::HostArbitraryCdi { .. }
        ));
    }

    #[test]
    fn unsatisfied_request_when_no_spec_of_kind() {
        let req = vec![CdiDeviceRequest::parse("nvidia.com/gpu=0").unwrap()];
        let specs = vec![spec("acme.com/nic", "sha256:TRUSTED")];
        let authorized = auth(&["sha256:TRUSTED"]);
        assert!(matches!(
            authorize_cdi(&req, &specs, &authorized).unwrap_err(),
            CdiError::UnsatisfiedRequest { .. }
        ));
    }
}
