use anyhow::{Context, Result, bail};
use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::Write;
use std::path::Path;
use tempfile::{NamedTempFile, PersistError};

/// Replace a file atomically using a synced sibling temporary file.
pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .with_context(|| format!("Path has no parent directory: {}", path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("Failed to create directory: {}", parent.display()))?;

    // Mode 0600 protects backend URLs containing credentials.
    let mut temp = NamedTempFile::new_in(parent)
        .with_context(|| format!("Failed to create temporary file in {}", parent.display()))?;
    temp.write_all(contents)
        .and_then(|()| temp.as_file().sync_all())
        .with_context(|| format!("Failed to write temporary file for {}", path.display()))?;

    match temp.persist(path) {
        Ok(_persisted) => Ok(()),
        Err(PersistError { error, file }) => {
            drop(file);
            Err(anyhow::Error::new(error).context(format!("Failed to replace {}", path.display())))
        }
    }
}

/// Lock a separate inode so atomic replacements cannot bypass the lock.
pub(crate) struct LockGuard {
    _file: File,
}

impl LockGuard {
    pub(crate) fn acquire(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .with_context(|| format!("Failed to open lock file: {}", path.display()))?;

        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => bail!(
                "Another process is modifying Pulumi profiles (lock held on {})",
                path.display()
            ),
            Err(TryLockError::Error(err)) => {
                Err(err).with_context(|| format!("Failed to lock {}", path.display()))
            }
        }
    }
}
