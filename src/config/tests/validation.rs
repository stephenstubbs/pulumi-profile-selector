use std::ffi::OsString;
use std::path::PathBuf;

use crate::config::{Profile, root_from_env, validate_value};

#[test]
fn valid_values_are_accepted() {
    let valid = [
        "dev",
        "my profile",
        "s3://bucket/state?acl=private&x=$(whoami)",
        "file://./state",
        "https://api.pulumi.com",
        "gs://bucket/state;rm -rf /",
        "a|b&&c`d`",
        "quotes'\"and{braces}",
        "*stars*",
        "dévelopment",
        "s3://bøtte/état",
        "🚀-rocket",
        "a",
    ];

    for value in valid {
        validate_value(value, "field").unwrap_or_else(|err| panic!("{value:?} rejected: {err:#}"));
    }
}

#[test]
fn empty_and_whitespace_only_values_are_rejected() {
    for value in ["", " ", "   ", "\t", "\n", " \t\n ", "\u{00a0}"] {
        let err = validate_value(value, "field").unwrap_err();
        assert!(
            format!("{err:#}").contains("field must not be empty"),
            "{value:?}: {err:#}"
        );
    }
}

#[test]
fn surrounding_whitespace_is_rejected() {
    for value in [" dev", "dev ", "\tdev", "dev\t", "\ndev", "dev\n", " dev "] {
        let err = validate_value(value, "field").unwrap_err();
        assert!(
            format!("{err:#}").contains("must not start or end with whitespace"),
            "{value:?}: {err:#}"
        );
    }
}

#[test]
fn control_characters_are_rejected() {
    for value in [
        "de\u{0}v",
        "de\u{7}v",
        "de\u{1b}v",
        "de\u{7f}v",
        "de\u{85}v",
        "de\nv",
        "de\tv",
    ] {
        let err = validate_value(value, "field").unwrap_err();
        assert!(
            format!("{err:#}").contains("must not contain control characters"),
            "{value:?}: {err:#}"
        );
    }
}

#[test]
fn interior_spaces_are_allowed() {
    validate_value("my dev profile", "field").unwrap();
}

#[test]
fn error_messages_use_the_supplied_label() {
    let err = validate_value("", "Backend URL").unwrap_err();
    assert_eq!(format!("{err:#}"), "Backend URL must not be empty");
}

#[test]
fn profile_new_validates_name_and_backend() {
    let err = Profile::new(String::new(), "s3://x".to_owned()).unwrap_err();
    assert!(format!("{err:#}").contains("Profile name must not be empty"));

    let err = Profile::new("dev".to_owned(), "s3://x\u{0}y".to_owned()).unwrap_err();
    assert!(format!("{err:#}").contains("Backend URL must not contain control characters"));

    let err = Profile::new(" dev".to_owned(), "s3://x".to_owned()).unwrap_err();
    assert!(format!("{err:#}").contains("Profile name must not start or end with whitespace"));
}

#[test]
fn profile_new_keeps_values_verbatim() {
    let profile = Profile::new("my dev".to_owned(), "s3://x?y=$(z)".to_owned()).unwrap();

    assert_eq!(profile.name, "my dev");
    assert_eq!(profile.backend, "s3://x?y=$(z)");
}

#[test]
fn root_prefers_pulumi_home() {
    let root = root_from_env(
        Some(OsString::from("/custom/pulumi")),
        Some(PathBuf::from("/home/user")),
    )
    .unwrap();

    assert_eq!(root, PathBuf::from("/custom/pulumi"));
}

#[test]
fn root_falls_back_to_home_directory() {
    let root = root_from_env(None, Some(PathBuf::from("/home/user"))).unwrap();

    assert_eq!(root, PathBuf::from("/home/user/.pulumi"));
}

#[test]
fn blank_pulumi_home_falls_back_to_home_directory() {
    for blank in ["", " ", "  \t\n"] {
        let root = root_from_env(
            Some(OsString::from(blank)),
            Some(PathBuf::from("/home/user")),
        )
        .unwrap();

        assert_eq!(root, PathBuf::from("/home/user/.pulumi"), "{blank:?}");
    }
}

#[test]
fn blank_pulumi_home_without_home_is_an_error() {
    let err = root_from_env(Some(OsString::from("  ")), None).unwrap_err();

    assert!(format!("{err:#}").contains("Unable to determine home directory"));
}

#[test]
fn missing_home_directory_is_an_error() {
    let err = root_from_env(None, None).unwrap_err();

    assert!(format!("{err:#}").contains("Unable to determine home directory"));
}

#[cfg(unix)]
#[test]
fn non_utf8_pulumi_home_is_used_as_is() {
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let raw = OsString::from_vec(b"/custom/\xff".to_vec());
    let root = root_from_env(Some(raw), Some(PathBuf::from("/home/user"))).unwrap();

    assert_eq!(root.as_os_str().as_bytes(), b"/custom/\xff");
}
