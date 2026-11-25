use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

use serde_json::{Value, json};
use tempfile::TempDir;

struct Fixture {
    home: TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("pulumi");
        Self { home, root }
    }

    fn profiles(&self, value: Value) {
        fs::create_dir_all(&self.root).unwrap();
        fs::write(self.root.join("profiles.json"), value.to_string()).unwrap();
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pulumi-profile-selector"));
        command
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env("PULUMI_HOME", &self.root)
            .env("SHELL", "/gnu/bin/bash");
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }

    fn success(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    fn failure(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        String::from_utf8(output.stderr).unwrap()
    }

    fn saved(&self) -> String {
        fs::read_to_string(self.root.join("current_profile")).unwrap()
    }
}

fn profiles() -> Value {
    json!([
        {"name": "dev", "backend": "s3://state-dev"},
        {"name": "prod", "backend": "s3://state-prod"},
    ])
}

#[test]
fn read_only_and_help_commands_do_not_create_files() {
    let fixture = Fixture::new();
    assert!(fixture.success(&["--help"]).contains("--show-current"));
    assert!(
        fixture
            .success(&["--version"])
            .contains(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(fixture.success(&["--list"]), "No profiles found.\n");
    assert_eq!(fixture.success(&["--show-current"]), "");
    assert!(!fixture.root.exists());
}

#[test]
fn activation_and_deactivation_keep_the_plain_text_format() {
    let fixture = Fixture::new();
    fixture.profiles(profiles());
    fixture.success(&["-a", "dev"]);
    assert_eq!(fixture.saved(), "dev");
    assert_eq!(fixture.success(&["--show-current"]), "s3://state-dev\n");
    assert!(fixture.success(&["-l"]).contains("prod -> s3://state-prod"));
    fixture.success(&["-d"]);
    assert!(!fixture.root.join("current_profile").exists());
    assert_eq!(fixture.success(&["--show-current"]), "");
    assert!(fixture.success(&["-d"]).contains("No active"));
}

#[test]
fn current_mode_outputs_structured_data_without_changing_state() {
    let fixture = Fixture::new();
    fixture.profiles(profiles());
    fixture.success(&["-a", "prod"]);
    let output = fixture.success(&["-c", "--json", "-a", "dev"]);
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap(),
        json!({"backend": "s3://state-dev"})
    );
    assert_eq!(fixture.saved(), "prod");
    let output = fixture.success(&["-c", "--json", "-d"]);
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap(),
        json!({"backend": null})
    );
    assert_eq!(fixture.saved(), "prod");
}

#[test]
fn raw_backends_work_persistently_and_in_current_mode() {
    let fixture = Fixture::new();
    let backend = "file:///tmp/state";
    assert_eq!(
        fixture.success(&["-c", "-n", backend]),
        format!("export PULUMI_BACKEND_URL='{backend}'\n")
    );
    assert!(!fixture.root.exists());
    fixture.success(&["-n", backend]);
    assert_eq!(fixture.saved(), backend);
    assert_eq!(fixture.success(&["--show-current"]), format!("{backend}\n"));

    fixture.profiles(profiles());
    assert_eq!(
        fixture.success(&["-c", "-n", "dev"]),
        "export PULUMI_BACKEND_URL='s3://state-dev'\n"
    );
}

#[test]
fn current_deactivation_is_read_only_and_idempotent() {
    let fixture = Fixture::new();
    for (shell, expected) in [
        ("nu", "hide-env --ignore-errors PULUMI_BACKEND_URL\n"),
        ("fish", "set -e PULUMI_BACKEND_URL\n"),
        ("posix", "unset PULUMI_BACKEND_URL\n"),
    ] {
        assert_eq!(fixture.success(&["-c", "-d", "--shell", shell]), expected);
    }
    fixture.success(&["-d"]);
    assert!(!fixture.root.exists());
}

#[test]
fn conflicting_actions_fail_before_touching_configuration() {
    let fixture = Fixture::new();
    for args in [
        vec!["-a", "dev", "-d"],
        vec!["--list", "--delete", "dev"],
        vec!["--current", "--add"],
        vec!["--json"],
    ] {
        assert!(fixture.failure(&args).contains("error:"));
    }
    assert!(!fixture.root.exists());
}

#[test]
fn malformed_json_and_unknown_profiles_are_not_overwritten() {
    let fixture = Fixture::new();
    fixture.profiles(profiles());
    fixture.success(&["-a", "prod"]);
    assert!(
        fixture
            .failure(&["-a", "missing"])
            .contains("Available profiles: dev, prod")
    );
    assert!(
        fixture
            .failure(&["--edit", "missing"])
            .contains("Profile 'missing' not found")
    );
    assert_eq!(fixture.saved(), "prod");

    let file = fixture.root.join("profiles.json");
    fs::write(&file, "{broken").unwrap();
    assert!(fixture.failure(&["-l"]).contains("parse"));
    assert!(fixture.failure(&["-c", "-n", "custom"]).contains("parse"));
    assert_eq!(fs::read_to_string(file).unwrap(), "{broken");
    assert_eq!(fixture.saved(), "prod");
    fixture.success(&["-d"]);
}

#[test]
fn deleting_the_active_profile_clears_persisted_state() {
    let fixture = Fixture::new();
    fixture.profiles(profiles());
    fixture.success(&["-a", "dev"]);
    fixture.success(&["--delete", "dev"]);
    assert_eq!(fixture.success(&["--show-current"]), "");
    assert_eq!(
        serde_json::from_str::<Value>(
            &fs::read_to_string(fixture.root.join("profiles.json")).unwrap()
        )
        .unwrap(),
        json!([{"name": "prod", "backend": "s3://state-prod"}])
    );
    assert!(
        fixture
            .failure(&["--delete", "missing"])
            .contains("not found")
    );
}

#[test]
fn empty_and_control_character_values_fail_without_writes() {
    let fixture = Fixture::new();
    for value in ["", "  ", "\nmalicious", "state\u{1b}", " padded "] {
        fixture.failure(&["--new", value]);
    }
    assert!(!fixture.root.exists());
}

#[test]
fn posix_shell_output_round_trips_metacharacters_without_execution() {
    let fixture = Fixture::new();
    let backend = r#"file:///tmp/a'"\$(printf injected)`printf injected`&;() ${HOME}"#;
    let source = fixture.success(&["-c", "-n", backend, "--shell", "posix"]);
    let output = Command::new("sh")
        .args([
            "-c",
            &format!("{source}\nprintf '%s' \"$PULUMI_BACKEND_URL\""),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), backend);
}

#[test]
fn explicit_shell_selection_overrides_login_shell_detection() {
    let fixture = Fixture::new();
    assert_eq!(
        fixture.success(&["-c", "-n", "s3://dev", "--shell", "fish"]),
        "set -gx PULUMI_BACKEND_URL 's3://dev'\n"
    );
    assert_eq!(
        fixture.success(&["-c", "-n", "s3://dev", "--shell", "nu"]),
        "$env.PULUMI_BACKEND_URL = \"s3://dev\"\n"
    );
}

#[test]
fn unset_or_empty_pulumi_home_uses_the_users_home_directory() {
    for value in [None, Some("")] {
        let fixture = Fixture::new();
        let mut command = fixture.command();
        command.env_remove("PULUMI_HOME");
        if let Some(value) = value {
            command.env("PULUMI_HOME", value);
        }
        let output = command.args(["--new", "s3://home-state"]).output().unwrap();
        assert!(output.status.success());
        assert_eq!(
            fs::read_to_string(fixture.home.path().join(".pulumi/current_profile")).unwrap(),
            "s3://home-state"
        );
        assert!(!fixture.root.exists());
    }
}

#[test]
fn a_configuration_path_that_is_a_file_is_reported_without_replacing_it() {
    let fixture = Fixture::new();
    fs::write(&fixture.root, "not a directory").unwrap();
    fixture.failure(&["--new", "s3://dev"]);
    assert_eq!(
        fs::read_to_string(&fixture.root).unwrap(),
        "not a directory"
    );
}

#[test]
fn show_current_json_is_read_only_and_handles_inactive_state() {
    let fixture = Fixture::new();
    let output = fixture.success(&["--show-current", "--json"]);
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap(),
        json!({"backend": null})
    );
    assert!(!fixture.root.exists());
    fixture.profiles(profiles());
    fixture.success(&["-a", "dev"]);
    let output = fixture.success(&["--show-current", "--json"]);
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap(),
        json!({"backend": "s3://state-dev"})
    );
    assert_eq!(fixture.saved(), "dev");
}

#[test]
fn json_takes_precedence_over_shell_syntax() {
    let fixture = Fixture::new();
    let output = fixture.success(&["-c", "-n", "s3://dev", "--shell", "nu", "--json"]);
    assert_eq!(
        serde_json::from_str::<Value>(&output).unwrap(),
        json!({"backend": "s3://dev"})
    );
}

#[test]
fn combined_short_flags_respect_options_that_consume_values() {
    let fixture = Fixture::new();
    fixture.profiles(profiles());
    fixture.success(&["-a", "prod"]);
    assert_eq!(
        fixture.success(&["-ca", "dev"]),
        "export PULUMI_BACKEND_URL='s3://state-dev'\n"
    );
    assert_eq!(
        fixture.success(&["-cn", "s3://custom"]),
        "export PULUMI_BACKEND_URL='s3://custom'\n"
    );
    assert_eq!(fixture.success(&["-cd"]), "unset PULUMI_BACKEND_URL\n");
    let output = fixture.run(&["-ac"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("Profile 'c' not found"));
    assert_eq!(fixture.saved(), "prod");
}

#[test]
fn usage_errors_and_application_errors_have_distinct_exit_codes() {
    let fixture = Fixture::new();
    assert_eq!(fixture.run(&["--list", "--add"]).status.code(), Some(2));
    assert_eq!(
        fixture.run(&["--activate", "missing"]).status.code(),
        Some(1)
    );
    assert_eq!(fixture.run(&["--help"]).status.code(), Some(0));
}
