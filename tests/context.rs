mod support;

use std::{fs, process::Command};

use serde_json::{Value, json};
use support::Fixture;

fn context(stdout: &str) -> Value {
    serde_json::from_str(stdout).unwrap()
}

fn companies() -> Value {
    json!([
        {"name": "company-a", "backend": "https://api.pulumi.com"},
        {"name": "company-b", "backend": "https://api.pulumi.com"},
    ])
}

fn profiles() -> Value {
    json!([
        {"name": "dev", "backend": "s3://state-dev"},
        {"name": "prod", "backend": "s3://state-prod"},
    ])
}

#[test]
fn context_requires_a_backend_output_action() {
    let fixture = Fixture::new();
    for args in [
        vec!["--context"],
        vec!["--context", "--list"],
        vec!["--context", "--delete", "dev"],
        vec!["--show-current", "--shell", "posix"],
    ] {
        assert_eq!(fixture.run(&args).status.code(), Some(2), "{args:?}");
    }
    assert!(!fixture.root.exists());
}

#[test]
fn context_json_reports_identity_even_when_backends_are_identical() {
    let fixture = Fixture::new();
    fixture.profiles(companies());
    for name in ["company-a", "company-b"] {
        fixture.success(&["-a", name]);
        assert_eq!(
            context(&fixture.success(&["--show-current", "--context", "--json"])),
            json!({"profile": name, "backend": "https://api.pulumi.com"})
        );
        assert_eq!(fixture.saved(), name);
    }
}

#[test]
fn context_json_never_infers_identity_from_a_matching_backend() {
    let fixture = Fixture::new();
    fixture.profiles(companies());

    fixture.success(&["-n", "https://api.pulumi.com"]);
    assert_eq!(
        context(&fixture.success(&["--show-current", "--context", "--json"])),
        json!({"profile": null, "backend": "https://api.pulumi.com"})
    );

    assert_eq!(
        context(&fixture.success(&["-c", "-n", "https://api.pulumi.com", "--context", "--json"])),
        json!({"profile": null, "backend": "https://api.pulumi.com"})
    );
    assert_eq!(
        context(&fixture.success(&["-c", "-n", "company-a", "--context", "--json"])),
        json!({"profile": "company-a", "backend": "https://api.pulumi.com"})
    );
    assert_eq!(
        context(&fixture.success(&["-c", "-n", "unsaved", "--context", "--json"])),
        json!({"profile": null, "backend": "unsaved"})
    );
}

#[test]
fn context_json_covers_explicit_and_deactivation_modes() {
    let fixture = Fixture::new();
    fixture.profiles(companies());
    fixture.success(&["-a", "company-b"]);

    assert_eq!(
        context(&fixture.success(&["-c", "-a", "company-a", "--context", "--json"])),
        json!({"profile": "company-a", "backend": "https://api.pulumi.com"})
    );
    assert_eq!(
        context(&fixture.success(&["-c", "-d", "--context", "--json"])),
        json!({"profile": null, "backend": null})
    );
    assert_eq!(
        context(&fixture.success(&["--show-current", "--json"])),
        json!({"backend": "https://api.pulumi.com"})
    );
    assert_eq!(fixture.saved(), "company-b");
}

#[test]
fn legacy_outputs_never_gain_profile_metadata() {
    let fixture = Fixture::new();
    fixture.profiles(profiles());
    fixture.success(&["-a", "dev"]);

    assert_eq!(
        fixture.success(&["-c", "-a", "dev", "--json"]).trim(),
        r#"{"backend":"s3://state-dev"}"#
    );
    assert_eq!(
        fixture.success(&["-c", "-d", "--json"]).trim(),
        r#"{"backend":null}"#
    );
    assert_eq!(
        fixture.success(&["--show-current", "--json"]).trim(),
        r#"{"backend":"s3://state-dev"}"#
    );
    assert_eq!(
        fixture.success(&["-c", "-a", "dev"]),
        "export PULUMI_BACKEND_URL='s3://state-dev'\n"
    );
    assert_eq!(fixture.success(&["-c", "-d"]), "unset PULUMI_BACKEND_URL\n");
    assert_eq!(fixture.success(&["--show-current"]), "s3://state-dev\n");
}

#[test]
fn context_shell_output_sets_and_clears_both_variables() {
    let fixture = Fixture::new();
    fixture.profiles(profiles());

    assert_eq!(
        fixture.success(&["-c", "--context", "-a", "dev", "--shell", "posix"]),
        "export PULUMI_BACKEND_URL='s3://state-dev'\n\
         export PULUMI_PROFILE_SELECTOR_PROFILE='dev'\n"
    );
    assert_eq!(
        fixture.success(&["-c", "--context", "-n", "s3://raw", "--shell", "posix"]),
        "export PULUMI_BACKEND_URL='s3://raw'\nunset PULUMI_PROFILE_SELECTOR_PROFILE\n"
    );
    assert_eq!(
        fixture.success(&["-c", "--context", "-d", "--shell", "fish"]),
        "set -e PULUMI_BACKEND_URL\nset -e PULUMI_PROFILE_SELECTOR_PROFILE\n"
    );
    assert_eq!(
        fixture.success(&["--show-current", "--context", "--shell", "nu"]),
        "hide-env --ignore-errors PULUMI_BACKEND_URL\n\
         hide-env --ignore-errors PULUMI_PROFILE_SELECTOR_PROFILE\n"
    );
    assert!(!fixture.root.join("current_profile").exists());
}

#[test]
fn context_shell_output_round_trips_adversarial_values() {
    let fixture = Fixture::new();
    let name = r#"team-'"$(touch injected)`id` quoted""#;
    let backend = r#"file:///tmp/a'"\$(touch injected)`id`&;() ${HOME} spaced"#;
    fixture.profiles(json!([{"name": name, "backend": backend}]));

    let source = fixture.success(&["-c", "--context", "-a", name, "--shell", "posix"]);
    assert_eq!(source.lines().count(), 2);
    let output = Command::new("sh")
        .current_dir(fixture.home.path())
        .args([
            "-c",
            &format!(
                "{source}\nprintf '%s\\n%s' \"$PULUMI_BACKEND_URL\" \"$PULUMI_PROFILE_SELECTOR_PROFILE\""
            ),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{backend}\n{name}")
    );
    assert!(!fixture.home.path().join("injected").exists());
}

#[test]
fn context_nu_output_round_trips_adversarial_values() {
    let fixture = Fixture::new();
    let name = r#"team-'"$(touch injected)`id`\ quoted""#;
    let backend = r#"file:///tmp/a'"\$(touch injected)`id`&;() ${HOME}"#;
    fixture.profiles(json!([{"name": name, "backend": backend}]));
    let Ok(version) = Command::new("nu").arg("--version").output() else {
        return;
    };
    assert!(version.status.success());

    let source = fixture.success(&["-c", "--context", "-a", name, "--shell", "nu"]);
    let output = Command::new("nu")
        .current_dir(fixture.home.path())
        .args([
            "--no-config-file",
            "-c",
            &format!("{source}\nprint $env.PULUMI_BACKEND_URL\nprint $env.PULUMI_PROFILE_SELECTOR_PROFILE"),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("{backend}\n{name}\n")
    );
    assert!(!fixture.home.path().join("injected").exists());
}

#[test]
fn show_current_context_is_read_only_and_never_creates_state() {
    let fixture = Fixture::new();
    assert_eq!(
        context(&fixture.success(&["--show-current", "--context", "--json"])),
        json!({"profile": null, "backend": null})
    );
    assert_eq!(
        fixture.success(&["--show-current", "--context", "--shell", "posix"]),
        "unset PULUMI_BACKEND_URL\nunset PULUMI_PROFILE_SELECTOR_PROFILE\n"
    );
    assert!(!fixture.root.exists());

    fixture.profiles(companies());
    fixture.success(&["-a", "company-a"]);
    assert_eq!(
        fixture.success(&["--show-current", "--context", "--shell", "posix"]),
        "export PULUMI_BACKEND_URL='https://api.pulumi.com'\n\
         export PULUMI_PROFILE_SELECTOR_PROFILE='company-a'\n"
    );
    assert_eq!(fixture.saved(), "company-a");
}

#[test]
fn context_errors_never_emit_a_stale_selection() {
    let fixture = Fixture::new();
    fixture.profiles(profiles());
    fixture.success(&["-a", "dev"]);

    assert!(
        fixture
            .failure(&["-c", "-a", "missing", "--context", "--json"])
            .contains("not found")
    );

    fs::write(fixture.root.join("current_profile"), "bad\u{1b}name").unwrap();
    fixture.failure(&["--show-current", "--context", "--json"]);
    assert_eq!(
        fs::read_to_string(fixture.root.join("current_profile")).unwrap(),
        "bad\u{1b}name"
    );

    fs::write(fixture.root.join("profiles.json"), "{broken").unwrap();
    fixture.failure(&["--show-current", "--context", "--json"]);
    assert_eq!(
        fs::read_to_string(fixture.root.join("profiles.json")).unwrap(),
        "{broken"
    );
}
