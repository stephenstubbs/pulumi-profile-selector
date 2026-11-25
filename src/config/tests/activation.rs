use super::*;

#[test]
fn activate_stores_name_and_resolves_backend() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://pulumi-dev")).unwrap();

    store.activate("dev").unwrap();

    assert_eq!(read_to_string(&store.current_profile_path()), "dev");
    assert_eq!(
        store.current_backend().unwrap(),
        Some("s3://pulumi-dev".to_owned())
    );
    assert_eq!(
        raw_entries(root_of(&store)),
        vec![
            ".profiles.lock".to_owned(),
            "current_profile".to_owned(),
            "profiles.json".to_owned(),
        ]
    );
}

#[test]
fn activate_raw_backend_falls_back_to_the_value() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://pulumi-dev")).unwrap();

    store.activate("https://api.pulumi.com").unwrap();

    assert_eq!(
        store.current_backend().unwrap(),
        Some("https://api.pulumi.com".to_owned())
    );
}

#[test]
fn activate_raw_backend_without_profiles_file_creates_no_profiles_file() {
    let (_dir, store) = temp_store();

    store.activate("file://./state").unwrap();

    assert_eq!(
        store.current_backend().unwrap(),
        Some("file://./state".to_owned())
    );
    assert!(!store.profiles_path().exists());
}

#[test]
fn activate_replaces_previous_value_without_temp_leftovers() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://dev")).unwrap();
    store.add(profile("prod", "s3://prod")).unwrap();

    store.activate("dev").unwrap();
    store.activate("prod").unwrap();

    assert_eq!(read_to_string(&store.current_profile_path()), "prod");
    assert_eq!(
        store.current_backend().unwrap(),
        Some("s3://prod".to_owned())
    );
    assert_eq!(
        raw_entries(root_of(&store)),
        vec![
            ".profiles.lock".to_owned(),
            "current_profile".to_owned(),
            "profiles.json".to_owned(),
        ]
    );
}

#[test]
fn activate_rejects_invalid_values_without_creating_root() {
    let (_dir, store) = temp_store();

    for invalid in ["", "   ", " dev", "dev ", "de\nv", "de\u{0}v"] {
        let err = store.activate(invalid).unwrap_err();
        assert!(
            error_chain(&err).contains("Profile name or backend URL"),
            "unexpected error for {invalid:?}: {}",
            error_chain(&err)
        );
    }

    assert!(!root_of(&store).exists());
}

#[test]
fn deactivate_is_idempotent() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://dev")).unwrap();
    store.activate("dev").unwrap();

    assert!(store.deactivate().unwrap());
    assert!(!store.current_profile_path().exists());
    assert!(!store.deactivate().unwrap());
    assert!(!store.deactivate().unwrap());
    assert_eq!(store.current_backend().unwrap(), None);
}

#[test]
fn deactivate_on_missing_root_creates_nothing() {
    let (_dir, store) = temp_store();

    assert!(!store.deactivate().unwrap());
    assert!(!root_of(&store).exists());
}

#[test]
fn delete_active_profile_clears_current_profile() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://dev")).unwrap();
    store.activate("dev").unwrap();

    store.delete("dev").unwrap();

    assert!(!store.current_profile_path().exists());
    assert_eq!(store.current_backend().unwrap(), None);
    assert!(store.read().unwrap().is_empty());
}

#[test]
fn delete_active_profile_with_trailing_newline_clears_current_profile() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://dev")).unwrap();
    write_raw(&store.current_profile_path(), "dev\n");

    store.delete("dev").unwrap();

    assert!(!store.current_profile_path().exists());
}

#[test]
fn delete_inactive_profile_keeps_current_profile() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://dev")).unwrap();
    store.add(profile("prod", "s3://prod")).unwrap();
    store.activate("prod").unwrap();

    store.delete("dev").unwrap();

    assert_eq!(read_to_string(&store.current_profile_path()), "prod");
    assert_eq!(
        store.current_backend().unwrap(),
        Some("s3://prod".to_owned())
    );
}

#[test]
fn empty_current_profile_is_none() {
    let (_dir, store) = temp_store();

    write_raw(&store.current_profile_path(), "");
    assert_eq!(store.current_backend().unwrap(), None);

    write_raw(&store.current_profile_path(), " \n\t\n");
    assert_eq!(store.current_backend().unwrap(), None);
}

#[test]
fn current_profile_with_trailing_newline_resolves() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://dev")).unwrap();
    write_raw(&store.current_profile_path(), "dev\r\n");

    assert_eq!(
        store.current_backend().unwrap(),
        Some("s3://dev".to_owned())
    );
}

#[test]
fn current_backend_without_active_profile_has_no_side_effects() {
    let (_dir, store) = temp_store();

    assert_eq!(store.current_backend().unwrap(), None);
    assert!(!root_of(&store).exists());
}
