use anyhow::{Context, Result};
use inquire::{InquireError, Select, Text, validator::Validation};

use crate::config::{Profile, validate_value};

pub fn select_profile(profiles: &[Profile]) -> Result<Option<&Profile>> {
    match Select::new("Select Pulumi Profile:", profiles.iter().collect())
        .with_page_size(10)
        .prompt_skippable()
    {
        Ok(profile) => Ok(profile),
        Err(InquireError::OperationInterrupted) => Ok(None),
        Err(error) => Err(error).context("Profile selection failed"),
    }
}

pub fn prompt_for_profile_details() -> Result<Profile> {
    let name = Text::new("Profile name:")
        .with_help_message("Enter a unique name for this profile")
        .with_validator(|input: &str| Ok(validation(input, "Profile name")))
        .prompt()?;
    let backend = prompt_for_backend_url("")?;
    Profile::new(name, backend)
}

pub fn prompt_for_backend_url(default: &str) -> Result<String> {
    Ok(Text::new("Backend URL:")
        .with_help_message("e.g., s3://my-bucket/state, file://./state, https://api.pulumi.com")
        .with_initial_value(default)
        .with_validator(|input: &str| Ok(validation(input, "Backend URL")))
        .prompt()?)
}

fn validation(value: &str, label: &str) -> Validation {
    match validate_value(value, label) {
        Ok(()) => Validation::Valid,
        Err(error) => Validation::Invalid(error.to_string().into()),
    }
}
