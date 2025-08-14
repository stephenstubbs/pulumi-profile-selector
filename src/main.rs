mod config;
mod ui;

use anyhow::Result;
use clap::{Arg, Command, ArgAction};
use config::{read_pulumi_profiles, add_profile, edit_profile, delete_profile};
use ui::{ProfileSelector, prompt_for_profile_details, prompt_for_backend_url};
use std::path::PathBuf;

fn main() -> Result<()> {
    let matches = Command::new("pulumi-profile-selector")
        .version("0.1.0")
        .author("Pulumi Profile Selector - Rust Edition")
        .about("Interactive Pulumi profile selector")
        .arg(
            Arg::new("activate")
                .short('a')
                .long("activate")
                .help("Activate a specific profile by name (skips interactive selection)")
                .value_name("PROFILE"),
        )
        .arg(
            Arg::new("deactivate")
                .short('d')
                .long("deactivate")
                .help("Deactivate PULUMI_BACKEND_URL")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("new")
                .short('n')
                .long("new")
                .help("Set a profile name that is not available in the list")
                .value_name("PROFILE"),
        )
        .arg(
            Arg::new("current")
                .short('c')
                .long("current")
                .help("Output the profile name only (for setting in current shell)")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("add")
                .long("add")
                .help("Add a new profile interactively")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("edit")
                .long("edit")
                .help("Edit an existing profile's backend URL")
                .value_name("PROFILE"),
        )
        .arg(
            Arg::new("delete")
                .long("delete")
                .help("Delete a profile")
                .value_name("PROFILE"),
        )
        .arg(
            Arg::new("list")
                .short('l')
                .long("list")
                .help("List all profiles")
                .action(ArgAction::SetTrue),
        )
        .get_matches();

    let current_profile_path = get_current_profile_path()?;
    let current_shell_mode = matches.get_flag("current");

    // Handle profile management commands first
    if matches.get_flag("add") {
        let (name, backend) = prompt_for_profile_details()?;
        add_profile(name.clone(), backend)?;
        println!("Profile '{}' added successfully", name);
        return Ok(());
    }

    if let Some(profile_name) = matches.get_one::<String>("edit") {
        let new_backend = prompt_for_backend_url()?;
        edit_profile(profile_name, new_backend)?;
        println!("Profile '{}' updated successfully", profile_name);
        return Ok(());
    }

    if let Some(profile_name) = matches.get_one::<String>("delete") {
        delete_profile(profile_name)?;
        println!("Profile '{}' deleted successfully", profile_name);
        return Ok(());
    }

    if matches.get_flag("list") {
        let profiles = read_pulumi_profiles()?;
        if profiles.is_empty() {
            println!("No profiles found.");
        } else {
            println!("Available profiles:");
            for profile in &profiles {
                println!("  {} -> {}", profile.name, profile.backend);
            }
        }
        return Ok(());
    }

    // Handle deactivation
    if matches.get_flag("deactivate") {
        if current_shell_mode {
            // Output shell-specific unset command
            print_shell_command(None);
        } else {
            if current_profile_path.exists() {
                std::fs::remove_file(&current_profile_path)?;
                println!("Pulumi profile deactivated");
            } else {
                println!("No active Pulumi profile to deactivate");
            }
        }
        return Ok(());
    }

    // Handle new profile (doesn't require reading existing profiles)
    if let Some(profile_name) = matches.get_one::<String>("new") {
        if current_shell_mode {
            // Output shell-specific export command
            print_shell_command(Some(profile_name));
        } else {
            // Create .pulumi directory if it doesn't exist
            if let Some(parent) = current_profile_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            
            // Write profile name to file
            std::fs::write(&current_profile_path, profile_name)?;
            println!("Pulumi profile activated: {profile_name}");
        }
        return Ok(());
    }

    let profiles = read_pulumi_profiles()?;

    if profiles.is_empty() {
        eprintln!("No Pulumi profiles found in ~/.pulumi/profiles.json");
        eprintln!("Use --add to create your first profile");
        std::process::exit(1);
    }

    // Handle direct profile activation
    let selected_profile = if let Some(profile_name) = matches.get_one::<String>("activate") {
        // Validate that the profile exists and get its backend URL
        if let Some(profile) = profiles.iter().find(|p| &p.name == profile_name) {
            Some((profile.name.clone(), profile.backend.clone()))
        } else {
            eprintln!("Profile '{}' not found in Pulumi profiles", profile_name);
            eprintln!("Available profiles:");
            for profile in &profiles {
                eprintln!("  {}", profile.name);
            }
            std::process::exit(1);
        }
    } else {
        // Run interactive selector
        let mut selector = ProfileSelector::new(profiles.clone());
        if let Some(selected_name) = selector.run()? {
            if let Some(profile) = profiles.iter().find(|p| p.name == selected_name) {
                Some((profile.name.clone(), profile.backend.clone()))
            } else {
                None
            }
        } else {
            None
        }
    };

    match selected_profile {
        Some((profile_name, backend_url)) => {
            if current_shell_mode {
                // Output shell-specific export command with backend URL
                print_shell_command_with_backend(Some(&backend_url));
            } else {
                // Create .pulumi directory if it doesn't exist
                if let Some(parent) = current_profile_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                
                // Write profile name to file
                std::fs::write(&current_profile_path, &profile_name)?;
                println!("Pulumi profile activated: {} ({})", profile_name, backend_url);
            }
        }
        None => {
            println!("No profile selected");
            std::process::exit(1);
        }
    }

    Ok(())
}

fn get_current_profile_path() -> Result<PathBuf> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Unable to determine home directory"))?;
    
    Ok(home_dir.join(".pulumi").join("current_profile"))
}

fn print_shell_command(profile_name: Option<&str>) {
    // This function is for backwards compatibility, but for Pulumi we need the backend URL
    // We'll read the current profile to get the backend URL
    if let Some(name) = profile_name {
        if let Ok(profiles) = read_pulumi_profiles() {
            if let Some(profile) = profiles.iter().find(|p| &p.name == name) {
                print_shell_command_with_backend(Some(&profile.backend));
                return;
            }
        }
        // Fallback: just print the profile name (this shouldn't happen in normal usage)
        print_shell_command_with_backend(Some(name));
    } else {
        print_shell_command_with_backend(None);
    }
}

fn print_shell_command_with_backend(backend_url: Option<&str>) {
    // Detect the shell from SHELL environment variable
    let shell = std::env::var("SHELL").unwrap_or_default();
    
    match backend_url {
        Some(url) => {
            if shell.contains("nu") || shell.contains("nushell") {
                // Nushell syntax
                print!("$env.PULUMI_BACKEND_URL = \"{}\"", url);
            } else if shell.contains("fish") {
                // Fish syntax
                print!("set -gx PULUMI_BACKEND_URL \"{}\"", url);
            } else {
                // Default to bash/zsh/POSIX syntax
                print!("export PULUMI_BACKEND_URL=\"{}\"", url);
            }
        }
        None => {
            if shell.contains("nu") || shell.contains("nushell") {
                // Nushell syntax for unsetting
                print!("hide-env PULUMI_BACKEND_URL");
            } else if shell.contains("fish") {
                // Fish syntax for unsetting
                print!("set -e PULUMI_BACKEND_URL");
            } else {
                // Default to bash/zsh/POSIX syntax
                print!("unset PULUMI_BACKEND_URL");
            }
        }
    }
}