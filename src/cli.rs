use clap::{ArgGroup, Parser};

use crate::shell::Shell;

#[derive(Debug, Parser)]
#[command(version, about)]
#[command(group(ArgGroup::new("action").args([
    "activate", "deactivate", "new", "add", "edit", "delete", "list", "show_current"
])))]
#[command(group(ArgGroup::new("backend_output").args(["current", "show_current"]).multiple(true)))]
pub struct Cli {
    /// Activate a saved profile without prompting
    #[arg(short, long, value_name = "PROFILE")]
    pub activate: Option<String>,

    /// Deactivate the profile
    #[arg(short, long)]
    pub deactivate: bool,

    /// Use a backend URL or an unsaved profile name
    #[arg(short, long, value_name = "BACKEND")]
    pub new: Option<String>,

    /// Output commands for the current shell without changing saved state
    #[arg(short, long, conflicts_with_all = ["add", "edit", "delete", "list", "show_current"])]
    pub current: bool,

    /// Add a profile interactively
    #[arg(long)]
    pub add: bool,

    /// Edit a profile's backend URL
    #[arg(long, value_name = "PROFILE")]
    pub edit: Option<String>,

    /// Delete a profile (also clears its persisted selection)
    #[arg(long, value_name = "PROFILE")]
    pub delete: Option<String>,

    /// List saved profiles
    #[arg(short, long)]
    pub list: bool,

    /// Print the persisted backend URL, or nothing if inactive
    #[arg(long)]
    pub show_current: bool,

    /// Shell syntax for --current (ignored with --json)
    #[arg(long, requires = "current", value_enum)]
    pub shell: Option<Shell>,

    /// Output backend JSON with --current or --show-current
    #[arg(long, requires = "backend_output")]
    pub json: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn command_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn actions_are_mutually_exclusive() {
        for args in [
            vec!["pps", "--add", "--list"],
            vec!["pps", "-a", "dev", "-d"],
            vec!["pps", "-n", "backend", "--delete", "dev"],
            vec!["pps", "--show-current", "-l"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
    }

    #[test]
    fn current_only_accepts_activation_options() {
        for action in ["--add", "--list", "--show-current"] {
            assert!(Cli::try_parse_from(["pps", "--current", action]).is_err());
        }
        assert!(Cli::try_parse_from(["pps", "--json"]).is_err());
        assert!(Cli::try_parse_from(["pps", "--shell", "nu"]).is_err());
        assert!(Cli::try_parse_from(["pps", "-c", "--json", "--shell", "nu"]).is_ok());
        assert!(Cli::try_parse_from(["pps", "-c", "-a", "dev"]).is_ok());
        assert!(Cli::try_parse_from(["pps", "-c", "--json", "-d"]).is_ok());
        assert!(Cli::try_parse_from(["pps", "--show-current", "--json"]).is_ok());
    }
}
