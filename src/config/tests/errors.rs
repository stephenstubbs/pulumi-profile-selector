use super::*;
use crate::config::storage::LockGuard;

#[test]
fn malformed_json_reports_path_and_cause() {
    let (_dir, store) = temp_store();
    write_raw(&store.profiles_path(), "[{\"name\": \"dev\",\"backend\": ");

    let err = store.read().unwrap_err();
    let chain = error_chain(&err);

    assert!(chain.contains("Failed to parse profiles file"), "{chain}");
    assert!(
        chain.contains(&path_text(&store.profiles_path())),
        "{chain}"
    );
    assert!(chain.contains("line"), "{chain}");
}

#[test]
fn empty_profiles_file_is_malformed() {
    let (_dir, store) = temp_store();
    write_raw(&store.profiles_path(), "");

    let err = store.read().unwrap_err();
    assert!(error_chain(&err).contains("Failed to parse profiles file"));
}

#[test]
fn json_object_instead_of_array_is_malformed() {
    let (_dir, store) = temp_store();
    write_raw(
        &store.profiles_path(),
        "{\"name\":\"dev\",\"backend\":\"s3://x\"}",
    );

    let err = store.read().unwrap_err();
    assert!(error_chain(&err).contains("Failed to parse profiles file"));
}

#[test]
fn profile_missing_a_field_is_malformed() {
    let (_dir, store) = temp_store();
    write_raw(&store.profiles_path(), "[{\"name\":\"dev\"}]");

    let err = store.read().unwrap_err();
    let chain = error_chain(&err);
    assert!(chain.contains("Failed to parse profiles file"), "{chain}");
    assert!(chain.contains("backend"), "{chain}");
}

#[test]
fn duplicate_profile_names_are_rejected() {
    let (_dir, store) = temp_store();
    write_raw(
        &store.profiles_path(),
        r#"[{"name":"dev","backend":"s3://a"},{"name":"dev","backend":"s3://b"}]"#,
    );

    let err = store.read().unwrap_err();
    let chain = error_chain(&err);

    assert!(chain.contains("Duplicate profile name 'dev'"), "{chain}");
    assert!(
        chain.contains(&path_text(&store.profiles_path())),
        "{chain}"
    );
}

#[test]
fn invalid_loaded_profiles_are_rejected() {
    let (_dir, store) = temp_store();
    let cases = [
        (
            r#"[{"name":"","backend":"s3://x"}]"#,
            "Profile name must not be empty",
        ),
        (
            r#"[{"name":"dev","backend":""}]"#,
            "Backend URL must not be empty",
        ),
        (
            r#"[{"name":" dev","backend":"s3://x"}]"#,
            "Profile name must not start or end with whitespace",
        ),
        (
            r#"[{"name":"dev","backend":"s3://x\u001by"}]"#,
            "Backend URL must not contain control characters",
        ),
    ];

    for (json, expected) in cases {
        write_raw(&store.profiles_path(), json);
        let err = store.read().unwrap_err();
        let chain = error_chain(&err);
        assert!(chain.contains("Invalid profile"), "{json}: {chain}");
        assert!(chain.contains(expected), "{json}: {chain}");
    }
}

#[test]
fn profiles_file_that_is_not_utf8_is_a_read_error() {
    let (_dir, store) = temp_store();
    std::fs::create_dir_all(root_of(&store)).unwrap();
    std::fs::write(store.profiles_path(), [0xff, 0xfe, 0x00]).unwrap();

    let err = store.read().unwrap_err();
    let chain = error_chain(&err);

    assert!(chain.contains("Failed to read profiles file"), "{chain}");
    assert!(
        chain.contains(&path_text(&store.profiles_path())),
        "{chain}"
    );
}

#[test]
fn profiles_path_that_is_a_directory_is_a_read_error() {
    let (_dir, store) = temp_store();
    std::fs::create_dir_all(store.profiles_path()).unwrap();

    let err = store.read().unwrap_err();
    assert!(error_chain(&err).contains("Failed to read profiles file"));
}

#[test]
fn current_profile_that_is_not_utf8_is_a_read_error() {
    let (_dir, store) = temp_store();
    std::fs::create_dir_all(root_of(&store)).unwrap();
    std::fs::write(store.current_profile_path(), [0xc3, 0x28]).unwrap();

    let err = store.current_backend().unwrap_err();
    assert!(error_chain(&err).contains("Failed to read"));
}

#[test]
fn current_backend_propagates_corrupt_profiles() {
    let (_dir, store) = temp_store();
    write_raw(&store.profiles_path(), "not json");
    write_raw(&store.current_profile_path(), "dev");

    let err = store.current_backend().unwrap_err();
    assert!(error_chain(&err).contains("Failed to parse profiles file"));
}

#[test]
fn current_profile_control_characters_are_rejected_without_rewriting() {
    let (_dir, store) = temp_store();
    for value in ["s3://state\ninjected", "dev\u{1b}[2J", "dev\0suffix"] {
        write_raw(&store.current_profile_path(), value);
        let error = store.current_backend().unwrap_err();
        let chain = error_chain(&error);
        assert!(chain.contains("Invalid current profile"), "{chain}");
        assert!(chain.contains("control characters"), "{chain}");
        assert_eq!(read_to_string(&store.current_profile_path()), value);
    }
}

#[test]
fn activate_onto_a_directory_reports_persist_failure() {
    let (_dir, store) = temp_store();
    std::fs::create_dir_all(store.current_profile_path()).unwrap();
    std::fs::write(store.current_profile_path().join("keep"), "kept").unwrap();

    let err = store.activate("dev").unwrap_err();
    let chain = error_chain(&err);

    assert!(chain.contains("Failed to replace"), "{chain}");
    assert!(store.current_profile_path().is_dir());
    assert_eq!(
        raw_entries(root_of(&store)),
        vec!["current_profile".to_owned()]
    );
    assert!(store.current_profile_path().join("keep").exists());
}

#[test]
fn deactivate_when_current_profile_is_a_directory_reports_error() {
    let (_dir, store) = temp_store();
    std::fs::create_dir_all(store.current_profile_path()).unwrap();

    let err = store.deactivate().unwrap_err();
    assert!(error_chain(&err).contains("Failed to remove"));
    assert!(store.current_profile_path().is_dir());
}

#[test]
fn lock_contention_fails_fast_without_waiting() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://dev")).unwrap();
    let guard = LockGuard::acquire(&store.lock_path()).unwrap();

    let err = store.add(profile("prod", "s3://prod")).unwrap_err();
    assert!(error_chain(&err).contains("Another process"), "{err:#}");

    drop(guard);
    store.add(profile("prod", "s3://prod")).unwrap();
    assert_eq!(profile_names(&store), vec!["dev", "prod"]);
}

#[cfg(unix)]
#[test]
fn unwritable_root_leaves_profiles_file_intact() {
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    struct RestoreMode {
        path: PathBuf,
        mode: u32,
    }

    impl Drop for RestoreMode {
        fn drop(&mut self) {
            let _ =
                std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(self.mode));
        }
    }

    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://dev")).unwrap();
    let before = profiles_bytes(&store);

    let mode = std::fs::metadata(root_of(&store))
        .unwrap()
        .permissions()
        .mode();
    std::fs::set_permissions(root_of(&store), std::fs::Permissions::from_mode(0o555)).unwrap();
    let _restore = RestoreMode {
        path: root_of(&store).to_path_buf(),
        mode,
    };

    if std::fs::write(root_of(&store).join("probe"), b"x").is_ok() {
        return; // privileged process: mode bits do not deny writes
    }

    let err = store.add(profile("prod", "s3://prod")).unwrap_err();
    assert!(error_chain(&err).contains("Failed to"), "{err:#}");
    assert_eq!(profiles_bytes(&store), before);
    assert_eq!(store.read().unwrap(), vec![profile("dev", "s3://dev")]);
}
