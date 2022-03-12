// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use vergen::{vergen, Config};

fn main() {
    // Generate the default 'cargo:' instruction output
    vergen(Config::default()).unwrap();
}
