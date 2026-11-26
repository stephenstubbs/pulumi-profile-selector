use std::{env, path::Path};

use clap::ValueEnum;

use crate::config::Selection;

/// Non-secret saved-profile identity exported alongside the backend.
pub const PROFILE_VAR: &str = "PULUMI_PROFILE_SELECTOR_PROFILE";

const BACKEND_VAR: &str = "PULUMI_BACKEND_URL";

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Shell {
    Posix,
    Fish,
    Nu,
}

impl Shell {
    pub fn detect() -> Self {
        env::var_os("SHELL")
            .as_deref()
            .map(Path::new)
            .map_or(Self::Posix, Self::from_path)
    }

    fn from_path(path: &Path) -> Self {
        match path.file_name().and_then(|name| name.to_str()) {
            Some("nu" | "nushell" | "nu.exe" | "nushell.exe") => Self::Nu,
            Some("fish" | "fish.exe") => Self::Fish,
            _ => Self::Posix,
        }
    }

    pub fn command(self, backend: Option<&str>) -> String {
        self.assignment(BACKEND_VAR, backend)
    }

    /// Backend plus profile identity, leaving unnamed backends without one.
    pub fn context_command(self, selection: &Selection) -> String {
        [
            self.assignment(BACKEND_VAR, selection.backend.as_deref()),
            self.assignment(PROFILE_VAR, selection.profile.as_deref()),
        ]
        .join("\n")
    }

    fn assignment(self, name: &str, value: Option<&str>) -> String {
        match (self, value) {
            (Self::Posix, Some(value)) => format!("export {name}={}", posix_quote(value)),
            (Self::Posix, None) => format!("unset {name}"),
            (Self::Fish, Some(value)) => format!("set -gx {name} {}", fish_quote(value)),
            (Self::Fish, None) => format!("set -e {name}"),
            (Self::Nu, Some(value)) => format!("$env.{name} = {}", nu_quote(value)),
            (Self::Nu, None) => format!("hide-env --ignore-errors {name}"),
        }
    }
}

fn posix_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn fish_quote(value: &str) -> String {
    let quoted = value.replace('\\', "\\\\").replace('\'', "\\'");
    format!("'{quoted}'")
}

fn nu_quote(value: &str) -> String {
    let quoted = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{quoted}\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_executable_not_parent_directories() {
        assert_eq!(Shell::from_path(Path::new("/gnu/bin/bash")), Shell::Posix);
        assert_eq!(Shell::from_path(Path::new("/usr/bin/nu")), Shell::Nu);
        assert_eq!(Shell::from_path(Path::new("/usr/bin/fish")), Shell::Fish);
        assert_eq!(Shell::from_path(Path::new("nushell")), Shell::Nu);
        assert_eq!(Shell::from_path(Path::new("")), Shell::Posix);
    }

    #[test]
    fn quotes_shell_metacharacters() {
        let backend = r#"s3://bucket/a'"\$()`"#;
        assert_eq!(
            Shell::Posix.command(Some(backend)),
            r#"export PULUMI_BACKEND_URL='s3://bucket/a'\''"\$()`'"#
        );
        assert_eq!(
            Shell::Fish.command(Some(backend)),
            r#"set -gx PULUMI_BACKEND_URL 's3://bucket/a\'"\\$()`'"#
        );
        assert_eq!(
            Shell::Nu.command(Some(backend)),
            r#"$env.PULUMI_BACKEND_URL = "s3://bucket/a'\"\\$()`""#
        );
    }

    #[test]
    fn unsets_the_backend_for_each_shell() {
        assert_eq!(Shell::Posix.command(None), "unset PULUMI_BACKEND_URL");
        assert_eq!(Shell::Fish.command(None), "set -e PULUMI_BACKEND_URL");
        assert_eq!(
            Shell::Nu.command(None),
            "hide-env --ignore-errors PULUMI_BACKEND_URL"
        );
    }

    #[test]
    fn quotes_a_trailing_backslash() {
        assert_eq!(
            Shell::Posix.command(Some("x\\")),
            "export PULUMI_BACKEND_URL='x\\'"
        );
        assert_eq!(
            Shell::Fish.command(Some("x\\")),
            "set -gx PULUMI_BACKEND_URL 'x\\\\'"
        );
        assert_eq!(
            Shell::Nu.command(Some("x\\")),
            "$env.PULUMI_BACKEND_URL = \"x\\\\\""
        );
    }

    #[test]
    fn context_output_carries_identity_and_backend() {
        let selection = Selection {
            profile: Some("dev".to_owned()),
            backend: Some("s3://state".to_owned()),
        };
        assert_eq!(
            Shell::Posix.context_command(&selection),
            "export PULUMI_BACKEND_URL='s3://state'\nexport PULUMI_PROFILE_SELECTOR_PROFILE='dev'"
        );
        assert_eq!(
            Shell::Fish.context_command(&selection),
            "set -gx PULUMI_BACKEND_URL 's3://state'\nset -gx PULUMI_PROFILE_SELECTOR_PROFILE 'dev'"
        );
        assert_eq!(
            Shell::Nu.context_command(&selection),
            "$env.PULUMI_BACKEND_URL = \"s3://state\"\n$env.PULUMI_PROFILE_SELECTOR_PROFILE = \"dev\""
        );
    }

    #[test]
    fn context_output_clears_unnamed_and_inactive_selections() {
        let raw = Selection {
            profile: None,
            backend: Some("s3://state".to_owned()),
        };
        assert_eq!(
            Shell::Posix.context_command(&raw),
            "export PULUMI_BACKEND_URL='s3://state'\nunset PULUMI_PROFILE_SELECTOR_PROFILE"
        );
        assert_eq!(
            Shell::Nu.context_command(&raw),
            "$env.PULUMI_BACKEND_URL = \"s3://state\"\nhide-env --ignore-errors PULUMI_PROFILE_SELECTOR_PROFILE"
        );
        assert_eq!(
            Shell::Fish.context_command(&Selection::default()),
            "set -e PULUMI_BACKEND_URL\nset -e PULUMI_PROFILE_SELECTOR_PROFILE"
        );
    }

    #[test]
    fn context_output_quotes_identity_literally() {
        let selection = Selection {
            profile: Some(r#"a'b"$() `id`"#.to_owned()),
            backend: Some("s3://x".to_owned()),
        };
        assert_eq!(
            Shell::Posix
                .context_command(&selection)
                .lines()
                .last()
                .unwrap(),
            r#"export PULUMI_PROFILE_SELECTOR_PROFILE='a'\''b"$() `id`'"#
        );
        assert_eq!(
            Shell::Nu
                .context_command(&selection)
                .lines()
                .last()
                .unwrap(),
            r#"$env.PULUMI_PROFILE_SELECTOR_PROFILE = "a'b\"$() `id`""#
        );
    }
}
