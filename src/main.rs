mod cli;
mod config;
mod shell;
mod ui;

use anyhow::{Context, Result, bail};
use clap::Parser;

use cli::Cli;
use config::{ProfileStore, Selection, validate_value};

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
        if cli.json {
            print_json(&cli, &store.current_selection()?)?;
        } else if cli.context {
            let shell = cli.shell.unwrap_or_else(shell::Shell::detect);
            println!("{}", shell.context_command(&store.current_selection()?));
        } else if let Some(backend) = store.current_backend()? {
            println!("{backend}");
        }
    } else if cli.deactivate {
        if cli.current {
            emit(&cli, &Selection::default())?;
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
            emit(cli, &store.resolve_selection(value)?)?;
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
        emit(
            cli,
            &Selection {
                profile: Some(selected.name.clone()),
                backend: Some(selected.backend.clone()),
            },
        )?;
    } else {
        store.activate(&selected.name)?;
        println!(
            "Pulumi profile activated: {} ({})",
            selected.name, selected.backend
        );
    }
    Ok(())
}

fn emit(cli: &Cli, selection: &Selection) -> Result<()> {
    if cli.json {
        print_json(cli, selection)?;
    } else {
        let shell = cli.shell.unwrap_or_else(shell::Shell::detect);
        if cli.context {
            println!("{}", shell.context_command(selection));
        } else {
            println!("{}", shell.command(selection.backend.as_deref()));
        }
    }
    Ok(())
}

fn print_json(cli: &Cli, selection: &Selection) -> Result<()> {
    if cli.context {
        #[derive(serde::Serialize)]
        struct Context<'a> {
            profile: Option<&'a str>,
            backend: Option<&'a str>,
        }
        println!(
            "{}",
            serde_json::to_string(&Context {
                profile: selection.profile.as_deref(),
                backend: selection.backend.as_deref(),
            })?
        );
    } else {
        #[derive(serde::Serialize)]
        struct Update<'a> {
            backend: Option<&'a str>,
        }
        println!(
            "{}",
            serde_json::to_string(&Update {
                backend: selection.backend.as_deref(),
            })?
        );
    }
    Ok(())
}
