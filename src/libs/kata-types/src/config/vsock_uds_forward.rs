// Copyright (c) 2026 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use std::io::{Error, ErrorKind, Result};
use std::path::Path;

const MIN_VSOCK_UDS_FORWARD_PORT: u32 = 1025;

/// Parse runtime configuration value `port:/absolute/unix/path`.
/// An empty value disables the feature (returns `Ok(None)`).
pub fn parse_vsock_uds_forward(val: &str) -> Result<Option<(u32, String)>> {
    let val = val.trim();
    if val.is_empty() {
        return Ok(None);
    }

    let (port_str, uds) = val.split_once(':').ok_or_else(|| {
        Error::new(
            ErrorKind::InvalidInput,
            format!("{val:?}: expected port:/absolute/unix/path"),
        )
    })?;

    let port: u32 = port_str.parse().map_err(|err| {
        Error::new(
            ErrorKind::InvalidInput,
            format!("{port_str:?}: invalid port: {err}"),
        )
    })?;

    if port < MIN_VSOCK_UDS_FORWARD_PORT {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("port {port} must be greater than 1024"),
        ));
    }

    if uds.is_empty() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "unix socket path must not be empty".to_string(),
        ));
    }

    if !Path::new(uds).is_absolute() {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            format!("unix socket path must be absolute: {uds:?}"),
        ));
    }

    Ok(Some((port, uds.to_string())))
}

/// Parse the first `vsock_uds_forward` list entry. An empty list disables the feature.
/// Additional entries are ignored until multi-forward support exists.
pub fn parse_vsock_uds_forward_list(vals: &[String]) -> Result<Option<(u32, String)>> {
    if vals.is_empty() {
        return Ok(None);
    }

    parse_vsock_uds_forward(vals[0].trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_vsock_uds_forward() {
        let cases = [
            ("empty disables", "", Ok(None)),
            (
                "valid",
                "1234:/tmp/foo.sock",
                Ok(Some((1234, "/tmp/foo.sock".to_string()))),
            ),
            (
                "port must be greater than 1024",
                "1024:/tmp/foo.sock",
                Err(()),
            ),
            ("relative path rejected", "1234:tmp/foo.sock", Err(())),
            (
                "path with colons",
                "5000:/tmp/a:b/c.sock",
                Ok(Some((5000, "/tmp/a:b/c.sock".to_string()))),
            ),
        ];

        for (name, val, want) in cases {
            let got = parse_vsock_uds_forward(val);
            match want {
                Ok(None) => assert!(got.as_ref().unwrap().is_none(), "{}", name),
                Ok(Some((port, uds))) => {
                    let got = got.unwrap().unwrap();
                    assert_eq!(got, (port, uds), "{}", name);
                }
                Err(()) => assert!(got.is_err(), "{} expected error", name),
            }
        }
    }

    #[test]
    fn test_parse_vsock_uds_forward_list() {
        assert!(parse_vsock_uds_forward_list(&[]).unwrap().is_none());

        let got = parse_vsock_uds_forward_list(&["1234:/tmp/foo.sock".to_string()])
            .unwrap()
            .unwrap();
        assert_eq!(got, (1234, "/tmp/foo.sock".to_string()));
    }
}
