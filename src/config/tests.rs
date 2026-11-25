use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::config::{Profile, ProfileStore};

mod activation;
mod crud;
mod errors;
mod validation;

/// Store rooted in a fresh temporary directory, kept alive by the `TempDir`.
pub(crate) fn temp_store() -> (TempDir, ProfileStore) {
    let dir = tempfile::tempdir().expect("create temporary directory");
    let store = ProfileStore::new(dir.path().join(".pulumi"));
    (dir, store)
}

pub(crate) fn profile(name: &str, backend: &str) -> Profile {
    Profile::new(name.to_owned(), backend.to_owned()).expect("valid test profile")
}

pub(crate) fn write_raw(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent directory");
    }
    std::fs::write(path, contents).expect("write fixture file");
}

pub(crate) fn raw_entries(dir: impl AsRef<Path>) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("read directory")
        .map(|entry| {
            entry
                .expect("dir entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    names.sort();
    names
}

pub(crate) fn error_chain(err: &anyhow::Error) -> String {
    format!("{err:#}")
}

pub(crate) fn path_text(path: &Path) -> String {
    path.display().to_string()
}

/// Pulumi home directory backing `store`, derived from the public paths.
pub(crate) fn root_of(store: &ProfileStore) -> PathBuf {
    store
        .profiles_path()
        .parent()
        .expect("profiles path has a parent")
        .to_path_buf()
}

pub(crate) fn read_to_string(path: &Path) -> String {
    std::fs::read_to_string(path).expect("read fixture file")
}

pub(crate) fn profile_names(store: &ProfileStore) -> Vec<String> {
    store
        .read()
        .expect("read profiles")
        .into_iter()
        .map(|profile| profile.name)
        .collect()
}

pub(crate) fn profiles_bytes(store: &ProfileStore) -> Vec<u8> {
    std::fs::read(store.profiles_path()).expect("read profiles file")
}
