// Copyright (c) 2025 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use hypervisor::BlockConfig;
use kata_types::build_path;

/// The path /run/kata-containers/shared/initdata, combined with the sandbox ID,
/// will form the default directory for storing the initdata image.
pub const DEFAULT_KATA_SHARED_INIT_DATA_PATH: &str = "/run/kata-containers/shared/initdata";

/// kata initdata image
pub const KATA_INIT_DATA_IMAGE: &str = "initdata.image";

/// InitDataConfig which is a tuple of Block Device Config and its digest of the encoded
/// string included in the disk. And, both of them will come up at the same time.
#[derive(Clone, Debug)]
pub struct InitDataConfig(pub BlockConfig, pub String);

/// The path /run/kata-containers/shared/initdata, combined with the sandbox ID,
/// will form the directory for storing the initdata image.
/// The directory will be prefixed with the rootless directory when running in rootless mode
pub fn kata_shared_init_data_path() -> String {
    build_path(DEFAULT_KATA_SHARED_INIT_DATA_PATH)
}
