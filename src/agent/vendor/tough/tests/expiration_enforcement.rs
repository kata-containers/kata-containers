// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs::File;
use test_utils::{dir_url, test_data};
use tough::error::Error::ExpiredMetadata;
use tough::schema::RoleType;
use tough::{ExpirationEnforcement, RepositoryLoader};

mod test_utils;

/// Test that `tough` fails to load an expired repository when `expiration_enforcement` is `Safe`.
///
#[test]
fn test_expiration_enforcement_safe() {
    let base = test_data().join("expired-repository");

    let result = RepositoryLoader::new(
        File::open(base.join("metadata").join("1.root.json")).unwrap(),
        dir_url(base.join("metadata")),
        dir_url(base.join("targets")),
    )
    .load();
    if let Err(err) = result {
        match err {
            ExpiredMetadata { role, backtrace: _ } => {
                assert_eq!(role, RoleType::Timestamp);
            }
            _ => {
                panic!(
                    "Expected an error type of 'ExpiredMetadata' but received a different error."
                );
            }
        }
    } else {
        panic!("Repository::load was expected to return an error.")
    }
}

/// Test that `tough` loads an expired repository when `expiration_enforcement` is `Unsafe`.
///
#[test]
fn test_expiration_enforcement_unsafe() {
    let base = test_data().join("expired-repository");
    let result = RepositoryLoader::new(
        File::open(base.join("metadata").join("1.root.json")).unwrap(),
        dir_url(base.join("metadata")),
        dir_url(base.join("targets")),
    )
    .expiration_enforcement(ExpirationEnforcement::Unsafe)
    .load();
    assert!(result.is_ok())
}
