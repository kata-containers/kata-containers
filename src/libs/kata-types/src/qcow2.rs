// Copyright (c) 2026 NVIDIA Corporation
//
// SPDX-License-Identifier: Apache-2.0
//
//! qcow2 description for direct-assigned volumes.
//!
//! The CSI driver owns image creation. Kata only validates and attaches what it
//! is handed: a qcow2 image, writable unless the mount is read-only.
//!
//! The format is carried in the `metadata` map of `mountInfo.json`:
//!
//! ```json
//! "device": "/var/lib/csi/vol1/disk.qcow2",
//! "metadata": {
//!   "io.katacontainers.volume.format": "qcow2"
//! }
//! ```
//!
//! The format is never probed from image contents, it is always taken from
//! this declaration.

use anyhow::{anyhow, Result};
use std::collections::HashMap;

/// `metadata` key holding the format of the volume's image. The keys are
/// namespaced because `metadata` is free-form and CSI drivers may already use
/// short names such as `format` for their own purposes.
pub const DIRECT_VOLUME_FORMAT_KEY: &str = "io.katacontainers.volume.format";

/// A qcow2 volume.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Qcow2Volume {}

/// Parse the qcow2 description of a direct volume out of its `metadata`.
///
/// Returns `Ok(None)` when the volume is not qcow2 (no `format` key, or
/// `raw`), which is the pre-existing behaviour.
pub fn parse_qcow2_volume(metadata: &HashMap<String, String>) -> Result<Option<Qcow2Volume>> {
    match metadata.get(DIRECT_VOLUME_FORMAT_KEY).map(String::as_str) {
        None | Some("raw") => Ok(None),
        Some("qcow2") => Ok(Some(Qcow2Volume {})),
        Some(other) => Err(anyhow!("unsupported volume format {:?}", other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FMT: &str = DIRECT_VOLUME_FORMAT_KEY;

    #[test]
    fn test_parse_qcow2_volume() {
        #[derive(Debug)]
        struct TestData {
            name: &'static str,
            metadata: Vec<(&'static str, String)>,
            // Ok: the parsed volume. Err: text the error must contain.
            result: std::result::Result<Option<Qcow2Volume>, String>,
        }
        let case = |name, metadata: &[(&'static str, &str)], result| TestData {
            name,
            metadata: metadata.iter().map(|(k, v)| (*k, v.to_string())).collect(),
            result,
        };
        let err = |text: &str| Err(text.to_string());

        let tests = [
            // Not qcow2: the pre-existing raw behaviour.
            case("no metadata", &[], Ok(None)),
            case("explicit raw", &[(FMT, "raw")], Ok(None)),
            // A CSI driver's own short keys are not ours, even with our values.
            case("foreign short keys", &[("format", "qcow2")], Ok(None)),
            // qcow2.
            case("qcow2", &[(FMT, "qcow2")], Ok(Some(Qcow2Volume {}))),
            // Format.
            case(
                "unknown format",
                &[(FMT, "vhd")],
                err(r#"unsupported volume format "vhd""#),
            ),
            case(
                "empty format",
                &[(FMT, "")],
                err(r#"unsupported volume format """#),
            ),
            case(
                "upper-case format",
                &[(FMT, "QCOW2")],
                err(r#"unsupported volume format "QCOW2""#),
            ),
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}] {}: {:?}", i, d.name, d);
            let metadata = d
                .metadata
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect();

            let result = parse_qcow2_volume(&metadata);
            let msg = format!("{}, result: {:?}", msg, result);

            match &d.result {
                Ok(expected) => assert_eq!(result.as_ref().ok(), Some(expected), "{}", msg),
                Err(expected) => {
                    let actual = format!("{:#}", result.as_ref().unwrap_err());
                    assert!(actual.contains(expected.as_str()), "{}", msg);
                }
            }
        }
    }
}
