// Copyright 2019 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: MIT OR Apache-2.0

#[cfg(test)]
mod tests {
    use crate::editor::RepositoryEditor;
    use crate::key_source::LocalKeySource;
    use crate::schema::{Signed, Snapshot, Target, Targets, Timestamp};
    use crate::TargetName;
    use chrono::{Duration, Utc};
    use std::num::NonZeroU64;
    use std::path::PathBuf;

    // Path to the root.json in the reference implementation
    fn tuf_root_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join("tuf-reference-impl")
            .join("metadata")
            .join("root.json")
    }

    // Path to the root.json that corresponds with snakeoil.pem
    fn root_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join("simple-rsa")
            .join("root.json")
    }

    fn key_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join("snakeoil.pem")
    }

    // Path to fake targets in the reference implementation
    fn targets_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("data")
            .join("tuf-reference-impl")
            .join("targets")
    }

    // Make sure we can't create a repo without any data
    #[test]
    fn empty_repository() {
        let root_key = key_path();
        let key_source = LocalKeySource { path: root_key };
        let root_path = root_path();

        let editor = RepositoryEditor::new(&root_path).unwrap();
        assert!(editor.sign(&[Box::new(key_source)]).is_err());
    }

    // Make sure we can add targets from different sources
    #[allow(clippy::similar_names)]
    #[test]
    fn add_targets_from_multiple_sources() {
        let targets: Signed<Targets> = serde_json::from_str(include_str!(
            "../../tests/data/tuf-reference-impl/metadata/targets.json"
        ))
        .unwrap();
        let target3_path = targets_path().join("file3.txt");
        let target2_path = targets_path().join("file2.txt");
        // Use file2.txt to create a "new" target
        let target4 = Target::from_path(target2_path).unwrap();
        let root_path = tuf_root_path();

        let mut editor = RepositoryEditor::new(&root_path).unwrap();
        editor
            .targets(targets)
            .unwrap()
            .add_target(TargetName::new("file4.txt").unwrap(), target4)
            .unwrap()
            .add_target_path(target3_path)
            .unwrap();
    }

    #[allow(clippy::similar_names)]
    #[test]
    fn clear_targets() {
        let targets: Signed<Targets> = serde_json::from_str(include_str!(
            "../../tests/data/tuf-reference-impl/metadata/targets.json"
        ))
        .unwrap();
        let target3 = targets_path().join("file3.txt");
        let root_path = tuf_root_path();

        let mut editor = RepositoryEditor::new(&root_path).unwrap();
        editor
            .targets(targets)
            .unwrap()
            .add_target_path(target3)
            .unwrap()
            .clear_targets()
            .unwrap();
    }

    // Create and fully sign a repo
    #[test]
    fn complete_repository() {
        let root = root_path();
        let root_key = key_path();
        let key_source = LocalKeySource { path: root_key };
        let timestamp_expiration = Utc::now().checked_add_signed(Duration::days(3)).unwrap();
        let timestamp_version = NonZeroU64::new(1234).unwrap();
        let snapshot_expiration = Utc::now().checked_add_signed(Duration::days(21)).unwrap();
        let snapshot_version = NonZeroU64::new(5432).unwrap();
        let targets_expiration = Utc::now().checked_add_signed(Duration::days(13)).unwrap();
        let targets_version = NonZeroU64::new(789).unwrap();
        let target1 = targets_path().join("file1.txt");
        let target2 = targets_path().join("file2.txt");
        let target3 = targets_path().join("file3.txt");
        let target_list = vec![target1, target2, target3];

        let mut editor = RepositoryEditor::new(&root).unwrap();
        editor
            .targets_expires(targets_expiration)
            .unwrap()
            .targets_version(targets_version)
            .unwrap()
            .snapshot_expires(snapshot_expiration)
            .snapshot_version(snapshot_version)
            .timestamp_expires(timestamp_expiration)
            .timestamp_version(timestamp_version)
            .add_target_paths(target_list)
            .unwrap();
        assert!(editor.sign(&[Box::new(key_source)]).is_ok());
    }

    // Make sure we can add existing role structs and the proper data is kept.
    #[test]
    fn existing_roles() {
        let targets: Signed<Targets> = serde_json::from_str(include_str!(
            "../../tests/data/tuf-reference-impl/metadata/targets.json"
        ))
        .unwrap();
        let snapshot: Signed<Snapshot> = serde_json::from_str(include_str!(
            "../../tests/data/tuf-reference-impl/metadata/snapshot.json"
        ))
        .unwrap();
        let timestamp: Signed<Timestamp> = serde_json::from_str(include_str!(
            "../../tests/data/tuf-reference-impl/metadata/timestamp.json"
        ))
        .unwrap();
        let root_path = tuf_root_path();

        let mut editor = RepositoryEditor::new(&root_path).unwrap();
        editor
            .targets(targets)
            .unwrap()
            .snapshot(snapshot.signed)
            .unwrap()
            .timestamp(timestamp.signed)
            .unwrap();

        assert!(editor.snapshot_version.is_none());
        assert!(editor.timestamp_version.is_none());

        assert!(editor.snapshot_expires.is_none());
        assert!(editor.timestamp_expires.is_none());
    }
}
