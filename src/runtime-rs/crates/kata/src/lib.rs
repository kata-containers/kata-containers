// Copyright (c) 2021 Alibaba Cloud
// Copyright (c) 2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::os::unix::fs::FileTypeExt;
use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("invalid argument")]
    InvalidArgument,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Received commandline arguments or environment arguments.
///
/// For defail information, please refer to the
/// [shim v2 spec](https://github.com/containerd/containerd/blob/main/runtime/v2/README.md).
#[derive(Debug, Default, Clone)]
pub struct ShimArgs {
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

impl ShimArgs {
    /// Check the shim argument object is vaild or not.
    ///
    /// The id, namespace, address and publish_binary are mandatory for START, RUN and DELETE.
    /// And bundle is mandatory for DELETE.
    pub fn validate(&mut self, is_delete: bool) -> Result<()> {
        if self.id.is_empty()
            || self.namespace.is_empty()
            || self.address.is_empty()
            || self.publish_binary.is_empty()
        {
            return Err(Error::InvalidArgument);
        }
        if is_delete && self.bundle.is_empty() {
            return Err(Error::InvalidArgument);
        }

        if Self::is_component_dangerous(&self.id) || Self::is_component_dangerous(&self.namespace) {
            return Err(Error::InvalidArgument);
        }

        // Ensure `address` is a valid path.
        let path = PathBuf::from(self.address.clone())
            .canonicalize()
            .map_err(|_| Error::InvalidArgument)?;
        let md = path.metadata().map_err(|_| Error::InvalidArgument)?;
        if !md.file_type().is_socket() {
            return Err(Error::InvalidArgument);
        }
        self.address = path
            .to_str()
            .map(|v| v.to_owned())
            .ok_or(Error::InvalidArgument)?;

        // Ensure `bundle` is a valid path.
        if !self.bundle.is_empty() {
            let path = PathBuf::from(self.bundle.clone())
                .canonicalize()
                .map_err(|_| Error::InvalidArgument)?;
            let md = path.metadata().map_err(|_| Error::InvalidArgument)?;
            if !md.is_dir() {
                return Err(Error::InvalidArgument);
            }
            self.bundle = path
                .to_str()
                .map(|v| v.to_owned())
                .ok_or(Error::InvalidArgument)?;
        }

        Ok(())
    }

    fn is_component_dangerous(comp: &str) -> bool {
        if comp.is_empty() {
            return true;
        }
        // only allow ascii alphanumeric character and '-', '_', '.' and '~'
        !comp
            .chars()
            .all(|x| matches!(x, '0'..='9' | 'A'..='Z' | 'a'..='z' | '-' | '_' | '.' | '~'))
    }
}

/// Command executor for shim.
#[allow(dead_code)]
pub struct ShimExecutor {
    args: ShimArgs,
}

impl ShimExecutor {
    /// Create a new instance of [`Shim`].
    pub fn new(args: ShimArgs) -> Self {
        ShimExecutor { args }
    }

    // implement start subcommand
    pub fn start(&mut self) {}

    // implement delete subcommand
    pub fn delete(&mut self) {}

    // implement rpc call from containerd
    pub fn run(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    #[test]
    fn test_args_is_valid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let path = path.to_str().unwrap();
        let bind_address = &format!("{}/socket1", path);
        UnixListener::bind(bind_address).unwrap();

        let mut arg = ShimArgs {
            id: "1dfc0567".to_string(),
            namespace: "ns1".to_owned(),
            address: bind_address.to_owned(),
            publish_binary: "containerd".to_string(),
            bundle: path.to_owned(),
            debug: false,
        };
        arg.validate(false).unwrap();

        arg.namespace = "".to_string();
        arg.validate(false).unwrap_err();
        arg.namespace = "ns1".to_owned();
        arg.validate(false).unwrap();

        arg.id = "".to_string();
        arg.validate(false).unwrap_err();
        arg.id = "1dfc0567".to_string();
        arg.validate(false).unwrap();

        arg.address = "".to_string();
        arg.validate(false).unwrap_err();
        arg.address = bind_address.to_owned();
        arg.validate(false).unwrap();

        arg.publish_binary = "".to_string();
        arg.validate(false).unwrap_err();
        arg.publish_binary = "containerd".to_string();
        arg.validate(false).unwrap();

        arg.bundle = "".to_string();
        arg.validate(false).unwrap();
        arg.validate(true).unwrap_err();
        arg.bundle = path.to_owned();

        arg.validate(true).unwrap();
        arg.namespace = "id1/id2".to_owned();
        arg.validate(true).unwrap_err();
        arg.namespace = path.to_owned() + "id1 id2";
        arg.validate(true).unwrap_err();
        arg.namespace = path.to_owned() + "id1\tid2";
        arg.validate(true).unwrap_err();
        arg.namespace = "1dfc0567".to_owned();

        arg.validate(true).unwrap();
        arg.namespace = "ns1/ns2".to_owned();
        arg.validate(true).unwrap_err();
        arg.namespace = path.to_owned() + "ns1 ns2";
        arg.validate(true).unwrap_err();
        arg.namespace = path.to_owned() + "ns1\tns2";
        arg.validate(true).unwrap_err();
        arg.namespace = "ns1".to_owned();

        arg.validate(true).unwrap();
        arg.address = bind_address.to_owned() + "/..";
        arg.validate(true).unwrap_err();
        arg.address = path.to_owned();
        arg.validate(true).unwrap_err();
        arg.address = format!("{}/././socket1", path);
        arg.validate(true).unwrap();
        assert_eq!(&arg.address, bind_address);
        arg.address = bind_address.to_owned();
        arg.validate(true).unwrap();

        arg.validate(true).unwrap();
        arg.bundle = path.to_owned() + "/test1";
        arg.validate(true).unwrap_err();
        arg.bundle = path.to_owned() + "/./.";
        arg.validate(true).unwrap();
        assert_eq!(&arg.bundle, path);
        arg.bundle = path.to_owned();
        arg.validate(false).unwrap();
    }

    #[test]
    fn test_is_component_dangerous() {
        assert!(ShimArgs::is_component_dangerous(""));
        assert!(ShimArgs::is_component_dangerous("../.."));
        assert!(ShimArgs::is_component_dangerous("é"));

        assert!(!ShimArgs::is_component_dangerous("avs098-09_8"));
    }
}
