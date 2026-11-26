use crate::config::storage::{LockGuard, write_atomic};
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

mod storage;

#[cfg(test)]
mod tests;

const PROFILES_FILE: &str = "profiles.json";
const CURRENT_PROFILE_FILE: &str = "current_profile";
const LOCK_FILE: &str = ".profiles.lock";

/// A named Pulumi backend URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub backend: String,
}

impl Profile {
    pub fn new(name: String, backend: String) -> Result<Self> {
        let profile = Self { name, backend };
        profile.validate()?;
        Ok(profile)
    }

    fn validate(&self) -> Result<()> {
        validate_value(&self.name, "Profile name")?;
        validate_value(&self.backend, "Backend URL")
    }
}

impl fmt::Display for Profile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} -> {}", self.name, self.backend)
    }
}

/// A resolved non-secret selection. `profile` is set only when the value is a
/// saved profile name; raw backends stay unnamed even when URLs coincide.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    pub profile: Option<String>,
    pub backend: Option<String>,
}

/// Validate a single-line value without rejecting literal shell metacharacters.
pub fn validate_value(value: &str, label: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{label} must not be empty");
    }
    if value != value.trim() {
        bail!("{label} must not start or end with whitespace: {value:?}");
    }
    if let Some(control) = value.chars().find(|c| c.is_control()) {
        bail!("{label} must not contain control characters (found {control:?})");
    }
    Ok(())
}

/// Profiles and the active profile, rooted at a Pulumi home directory.
#[derive(Debug, Clone)]
pub struct ProfileStore {
    root: PathBuf,
}

impl ProfileStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// `PULUMI_HOME` when set to a non-empty value, otherwise `~/.pulumi`.
    pub fn from_env() -> Result<Self> {
        let root = root_from_env(std::env::var_os("PULUMI_HOME"), std::env::home_dir())?;
        Ok(Self::new(root))
    }

    pub fn profiles_path(&self) -> PathBuf {
        self.root.join(PROFILES_FILE)
    }

    pub fn current_profile_path(&self) -> PathBuf {
        self.root.join(CURRENT_PROFILE_FILE)
    }

    fn lock_path(&self) -> PathBuf {
        self.root.join(LOCK_FILE)
    }

    /// Load all profiles. A missing file is an empty list and never creates
    /// files or directories.
    pub fn read(&self) -> Result<Vec<Profile>> {
        let path = self.profiles_path();
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => {
                return Err(err)
                    .with_context(|| format!("Failed to read profiles file: {}", path.display()));
            }
        };

        let profiles: Vec<Profile> = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse profiles file: {}", path.display()))?;

        let mut names = HashSet::with_capacity(profiles.len());
        for profile in &profiles {
            profile.validate().with_context(|| {
                format!("Invalid profile {:?} in {}", profile.name, path.display())
            })?;
            if !names.insert(profile.name.as_str()) {
                bail!(
                    "Duplicate profile name '{}' in {}",
                    profile.name,
                    path.display()
                );
            }
        }

        Ok(profiles)
    }

    pub fn add(&self, profile: Profile) -> Result<()> {
        profile.validate()?;
        self.modify(|profiles| {
            if profiles
                .iter()
                .any(|existing| existing.name == profile.name)
            {
                bail!("Profile '{}' already exists", profile.name);
            }
            profiles.push(profile);
            Ok(())
        })
    }

    pub fn edit(&self, name: &str, backend: String) -> Result<()> {
        validate_value(&backend, "Backend URL")?;
        self.modify(
            |profiles| match profiles.iter_mut().find(|p| p.name == name) {
                Some(profile) => {
                    profile.backend = backend;
                    Ok(())
                }
                None => bail!("Profile '{name}' not found"),
            },
        )
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        self.modify(|profiles| {
            let remaining = profiles.len();
            profiles.retain(|profile| profile.name != name);
            if profiles.len() == remaining {
                bail!("Profile '{name}' not found");
            }
            Ok(())
        })?;
        self.clear_active(name)
    }

    /// Store a profile name, or a raw backend URL for `--new`, as the active
    /// profile.
    pub fn activate(&self, value: &str) -> Result<()> {
        validate_value(value, "Profile name or backend URL")?;
        write_atomic(&self.current_profile_path(), value.as_bytes())
    }

    /// Remove the active profile. Returns whether one was active.
    pub fn deactivate(&self) -> Result<bool> {
        remove_if_present(&self.current_profile_path())
    }

    /// Resolve the active profile to its backend URL. Values that are not a
    /// known profile name are returned unchanged, which supports backends
    /// activated with `--new`.
    pub fn current_backend(&self) -> Result<Option<String>> {
        Ok(self.current_selection()?.backend)
    }

    /// Resolve the active value to profile identity plus backend URL.
    pub fn current_selection(&self) -> Result<Selection> {
        match self.read_current()? {
            Some(active) => self.match_saved(&active),
            None => Ok(Selection::default()),
        }
    }

    /// Resolve an explicit activation value to profile identity plus backend.
    pub fn resolve_selection(&self, value: &str) -> Result<Selection> {
        validate_value(value, "Profile name or backend URL")?;
        self.match_saved(value)
    }

    fn match_saved(&self, value: &str) -> Result<Selection> {
        Ok(
            match self
                .read()?
                .into_iter()
                .find(|profile| profile.name == value)
            {
                Some(profile) => Selection {
                    profile: Some(profile.name),
                    backend: Some(profile.backend),
                },
                None => Selection {
                    profile: None,
                    backend: Some(value.to_owned()),
                },
            },
        )
    }

    fn read_current(&self) -> Result<Option<String>> {
        let path = self.current_profile_path();
        let content = match fs::read_to_string(&path) {
            Ok(content) => content,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => {
                return Err(err).with_context(|| format!("Failed to read {}", path.display()));
            }
        };

        let active = content.trim();
        if active.is_empty() {
            return Ok(None);
        }
        validate_value(active, "Current profile")
            .with_context(|| format!("Invalid current profile in {}", path.display()))?;
        Ok(Some(active.to_owned()))
    }

    fn clear_active(&self, name: &str) -> Result<()> {
        if self.read_current()?.as_deref() == Some(name) {
            remove_if_present(&self.current_profile_path())?;
        }
        Ok(())
    }

    fn modify<T>(&self, update: impl FnOnce(&mut Vec<Profile>) -> Result<T>) -> Result<T> {
        fs::create_dir_all(&self.root)
            .with_context(|| format!("Failed to create directory: {}", self.root.display()))?;

        let _lock = LockGuard::acquire(&self.lock_path())?;
        let mut profiles = self.read()?;
        let updated = update(&mut profiles)?;

        let mut content = serde_json::to_string_pretty(&profiles)
            .context("Failed to serialise profiles as JSON")?;
        content.push('\n');
        write_atomic(&self.profiles_path(), content.as_bytes())?;

        Ok(updated)
    }
}

fn root_from_env(pulumi_home: Option<OsString>, home: Option<PathBuf>) -> Result<PathBuf> {
    if let Some(root) = usable_pulumi_home(pulumi_home) {
        return Ok(root);
    }

    match home {
        Some(home) => Ok(home.join(".pulumi")),
        None => bail!("Unable to determine home directory"),
    }
}

fn usable_pulumi_home(pulumi_home: Option<OsString>) -> Option<PathBuf> {
    let root = pulumi_home?;
    if root.is_empty() {
        return None;
    }
    if root.to_str().is_some_and(|root| root.trim().is_empty()) {
        return None;
    }
    Some(PathBuf::from(root))
}

/// Remove `path`, treating an already absent file as success.
fn remove_if_present(path: &Path) -> Result<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err).with_context(|| format!("Failed to remove {}", path.display())),
    }
}
