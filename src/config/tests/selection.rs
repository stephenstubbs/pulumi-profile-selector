use super::*;
use crate::config::Selection;

#[test]
fn identical_backends_keep_distinct_profile_identity() {
    let (_dir, store) = temp_store();
    store
        .add(profile("company-a", "https://api.pulumi.com"))
        .unwrap();
    store
        .add(profile("company-b", "https://api.pulumi.com"))
        .unwrap();

    store.activate("company-a").unwrap();
    assert_eq!(
        store.current_selection().unwrap(),
        Selection {
            profile: Some("company-a".to_owned()),
            backend: Some("https://api.pulumi.com".to_owned()),
        }
    );

    store.activate("company-b").unwrap();
    assert_eq!(
        store.current_selection().unwrap(),
        Selection {
            profile: Some("company-b".to_owned()),
            backend: Some("https://api.pulumi.com".to_owned()),
        }
    );
}

#[test]
fn raw_backend_matching_a_saved_url_stays_unnamed() {
    let (_dir, store) = temp_store();
    store
        .add(profile("company", "https://api.pulumi.com"))
        .unwrap();

    store.activate("https://api.pulumi.com").unwrap();

    assert_eq!(
        store.current_selection().unwrap(),
        Selection {
            profile: None,
            backend: Some("https://api.pulumi.com".to_owned()),
        }
    );
}

#[test]
fn explicit_resolution_distinguishes_names_from_backends() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://state-dev")).unwrap();

    assert_eq!(
        store.resolve_selection("dev").unwrap(),
        Selection {
            profile: Some("dev".to_owned()),
            backend: Some("s3://state-dev".to_owned()),
        }
    );
    assert_eq!(
        store.resolve_selection("s3://state-dev").unwrap(),
        Selection {
            profile: None,
            backend: Some("s3://state-dev".to_owned()),
        }
    );
    assert_eq!(
        store.resolve_selection("unsaved").unwrap(),
        Selection {
            profile: None,
            backend: Some("unsaved".to_owned()),
        }
    );
}

#[test]
fn inactive_selection_has_no_identity_or_backend() {
    let (_dir, store) = temp_store();

    assert_eq!(store.current_selection().unwrap(), Selection::default());
    assert!(!root_of(&store).exists());
}

#[test]
fn deleted_profile_name_is_reported_as_a_raw_backend() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://state-dev")).unwrap();
    write_raw(&store.current_profile_path(), "gone");

    assert_eq!(
        store.current_selection().unwrap(),
        Selection {
            profile: None,
            backend: Some("gone".to_owned()),
        }
    );
}

#[test]
fn selection_reads_are_read_only_and_reject_invalid_current_values() {
    let (_dir, store) = temp_store();
    store.add(profile("dev", "s3://state-dev")).unwrap();
    write_raw(&store.current_profile_path(), "bad\u{1b}value");

    let err = store.current_selection().unwrap_err();
    assert!(error_chain(&err).contains("Invalid current profile"));
    assert_eq!(
        read_to_string(&store.current_profile_path()),
        "bad\u{1b}value"
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
fn selection_errors_on_malformed_profiles_file() {
    let (_dir, store) = temp_store();
    write_raw(&store.profiles_path(), "{broken");
    store.activate("dev").unwrap();

    let err = store.current_selection().unwrap_err();
    assert!(error_chain(&err).contains("parse"));
}
