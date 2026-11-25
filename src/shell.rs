use std::{env, path::Path};

use clap::ValueEnum;

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
        let Some(backend) = backend else {
            return match self {
                Self::Posix => "unset PULUMI_BACKEND_URL",
                Self::Fish => "set -e PULUMI_BACKEND_URL",
                Self::Nu => "hide-env --ignore-errors PULUMI_BACKEND_URL",
            }
            .into();
        };

        match self {
            Self::Posix => {
                let quoted = backend.replace('\'', "'\\''");
                format!("export PULUMI_BACKEND_URL='{quoted}'")
            }
            Self::Fish => {
                let quoted = backend.replace('\\', "\\\\").replace('\'', "\\'");
                format!("set -gx PULUMI_BACKEND_URL '{quoted}'")
            }
            Self::Nu => {
                let quoted = backend.replace('\\', "\\\\").replace('"', "\\\"");
                format!("$env.PULUMI_BACKEND_URL = \"{quoted}\"")
            }
        }
    }
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
}
