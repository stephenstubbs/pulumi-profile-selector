use super::*;
use crate::config::ProfileStore;
use crate::config::storage::LockGuard;
use std::path::PathBuf;

#[test]
fn add_and_read_round_trip_preserves_file_shape() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://pulumi-dev")).unwrap();
    store
        .add(profile(
            "prod",
            "s3://pulumi-prod/state?acl=private&x=$(whoami)",
        ))
        .unwrap();

    assert_eq!(
        store.read().unwrap(),
        vec![
            profile("dev", "s3://pulumi-dev"),
            profile("prod", "s3://pulumi-prod/state?acl=private&x=$(whoami)"),
        ]
    );

    let raw = read_to_string(&store.profiles_path());
    assert!(raw.ends_with('\n'), "serialised profiles end with newline");

    let value: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let entries = value.as_array().expect("profiles file is a JSON array");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0]["name"], "dev");
    assert_eq!(entries[0]["backend"], "s3://pulumi-dev");
    assert_eq!(entries[1]["name"], "prod");
    assert!(entries[0].get("extra").is_none());

    assert_eq!(
        raw_entries(root_of(&store)),
        vec![".profiles.lock".to_owned(), "profiles.json".to_owned()]
    );
}

#[test]
fn read_missing_profiles_file_has_no_side_effects() {
    let (_dir, store) = temp_store();

    assert!(store.read().unwrap().is_empty());
    assert!(!root_of(&store).exists());
}

#[test]
fn read_in_existing_root_creates_nothing() {
    let (_dir, store) = temp_store();
    std::fs::create_dir_all(root_of(&store)).unwrap();

    assert!(store.read().unwrap().is_empty());
    assert!(raw_entries(root_of(&store)).is_empty());
}

#[test]
fn read_accepts_existing_json_array() {
    let (_dir, store) = temp_store();
    write_raw(
        &store.profiles_path(),
        "[{\"name\":\"dev\",\"backend\":\"file://./state\"}]\n",
    );

    assert_eq!(
        store.read().unwrap(),
        vec![profile("dev", "file://./state")]
    );
    assert_eq!(
        raw_entries(root_of(&store)),
        vec!["profiles.json".to_owned()]
    );
}

#[test]
fn read_accepts_unicode_values() {
    let (_dir, store) = temp_store();
    store
        .add(profile("dévelopment", "s3://bøtte/état"))
        .unwrap();

    assert_eq!(
        store.read().unwrap(),
        vec![profile("dévelopment", "s3://bøtte/état")]
    );
}

#[test]
fn edit_updates_backend_in_place() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://old")).unwrap();
    store.add(profile("prod", "s3://prod")).unwrap();

    store.edit("dev", "s3://new".to_owned()).unwrap();

    assert_eq!(
        store.read().unwrap(),
        vec![profile("dev", "s3://new"), profile("prod", "s3://prod")]
    );
    assert_eq!(profile_names(&store), vec!["dev", "prod"]);
}

#[test]
fn edit_missing_profile_fails_without_rewriting() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://dev")).unwrap();
    let before = profiles_bytes(&store);

    let err = store.edit("nope", "s3://x".to_owned()).unwrap_err();

    assert!(error_chain(&err).contains("Profile 'nope' not found"));
    assert_eq!(profiles_bytes(&store), before);
}

#[test]
fn add_duplicate_is_rejected_without_rewriting() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://dev")).unwrap();
    let before = profiles_bytes(&store);

    let err = store.add(profile("dev", "s3://other")).unwrap_err();

    assert!(error_chain(&err).contains("already exists"));
    assert_eq!(profiles_bytes(&store), before);
    assert_eq!(store.read().unwrap(), vec![profile("dev", "s3://dev")]);
}

#[test]
fn delete_removes_only_the_named_profile() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://dev")).unwrap();
    store.add(profile("prod", "s3://prod")).unwrap();

    store.delete("dev").unwrap();
    assert_eq!(store.read().unwrap(), vec![profile("prod", "s3://prod")]);

    let err = store.delete("dev").unwrap_err();
    assert!(error_chain(&err).contains("Profile 'dev' not found"));
}

#[test]
fn delete_on_empty_store_writes_no_profiles_file() {
    let (_dir, store) = temp_store();

    let err = store.delete("ghost").unwrap_err();

    assert!(error_chain(&err).contains("Profile 'ghost' not found"));
    assert!(!store.profiles_path().exists());
    assert_eq!(
        raw_entries(root_of(&store)),
        vec![".profiles.lock".to_owned()]
    );
}

#[test]
fn profile_display_is_name_arrow_backend() {
    let profile = profile("dev", "s3://pulumi-dev");

    assert_eq!(profile.to_string(), "dev -> s3://pulumi-dev");
}

#[test]
fn paths_derive_from_the_configured_root() {
    let root = PathBuf::from("example/.pulumi");
    let store = ProfileStore::new(root.clone());

    assert_eq!(store.profiles_path(), root.join("profiles.json"));
    assert_eq!(store.current_profile_path(), root.join("current_profile"));
    assert_eq!(root_of(&store), root);
    assert!(format!("{store:?}").contains("example/.pulumi"));
}

#[test]
fn read_does_not_require_the_profile_lock() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://dev")).unwrap();
    let guard = LockGuard::acquire(&store.lock_path()).unwrap();

    assert_eq!(store.read().unwrap(), vec![profile("dev", "s3://dev")]);
    drop(guard);
}

#[cfg(unix)]
#[test]
fn created_files_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;

    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://dev")).unwrap();
    store.activate("dev").unwrap();

    for path in [store.profiles_path(), store.current_profile_path()] {
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "{} is {:o}", path.display(), mode);
    }
}
