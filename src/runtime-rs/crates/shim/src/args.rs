// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use kata_sys_util::validate;

use crate::Error;

/// Received command-line arguments or environment arguments
/// from a shimv2 container manager such as containerd.
///
/// For detailed information, please refer to the
/// [shim spec](https://github.com/containerd/containerd/blob/v1.6.8/runtime/v2/README.md).
#[derive(Debug, Default, Clone)]
pub struct Args {
    /// the id of the container
    pub id: String,
    /// the namespace for the container
    pub namespace: String,
    /// the address of the containerd's main socket
    pub address: String,
    /// the binary path to publish events back to containerd
    pub publish_binary: String,
    /// the path to the bundle to delete
    pub bundle: String,
    /// Whether or not to enable debug
    pub debug: bool,
}

impl Args {
    /// Check the shim argument object is vaild or not.
    ///
    /// The id, namespace, address and publish_binary are mandatory for START, RUN and DELETE.
    /// And bundle is mandatory for DELETE.
    pub fn validate(&mut self, should_check_bundle: bool) -> Result<()> {
        if self.id.is_empty() || self.namespace.is_empty() || self.publish_binary.is_empty() {
            return Err(anyhow!(Error::ArgumentIsEmpty(format!(
                "id: {} namespace: {} address: {} publish_binary: {}",
                &self.id, &self.namespace, &self.address, &self.publish_binary
            ))));
        }

        validate::verify_id(&self.id).context("verify container id")?;
        validate::verify_id(&self.namespace).context("verify namespace")?;

        // Ensure `bundle` is a valid path.
        if should_check_bundle {
            if self.bundle.is_empty() {
                return Err(anyhow!(Error::ArgumentIsEmpty("bundle".to_string())));
            }

            let path = PathBuf::from(self.bundle.clone())
                .canonicalize()
                .map_err(|_| Error::InvalidArgument)?;
            let md = path
                .metadata()
                .map_err(|_| Error::InvalidArgument)
                .context("get address metadata")?;
            if !md.is_dir() {
                return Err(Error::InvalidArgument).context("medata is dir");
            }
            self.bundle = path
                .to_str()
                .map(|v| v.to_owned())
                .ok_or(Error::InvalidArgument)
                .context("path to string")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    use anyhow::anyhow;
    use kata_sys_util::validate;

    #[test]
    fn test_args_is_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let path = path.to_str().unwrap();
        let bind_address = &format!("{}/socket1", path);
        UnixListener::bind(bind_address).unwrap();

        #[derive(Debug)]
        struct TestData {
            arg: Args,
            should_check_bundle: bool,
            result: Result<()>,
        }

        let default_id = "default_id".to_string();
        let default_namespace = "default_namespace".to_string();
        let default_address = bind_address.to_string();
        let default_publish_binary = "containerd".to_string();
        let default_bundle = path.to_string();

        let mut arg = Args {
            id: default_id.clone(),
            namespace: default_namespace.clone(),
            address: default_address.clone(),
            publish_binary: default_publish_binary.clone(),
            bundle: default_bundle.clone(),
            ..Default::default()
        };

        let tests = &[
            TestData {
                arg: arg.clone(),
                should_check_bundle: false,
                result: Ok(()),
            },
            TestData {
                arg: {
                    arg.namespace = "".to_string();
                    arg.clone()
                },
                should_check_bundle: false,
                result: Err(anyhow!(Error::ArgumentIsEmpty(format!(
                    "id: {} namespace: {} address: {} publish_binary: {}",
                    &arg.id, &arg.namespace, &arg.address, &arg.publish_binary
                )))),
            },
            TestData {
                arg: {
                    arg.namespace = default_namespace.clone();
                    arg.clone()
                },
                should_check_bundle: false,
                result: Ok(()),
            },
            TestData {
                arg: {
                    arg.id = "".to_string();
                    arg.clone()
                },
                should_check_bundle: false,
                result: Err(anyhow!(Error::ArgumentIsEmpty(format!(
                    "id: {} namespace: {} address: {} publish_binary: {}",
                    &arg.id, &arg.namespace, &arg.address, &arg.publish_binary
                )))),
            },
            TestData {
                arg: {
                    arg.id = default_id;
                    arg.clone()
                },
                should_check_bundle: false,
                result: Ok(()),
            },
            TestData {
                arg: {
                    arg.address = "".to_string();
                    arg.clone()
                },
                should_check_bundle: false,
                result: Ok(()),
            },
            TestData {
                arg: {
                    arg.address = default_address.clone();
                    arg.clone()
                },
                should_check_bundle: false,
                result: Ok(()),
            },
            TestData {
                arg: {
                    arg.publish_binary = "".to_string();
                    arg.clone()
                },
                should_check_bundle: false,
                result: Err(anyhow!(Error::ArgumentIsEmpty(format!(
                    "id: {} namespace: {} address: {} publish_binary: {}",
                    &arg.id, &arg.namespace, &arg.address, &arg.publish_binary
                )))),
            },
            TestData {
                arg: {
                    arg.publish_binary = default_publish_binary;
                    arg.clone()
                },
                should_check_bundle: false,
                result: Ok(()),
            },
            TestData {
                arg: {
                    arg.bundle = "".to_string();
                    arg.clone()
                },
                should_check_bundle: false,
                result: Ok(()),
            },
            TestData {
                arg: arg.clone(),
                should_check_bundle: true,
                result: Err(anyhow!(Error::ArgumentIsEmpty("bundle".to_string()))),
            },
            TestData {
                arg: {
                    arg.bundle = default_bundle;
                    arg.clone()
                },
                should_check_bundle: true,
                result: Ok(()),
            },
            TestData {
                arg: {
                    arg.namespace = "id1/id2".to_string();
                    arg.clone()
                },
                should_check_bundle: true,
                result: Err(
                    anyhow!(validate::Error::InvalidContainerID("id/id2".to_string()))
                        .context("verify namespace"),
                ),
            },
            TestData {
                arg: {
                    arg.namespace = default_namespace.clone() + "id1 id2";
                    arg.clone()
                },
                should_check_bundle: true,
                result: Err(anyhow!(validate::Error::InvalidContainerID(
                    default_namespace.clone() + "id1 id2",
                ))
                .context("verify namespace")),
            },
            TestData {
                arg: {
                    arg.namespace = default_namespace.clone() + "id2\tid2";
                    arg.clone()
                },
                should_check_bundle: true,
                result: Err(anyhow!(validate::Error::InvalidContainerID(
                    default_namespace.clone() + "id1\tid2",
                ))
                .context("verify namespace")),
            },
            TestData {
                arg: {
                    arg.namespace = default_namespace;
                    arg.clone()
                },
                should_check_bundle: true,
                result: Ok(()),
            },
            TestData {
                arg: {
                    arg.address = default_address;
                    arg
                },
                should_check_bundle: true,
                result: Ok(()),
            },
        ];

        for (i, t) in tests.iter().enumerate() {
            let msg = format!("test[{}]: {:?}", i, t);
            let should_check_bundle = t.should_check_bundle;
            let result = t.arg.clone().validate(should_check_bundle);
            let msg = format!("{}, result: {:?}", msg, result);

            if t.result.is_ok() {
                assert!(result.is_ok(), "{}", msg);
            } else {
                let expected_error = format!("{}", t.result.as_ref().unwrap_err());
                let actual_error = format!("{}", result.unwrap_err());
                assert!(actual_error == expected_error, "{}", msg);
            }
        }
    }
}
