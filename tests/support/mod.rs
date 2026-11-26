// Shared CLI fixture; each integration test target uses a subset of these.
#![allow(dead_code)]

use std::{
    fs,
    path::PathBuf,
    process::{Command, Output},
};

use serde_json::Value;
use tempfile::TempDir;

pub struct Fixture {
    pub home: TempDir,
    pub root: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let home = tempfile::tempdir().unwrap();
        let root = home.path().join("pulumi");
        Self { home, root }
    }

    pub fn profiles(&self, value: Value) {
        fs::create_dir_all(&self.root).unwrap();
        fs::write(self.root.join("profiles.json"), value.to_string()).unwrap();
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_pulumi-profile-selector"));
        command
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env("PULUMI_HOME", &self.root)
            .env("SHELL", "/gnu/bin/bash");
        command
    }

    pub fn run(&self, args: &[&str]) -> Output {
        self.command().args(args).output().unwrap()
    }

    pub fn success(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    }

    pub fn failure(&self, args: &[&str]) -> String {
        let output = self.run(args);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        String::from_utf8(output.stderr).unwrap()
    }

    pub fn saved(&self) -> String {
        fs::read_to_string(self.root.join("current_profile")).unwrap()
    }
}
