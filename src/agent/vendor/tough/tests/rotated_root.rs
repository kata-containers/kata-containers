// Copyright 2020 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

mod test_utils;

use std::fs::File;
use test_utils::{dir_url, test_data};
use tough::RepositoryLoader;

#[test]
fn rotated_root() {
    let base = test_data().join("rotated-root");

    let repo = RepositoryLoader::new(
        File::open(base.join("1.root.json")).unwrap(),
        dir_url(&base),
        dir_url(base.join("targets")),
    )
    .load()
    .unwrap();

    assert_eq!(u64::from(repo.root().signed.version), 2);
}
