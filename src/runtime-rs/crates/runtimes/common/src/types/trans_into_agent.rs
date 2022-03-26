// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::From;

use agent;

use super::{ContainerID, ContainerProcess};

impl From<ContainerID> for agent::ContainerID {
    fn from(from: ContainerID) -> Self {
        Self {
            container_id: from.container_id,
        }
    }
}

impl From<ContainerProcess> for agent::ContainerProcessID {
    fn from(from: ContainerProcess) -> Self {
        Self {
            container_id: from.container_id.into(),
            exec_id: from.exec_id,
        }
    }
}
