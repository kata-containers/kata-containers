mod test_utils;

use chrono::{DateTime, TimeZone, Utc};
use maplit::hashmap;
use ring::rand::SystemRandom;
use std::collections::HashMap;
use std::fs::{self, create_dir_all, File};
use std::num::NonZeroU64;
use std::path::Path;
use tempfile::TempDir;
use test_utils::{dir_url, test_data, DATA_1, DATA_2, DATA_3};
use tough::editor::signed::SignedRole;
use tough::editor::RepositoryEditor;
use tough::key_source::{KeySource, LocalKeySource};
use tough::schema::{KeyHolder, PathPattern, PathSet, RoleKeys, RoleType, Root, Signed, Target};
use tough::{Prefix, RepositoryLoader, TargetName};

/// Returns a date in the future when Rust programs will no longer exist. `MAX_DATETIME` is so huge
/// that it serializes to something weird-looking, so we use something that is recognizable to
/// humans as a date.
fn later() -> DateTime<Utc> {
    Utc.ymd(2999, 1, 1).and_hms(0, 0, 0)
}

/// This test ensures that we can safely handle path-like target names with ../'s in them.
fn create_root(root_path: &Path, consistent_snapshot: bool) -> Vec<Box<dyn KeySource>> {
    let keys: Vec<Box<dyn KeySource>> = vec![Box::new(LocalKeySource {
        path: test_data().join("snakeoil.pem"),
    })];

    let key_pair = keys.get(0).unwrap().as_sign().unwrap().tuf_key();
    let key_id = key_pair.key_id().unwrap();

    let empty_keys = RoleKeys {
        keyids: vec![key_id.clone()],
        threshold: NonZeroU64::new(1).unwrap(),
        _extra: Default::default(),
    };

    let mut root = Signed {
        signed: Root {
            spec_version: "1.0.0".into(),
            consistent_snapshot,
            version: NonZeroU64::new(1).unwrap(),
            expires: later(),
            keys: HashMap::new(),
            roles: hashmap! {
                RoleType::Root => empty_keys.clone(),
                RoleType::Snapshot => empty_keys.clone(),
                RoleType::Targets => empty_keys.clone(),
                RoleType::Timestamp => empty_keys,
                // RoleType::DelegatedTargets => empty_keys.clone(),
            },
            _extra: HashMap::new(),
        },
        signatures: Vec::new(),
    };

    root.signed.keys.insert(key_id, key_pair);

    let signed_root = SignedRole::new(
        root.signed.clone(),
        &KeyHolder::Root(root.signed.clone()),
        &keys,
        &SystemRandom::new(),
    )
    .unwrap();

    std::fs::write(&root_path, signed_root.buffer()).unwrap();

    keys
}

#[test]
fn safe_target_paths() {
    let tempdir = TempDir::new().unwrap();
    let root_path = tempdir.path().join("root.json");
    let keys = create_root(&root_path, false);
    let one = NonZeroU64::new(1).unwrap();

    let mut editor = RepositoryEditor::new(&root_path).unwrap();
    editor
        .snapshot_version(one)
        .snapshot_expires(later())
        .timestamp_version(one)
        .timestamp_expires(later())
        .delegate_role(
            "delegated",
            &keys,
            PathSet::Paths(vec![PathPattern::new("delegated/*").unwrap()]),
            one,
            later(),
            one,
        )
        .unwrap();
    let repo_dir = tempdir.path().join("repo");
    let targets_dir = repo_dir.join("targets");
    fs::create_dir_all(targets_dir.join("foo/bar")).unwrap();
    fs::create_dir_all(targets_dir.join("delegated/subdir")).unwrap();
    let targets_file_1 = targets_dir.join("data1.txt");
    let targets_file_2 = targets_dir.join("foo/bar/data2.txt");
    let targets_file_3 = targets_dir.join("delegated/subdir/data3.txt");
    fs::write(&targets_file_1, DATA_1).unwrap();
    fs::write(&targets_file_2, DATA_2).unwrap();
    fs::write(&targets_file_3, DATA_3).unwrap();

    let target_name_1 = TargetName::new("foo/../bar/../baz/../../../../data1.txt").unwrap();
    let target_1 = Target::from_path(&targets_file_1).unwrap();
    let target_name_2 = TargetName::new("foo/bar/baz/../data2.txt").unwrap();
    let target_2 = Target::from_path(&targets_file_2).unwrap();
    let target_name_3 = TargetName::new("../delegated/foo/../subdir/data3.txt").unwrap();
    let target_3 = Target::from_path(&targets_file_3).unwrap();

    editor.add_target(target_name_1.clone(), target_1).unwrap();
    editor.add_target(target_name_2.clone(), target_2).unwrap();
    editor
        .targets_version(one)
        .unwrap()
        .targets_expires(later())
        .unwrap()
        .sign_targets_editor(&keys)
        .unwrap()
        .change_delegated_targets("delegated")
        .unwrap()
        .add_target(target_name_3.clone(), target_3)
        .unwrap()
        .targets_version(one)
        .unwrap()
        .targets_expires(later())
        .unwrap()
        .sign_targets_editor(&keys)
        .unwrap();

    let signed_repo = editor.sign(&keys).unwrap();
    let metadata_dir = repo_dir.join("metadata");
    signed_repo.write(&metadata_dir).unwrap();

    let loaded_repo = RepositoryLoader::new(
        File::open(&root_path).unwrap(),
        dir_url(&metadata_dir),
        dir_url(&targets_dir),
    )
    .load()
    .unwrap();

    let outdir = tempdir.path().join("outdir");
    create_dir_all(&outdir).unwrap();
    loaded_repo
        .save_target(&target_name_1, &outdir, Prefix::None)
        .unwrap();
    loaded_repo
        .save_target(&target_name_2, &outdir, Prefix::None)
        .unwrap();
    loaded_repo
        .save_target(&target_name_3, &outdir, Prefix::None)
        .unwrap();

    // These might be created if we didn't safely clean the target names as paths.
    assert!(!outdir.join("bar").exists());
    assert!(!outdir.join("baz").exists());
    assert!(!outdir.join("foo/bar/baz").exists());
    assert!(!outdir.join("../delegated/foo/../subdir/data3.txt").exists());

    // The targets should end up at these paths.
    assert_eq!(
        fs::read_to_string(outdir.join("data1.txt")).unwrap(),
        DATA_1
    );
    assert_eq!(
        fs::read_to_string(outdir.join("foo/bar/data2.txt")).unwrap(),
        DATA_2
    );
    assert_eq!(
        fs::read_to_string(outdir.join("delegated/subdir/data3.txt")).unwrap(),
        DATA_3
    );
}
