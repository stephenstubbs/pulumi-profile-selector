use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub backend: String,
}

impl Profile {
    pub fn new(name: String, backend: String) -> Self {
        Self { name, backend }
    }
}

pub fn read_pulumi_profiles() -> Result<Vec<Profile>> {
    let profiles_path = get_pulumi_profiles_path()?;

    if !profiles_path.exists() {
        // Create empty profiles file if it doesn't exist
        let empty_profiles: Vec<Profile> = Vec::new();
        save_pulumi_profiles(&empty_profiles)?;
        return Ok(empty_profiles);
    }

    let content = fs::read_to_string(&profiles_path)
        .with_context(|| format!("Failed to read Pulumi profiles file: {profiles_path:?}"))?;

    let profiles: Vec<Profile> = serde_json::from_str(&content)
        .with_context(|| "Failed to parse Pulumi profiles JSON")?;

    Ok(profiles)
}

pub fn save_pulumi_profiles(profiles: &[Profile]) -> Result<()> {
    let profiles_path = get_pulumi_profiles_path()?;

    // Create .pulumi directory if it doesn't exist
    if let Some(parent) = profiles_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(profiles)
        .with_context(|| "Failed to serialize profiles to JSON")?;

    fs::write(&profiles_path, content)
        .with_context(|| format!("Failed to write Pulumi profiles file: {profiles_path:?}"))?;

    Ok(())
}

pub fn add_profile(name: String, backend: String) -> Result<()> {
    let mut profiles = read_pulumi_profiles()?;
    
    // Check if profile already exists
    if profiles.iter().any(|p| p.name == name) {
        return Err(anyhow::anyhow!("Profile '{}' already exists", name));
    }

    profiles.push(Profile::new(name, backend));
    save_pulumi_profiles(&profiles)?;

    Ok(())
}

pub fn edit_profile(name: &str, new_backend: String) -> Result<()> {
    let mut profiles = read_pulumi_profiles()?;
    
    // Find and update the profile
    if let Some(profile) = profiles.iter_mut().find(|p| p.name == name) {
        profile.backend = new_backend;
        save_pulumi_profiles(&profiles)?;
        Ok(())
    } else {
        Err(anyhow::anyhow!("Profile '{}' not found", name))
    }
}

pub fn delete_profile(name: &str) -> Result<()> {
    let mut profiles = read_pulumi_profiles()?;
    
    let original_len = profiles.len();
    profiles.retain(|p| p.name != name);
    
    if profiles.len() == original_len {
        return Err(anyhow::anyhow!("Profile '{}' not found", name));
    }

    save_pulumi_profiles(&profiles)?;
    Ok(())
}

fn get_pulumi_profiles_path() -> Result<PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Unable to determine home directory"))?;

    Ok(home_dir.join(".pulumi").join("profiles.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_creation() {
        let profile = Profile::new("test".to_string(), "file://./state".to_string());
        assert_eq!(profile.name, "test");
        assert_eq!(profile.backend, "file://./state");
    }

    #[test]
    fn test_json_serialization() {
        let profiles = vec![
            Profile::new("dev".to_string(), "s3://pulumi-state-dev".to_string()),
            Profile::new("prod".to_string(), "s3://pulumi-state-prod".to_string()),
        ];

        let json = serde_json::to_string(&profiles).unwrap();
        let deserialized: Vec<Profile> = serde_json::from_str(&json).unwrap();

        assert_eq!(profiles.len(), deserialized.len());
        assert_eq!(profiles[0].name, deserialized[0].name);
        assert_eq!(profiles[0].backend, deserialized[0].backend);
    }
}