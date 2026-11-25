mod cli;
mod config;
mod shell;
mod ui;

use anyhow::{Context, Result, bail};
use clap::Parser;

use cli::Cli;
use config::{ProfileStore, validate_value};

fn main() -> Result<()> {
    let cli = Cli::parse();
    let store = ProfileStore::from_env()?;

    if cli.add {
        let profile = ui::prompt_for_profile_details()?;
        let name = profile.name.clone();
        store.add(profile)?;
        println!("Profile '{name}' added successfully");
    } else if let Some(name) = &cli.edit {
        let profiles = store.read()?;
        let profile = profiles
            .iter()
            .find(|profile| &profile.name == name)
            .with_context(|| format!("Profile '{name}' not found"))?;
        let backend = ui::prompt_for_backend_url(&profile.backend)?;
        store.edit(name, backend)?;
        println!("Profile '{name}' updated successfully");
    } else if let Some(name) = &cli.delete {
        store.delete(name)?;
        println!("Profile '{name}' deleted successfully");
    } else if cli.list {
        let profiles = store.read()?;
        if profiles.is_empty() {
            println!("No profiles found.");
        } else {
            for profile in profiles {
                println!("{profile}");
            }
        }
    } else if cli.show_current {
        let backend = store.current_backend()?;
        if cli.json {
            print_json(backend.as_deref())?;
        } else if let Some(backend) = backend {
            println!("{backend}");
        }
    } else if cli.deactivate {
        if cli.current {
            print_current(&cli, None)?;
        } else if store.deactivate()? {
            println!("Pulumi profile deactivated");
        } else {
            println!("No active Pulumi profile to deactivate");
        }
    } else {
        activate(&cli, &store)?;
    }

    Ok(())
}

fn activate(cli: &Cli, store: &ProfileStore) -> Result<()> {
    if let Some(value) = &cli.new {
        validate_value(value, "Backend or profile name")?;
        if cli.current {
            let profiles = store.read()?;
            let backend = profiles
                .iter()
                .find(|profile| &profile.name == value)
                .map_or(value.as_str(), |profile| &profile.backend);
            print_current(cli, Some(backend))?;
        } else {
            store.activate(value)?;
            println!("Pulumi profile activated: {value}");
        }
        return Ok(());
    }

    let profiles = store.read()?;
    if profiles.is_empty() {
        bail!("No Pulumi profiles found. Use --add to create your first profile");
    }

    let selected = if let Some(name) = &cli.activate {
        profiles
            .iter()
            .find(|profile| &profile.name == name)
            .with_context(|| {
                let available = profiles
                    .iter()
                    .map(|profile| profile.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                format!("Profile '{name}' not found. Available profiles: {available}")
            })?
    } else {
        ui::select_profile(&profiles)?.context("No profile selected")?
    };

    if cli.current {
        print_current(cli, Some(&selected.backend))?;
    } else {
        store.activate(&selected.name)?;
        println!(
            "Pulumi profile activated: {} ({})",
            selected.name, selected.backend
        );
    }
    Ok(())
}

fn print_current(cli: &Cli, backend: Option<&str>) -> Result<()> {
    if cli.json {
        print_json(backend)?;
    } else {
        let shell = cli.shell.unwrap_or_else(shell::Shell::detect);
        println!("{}", shell.command(backend));
    }
    Ok(())
}

fn print_json(backend: Option<&str>) -> Result<()> {
    #[derive(serde::Serialize)]
    struct Update<'a> {
        backend: Option<&'a str>,
    }
    println!("{}", serde_json::to_string(&Update { backend })?);
    Ok(())
}
