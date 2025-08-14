use crate::config::Profile;
use anyhow::Result;
use inquire::{InquireError, Select, Text};

pub struct ProfileSelector {
    profiles: Vec<Profile>,
}

impl ProfileSelector {
    pub fn new(profiles: Vec<Profile>) -> Self {
        Self { profiles }
    }

    pub fn run(&mut self) -> Result<Option<String>> {
        if self.profiles.is_empty() {
            return Ok(None);
        }

        let options: Vec<String> = self.profiles.iter().map(format_profile_display).collect();

        let ans = Select::new("Select Pulumi Profile:", options)
            .with_page_size(10)
            .with_help_message("↑↓ to move, enter to select, type to filter")
            .prompt();

        match ans {
            Ok(selected_display) => {
                // Find the profile that matches the selected display string
                let selected_profile = self
                    .profiles
                    .iter()
                    .find(|profile| format_profile_display(profile) == selected_display)
                    .map(|profile| profile.name.clone());

                Ok(selected_profile)
            }
            Err(InquireError::OperationCanceled) => Ok(None),
            Err(InquireError::OperationInterrupted) => Ok(None),
            Err(e) => Err(anyhow::anyhow!("Selection failed: {}", e)),
        }
    }
}

pub fn prompt_for_profile_details() -> Result<(String, String)> {
    let name = Text::new("Profile name:")
        .with_help_message("Enter a unique name for this profile")
        .prompt()?;

    let backend = Text::new("Backend URL:")
        .with_help_message("e.g., s3://my-bucket/state, file://./state, https://api.pulumi.com")
        .prompt()?;

    Ok((name, backend))
}

pub fn prompt_for_backend_url() -> Result<String> {
    let backend = Text::new("New backend URL:")
        .with_help_message("e.g., s3://my-bucket/state, file://./state, https://api.pulumi.com")
        .prompt()?;

    Ok(backend)
}

fn format_profile_display(profile: &Profile) -> String {
    format!("{} -> {}", profile.name, profile.backend)
}