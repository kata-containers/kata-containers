// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use crate::types::{ContainerProcess, TaskResponse};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed to find container {0}")]
    ContainerNotFound(String),
    #[error("failed to find process {0}")]
    ProcessNotFound(ContainerProcess),
    #[error("unexpected response {0} to shim {1}")]
    UnexpectedResponse(TaskResponse, String),
}
