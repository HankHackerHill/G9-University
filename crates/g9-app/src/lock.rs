//! Single-instance guard (app-4). One process at a time may run the backend
//! over a database: open-time recovery treats an open timer interval as a crash
//! and closes it, so a second process (the CLI, a second window) opening the
//! same data would interrupt a live session.
//!
//! The guard is an exclusive OS lock (`File::try_lock`) on `<database>.lock`,
//! taken before recovery and held by the context for its lifetime. The OS
//! releases it when the file is closed, so a crashed process leaves no stale
//! lock behind; the lock file itself is empty and harmless.

use g9_domain::G9Error;
use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

/// An exclusive hold on one database's data. Dropping it releases the lock.
#[derive(Debug)]
pub struct DataLock {
    _file: File,
    path: PathBuf,
}

impl DataLock {
    /// Take the lock for `database`, or refuse with
    /// `RESOURCE_UNAVAILABLE app.data_in_use` when another process holds it.
    pub fn acquire(database: &Path) -> Result<Self, G9Error> {
        let path = lock_path(database);
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(&path).map_err(|e| {
            G9Error::resource_unavailable(
                "app.data_lock_unavailable",
                format!("the data lock could not be opened ({:?})", e.kind()),
            )
        })?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file, path }),
            Err(TryLockError::WouldBlock) => Err(G9Error::resource_unavailable(
                "app.data_in_use",
                "another G9 process is using this data; close it first",
            )),
            Err(TryLockError::Error(e)) => Err(G9Error::resource_unavailable(
                "app.data_lock_unavailable",
                format!("the data lock could not be taken ({:?})", e.kind()),
            )),
        }
    }

    /// Path of the lock file.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// `<database>.lock` beside the database file.
pub fn lock_path(database: &Path) -> PathBuf {
    let mut name = database.as_os_str().to_owned();
    name.push(".lock");
    PathBuf::from(name)
}
