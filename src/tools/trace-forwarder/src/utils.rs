// Copyright (c) 2021 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};

// Request for "any CID"
pub const VSOCK_CID_ANY_STR: &str = "any";
// Numeric equivalent to VSOCK_CID_ANY_STR
pub const VSOCK_CID_ANY: u32 = libc::VMADDR_CID_ANY;

pub const ERR_VSOCK_PORT_EMPTY: &str = "VSOCK port cannot be empty";
pub const ERR_VSOCK_PORT_NOT_NUMERIC: &str = "VSOCK port number must be an integer";
pub const ERR_VSOCK_PORT_ZERO: &str = "VSOCK port number cannot be zero";

pub const ERR_VSOCK_CID_EMPTY: &str = "VSOCK CID cannot be empty";
pub const ERR_VSOCK_CID_NOT_NUMERIC: &str = "VSOCK CID must be an integer";

pub const ERR_HVSOCK_SOC_PATH_EMPTY: &str = "Hybrid VSOCK socket path cannot be empty";

// Parameters:
//
// 1: expected Result
// 2: actual Result
// 3: string used to identify the test on error
#[macro_export]
macro_rules! assert_result {
    ($expected_result:expr, $actual_result:expr, $msg:expr) => {
        if $expected_result.is_ok() {
            let expected_level = $expected_result.as_ref().unwrap();
            let actual_level = $actual_result.unwrap();
            assert!(*expected_level == actual_level, "{}", $msg);
        } else {
            let expected_error = $expected_result.as_ref().unwrap_err();
            let expected_error_msg = format!("{:?}", expected_error);

            if let Err(actual_error) = $actual_result {
                let actual_error_msg = format!("{:?}", actual_error);

                assert!(expected_error_msg == actual_error_msg, "{}", $msg);
            } else {
                assert!(expected_error_msg == "expected error, got OK", "{}", $msg);
            }
        }
    };
}

// Create a Hybrid VSOCK path from the specified socket, appending either
// the user specified port or the default port.
pub fn make_hybrid_socket_path(
    socket_path: &str,
    user_port: Option<&str>,
    default_port: &str,
) -> Result<String> {
    if socket_path.is_empty() {
        return Err(anyhow!(ERR_HVSOCK_SOC_PATH_EMPTY));
    }

    let port_str = if let Some(user_port) = user_port {
        user_port
    } else {
        default_port
    };

    let port = port_str_to_port(port_str)?;

    let full_path = format!("{}_{}", socket_path, port);

    Ok(full_path)
}

// Convert a string to a VSOCK CID value.
pub fn str_to_vsock_cid(cid: Option<&str>) -> Result<u32> {
    let cid_str = if let Some(cid) = cid {
        cid
    } else {
        VSOCK_CID_ANY_STR
    };

    let cid: u32 = match cid_str {
        VSOCK_CID_ANY_STR => Ok(VSOCK_CID_ANY),
        "" => return Err(anyhow!(ERR_VSOCK_CID_EMPTY)),
        _ => cid_str
            .parse::<u32>()
            .map_err(|_| anyhow!(ERR_VSOCK_CID_NOT_NUMERIC)),
    }?;

    Ok(cid)
}

// Convert a user specified VSOCK port number string into a VSOCK port number,
// or use the default value if not specified.
pub fn str_to_vsock_port(port: Option<&str>, default_port: &str) -> Result<u32> {
    let port_str = if let Some(port) = port {
        port
    } else {
        default_port
    };

    let port = port_str_to_port(port_str)?;

    Ok(port)
}

// Convert a string port value into a numeric value.
fn port_str_to_port(port: &str) -> Result<u32> {
    if port.is_empty() {
        return Err(anyhow!(ERR_VSOCK_PORT_EMPTY));
    }

    let port: u32 = port
        .parse::<u32>()
        .map_err(|_| anyhow!(ERR_VSOCK_PORT_NOT_NUMERIC))?;

    if port == 0 {
        return Err(anyhow!(ERR_VSOCK_PORT_ZERO));
    }

    Ok(port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_str_to_port() {
        #[derive(Debug)]
        struct TestData<'a> {
            port: &'a str,
            result: Result<u32>,
        }

        let tests = &[
            TestData {
                port: "",
                result: Err(anyhow!(ERR_VSOCK_PORT_EMPTY)),
            },
            TestData {
                port: "a",
                result: Err(anyhow!(ERR_VSOCK_PORT_NOT_NUMERIC)),
            },
            TestData {
                port: "foo bar",
                result: Err(anyhow!(ERR_VSOCK_PORT_NOT_NUMERIC)),
            },
            TestData {
                port: "1 bar",
                result: Err(anyhow!(ERR_VSOCK_PORT_NOT_NUMERIC)),
            },
            TestData {
                port: "0",
                result: Err(anyhow!(ERR_VSOCK_PORT_ZERO)),
            },
            TestData {
                port: "2",
                result: Ok(2),
            },
            TestData {
                port: "12345",
                result: Ok(12345),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = port_str_to_port(d.port);

            let msg = format!("{}: result: {:?}", msg, result);

            assert_result!(d.result, result, msg);
        }
    }

    #[test]
    fn test_str_to_vsock_port() {
        #[derive(Debug)]
        struct TestData<'a> {
            port: Option<&'a str>,
            default_port: &'a str,
            result: Result<u32>,
        }

        let tests = &[
            TestData {
                port: None,
                default_port: "",
                result: Err(anyhow!(ERR_VSOCK_PORT_EMPTY)),
            },
            TestData {
                port: None,
                default_port: "foo",
                result: Err(anyhow!(ERR_VSOCK_PORT_NOT_NUMERIC)),
            },
            TestData {
                port: None,
                default_port: "1 foo",
                result: Err(anyhow!(ERR_VSOCK_PORT_NOT_NUMERIC)),
            },
            TestData {
                port: None,
                default_port: "0",
                result: Err(anyhow!(ERR_VSOCK_PORT_ZERO)),
            },
            TestData {
                port: None,
                default_port: "1234",
                result: Ok(1234),
            },
            TestData {
                port: Some(""),
                default_port: "1234",
                result: Err(anyhow!(ERR_VSOCK_PORT_EMPTY)),
            },
            TestData {
                port: Some("1 foo"),
                default_port: "1234",
                result: Err(anyhow!(ERR_VSOCK_PORT_NOT_NUMERIC)),
            },
            TestData {
                port: Some("0"),
                default_port: "1234",
                result: Err(anyhow!(ERR_VSOCK_PORT_ZERO)),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = str_to_vsock_port(d.port, d.default_port);

            let msg = format!("{}: result: {:?}", msg, result);

            assert_result!(d.result, result, msg);
        }
    }

    #[test]
    fn test_str_to_vsock_cid() {
        #[derive(Debug)]
        struct TestData<'a> {
            cid: Option<&'a str>,
            result: Result<u32>,
        }

        let tests = &[
            TestData {
                cid: None,
                result: Ok(VSOCK_CID_ANY),
            },
            TestData {
                cid: Some(VSOCK_CID_ANY_STR),
                result: Ok(VSOCK_CID_ANY),
            },
            TestData {
                cid: Some(""),
                result: Err(anyhow!(ERR_VSOCK_CID_EMPTY)),
            },
            TestData {
                cid: Some("foo"),
                result: Err(anyhow!(ERR_VSOCK_CID_NOT_NUMERIC)),
            },
            TestData {
                cid: Some("1 foo"),
                result: Err(anyhow!(ERR_VSOCK_CID_NOT_NUMERIC)),
            },
            TestData {
                cid: Some("123"),
                result: Ok(123),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = str_to_vsock_cid(d.cid);

            let msg = format!("{}: result: {:?}", msg, result);

            assert_result!(d.result, result, msg);
        }
    }

    #[test]
    fn test_make_hybrid_socket_path() {
        #[derive(Debug)]
        struct TestData<'a> {
            socket_path: &'a str,
            user_port: Option<&'a str>,
            default_port: &'a str,
            result: Result<String>,
        }

        let tests = &[
            TestData {
                socket_path: "",
                user_port: None,
                default_port: "",
                result: Err(anyhow!(ERR_HVSOCK_SOC_PATH_EMPTY)),
            },
            TestData {
                socket_path: "/foo",
                user_port: None,
                default_port: "",
                result: Err(anyhow!(ERR_VSOCK_PORT_EMPTY)),
            },
            TestData {
                socket_path: "/foo",
                user_port: None,
                default_port: "1 foo",
                result: Err(anyhow!(ERR_VSOCK_PORT_NOT_NUMERIC)),
            },
            TestData {
                socket_path: "/foo",
                user_port: None,
                default_port: "0",
                result: Err(anyhow!(ERR_VSOCK_PORT_ZERO)),
            },
            TestData {
                socket_path: "/foo",
                user_port: None,
                default_port: "1",
                result: Ok("/foo_1".into()),
            },
            TestData {
                socket_path: "/foo",
                user_port: Some(""),
                default_port: "1",
                result: Err(anyhow!(ERR_VSOCK_PORT_EMPTY)),
            },
            TestData {
                socket_path: "/foo",
                user_port: Some("1 foo"),
                default_port: "1",
                result: Err(anyhow!(ERR_VSOCK_PORT_NOT_NUMERIC)),
            },
            TestData {
                socket_path: "/foo",
                user_port: Some("2"),
                default_port: "1",
                result: Ok("/foo_2".into()),
            },
        ];

        for (i, d) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, d);

            let result = make_hybrid_socket_path(d.socket_path, d.user_port, d.default_port);

            let msg = format!("{}: result: {:?}", msg, result);

            assert_result!(d.result, result, msg);
        }
    }
}
