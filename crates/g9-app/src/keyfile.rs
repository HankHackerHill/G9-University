//! Approval binding key provisioning (ARCHITECTURE.md §11; ADR-0003;
//! `docs/privacy/data-lifecycle.md`, row "Approval binding key").
//!
//! The binding key is the per-installation HMAC secret that `g9-policy` uses to
//! bind an approval to the exact operation, resource and expected revision. It
//! lives only in the backend process.
//!
//! # Storage decision (2026-11-06, `apps/desktop/README.md` "Binding key")
//!
//! The OS keyring (Secret Service) is **not** used. Every Secret Service client
//! that builds without system libraries was rejected:
//!
//! * `keyring` 4.2.0 (`v1` feature) and `zbus-secret-service-keyring-store`
//!   1.0.1 depend on `zbus` 5 / `zvariant` / `zbus_names`, which the A12
//!   advisory gate refuses (RUSTSEC-2026-0370/0371/0372, unmaintained, no safe
//!   upgrade). No advisory ignore is added for them.
//! * `dbus-secret-service-keyring-store` 1.0.1 with `vendored` statically links
//!   libdbus C sources (AFL-2.1 OR GPL-2.0-or-later), a licence class outside
//!   the `deny.toml` allow-list that cargo-deny cannot see (it reads the
//!   `libdbus-sys` crate metadata only).
//! * `secret-service` 6.0.0-rc.1 (zbus 6) is a release candidate.
//!
//! The documented fallback is therefore the only key store:
//! `<app data>/binding.key`, exactly 32 bytes from the OS CSPRNG (`getrandom`),
//! created atomically with mode `0600` (private temporary file + `hard_link`,
//! so a concurrent process never observes a partial key). Every start adds the
//! warning [`WARN_FILE_FALLBACK`], which the bridge reports in `system.status`
//! (`CommandOutcome.warnings`).
//!
//! # Integrity rules
//!
//! * Key bytes are never logged, serialized, displayed or placed in an error or
//!   warning; warnings name `<app data>` instead of the host path.
//! * A key file that is group/other accessible or has the wrong length is
//!   replaced (warning [`WARN_FILE_ROTATED`]); approvals bound to the old key then
//!   fail verification, which is the fail-closed outcome. A path that is not a
//!   regular file (symlink, directory) is refused with `DATA_CORRUPTION` and is
//!   never followed or deleted.
//! * Randomness is injected (`random`), so tests are deterministic; production
//!   passes [`os_random`].
//!
//! The file store needs Unix permissions; on other platforms provisioning is
//! refused (`PRECONDITION_FAILED binding_key.file_store_unsupported`). Linux is
//! the only shipping platform.

use g9_domain::G9Error;
use g9_policy::{BindingKey, BINDING_KEY_LEN};
use std::fs;
use std::io::{self, Read};
use std::path::Path;

/// File name of the binding key inside the app data directory.
pub const KEY_FILE_NAME: &str = "binding.key";

/// Warning code: the key is kept in the `0600` file, not in the OS keyring.
pub const WARN_FILE_FALLBACK: &str = "binding_key.file_fallback";
/// Warning code: an unusable key file was replaced by a new key.
pub const WARN_FILE_ROTATED: &str = "binding_key.file_rotated";
/// Warning code: a temporary key file could not be removed.
pub const WARN_STALE_FILE: &str = "binding_key.stale_file_not_removed";

/// Format a warning string as reported in `CommandOutcome.warnings`.
pub fn warning(code: &str, message: &str) -> String {
    format!("WARNING {code}: {message}")
}

/// Fill `buf` from the OS CSPRNG.
pub fn os_random(buf: &mut [u8]) -> Result<(), G9Error> {
    getrandom::fill(buf).map_err(|_| {
        G9Error::resource_unavailable(
            "binding_key.os_random_unavailable",
            "the operating system random source is unavailable",
        )
    })
}

/// The provisioned key and the warnings to report. `BindingKey`'s `Debug`
/// output is redacted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionedKey {
    /// The per-installation HMAC key.
    pub key: BindingKey,
    /// Warnings for `system.status` (secret-free, no host paths). The first
    /// entry is always the [`WARN_FILE_FALLBACK`] warning.
    pub warnings: Vec<String>,
}

/// Provision the binding key from `<data_dir>/binding.key` (the directory must
/// already exist): reuse a valid key file, replace an unusable one, or create
/// one. See the module docs for the rules.
pub fn provision(
    data_dir: &Path,
    random: &mut dyn FnMut(&mut [u8]) -> Result<(), G9Error>,
) -> Result<ProvisionedKey, G9Error> {
    let mut warnings = vec![warning(
        WARN_FILE_FALLBACK,
        &format!(
            "the OS keyring is not used (no Secret Service client passes the supply-chain \
             gate); the approval binding key is kept in <app data>/{KEY_FILE_NAME} with \
             mode 0600"
        ),
    )];
    let key = from_file(&data_dir.join(KEY_FILE_NAME), random, &mut warnings)?;
    Ok(ProvisionedKey { key, warnings })
}

fn fresh_key(
    random: &mut dyn FnMut(&mut [u8]) -> Result<(), G9Error>,
) -> Result<[u8; BINDING_KEY_LEN], G9Error> {
    let mut bytes = [0u8; BINDING_KEY_LEN];
    random(&mut bytes)?;
    Ok(bytes)
}

/// Why an existing key file cannot be used as is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyFileProblem {
    /// Symlink, directory or other non-regular file.
    NotRegular,
    /// Group or other permission bits are set.
    PermissiveMode,
    /// Not exactly 32 bytes.
    InvalidLength,
    /// The file exists but cannot be read.
    Unreadable,
}

/// Read the key file; `Ok(None)` when it does not exist. Never reads through a link.
fn read_key_file(path: &Path) -> Result<Option<[u8; BINDING_KEY_LEN]>, KeyFileProblem> {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(KeyFileProblem::Unreadable),
    };
    if !meta.file_type().is_file() {
        return Err(KeyFileProblem::NotRegular);
    }
    if !platform::is_private(&meta) {
        return Err(KeyFileProblem::PermissiveMode);
    }
    let file = fs::File::open(path).map_err(|_| KeyFileProblem::Unreadable)?;
    // The opened file must be the regular file inspected above (a link swapped
    // in between the two calls resolves to a different inode).
    let opened = file.metadata().map_err(|_| KeyFileProblem::Unreadable)?;
    if !platform::same_file(&meta, &opened) {
        return Err(KeyFileProblem::NotRegular);
    }
    let mut bytes = Vec::with_capacity(BINDING_KEY_LEN + 1);
    // Read at most one byte past the key length to detect oversize files.
    file.take(BINDING_KEY_LEN as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| KeyFileProblem::Unreadable)?;
    <[u8; BINDING_KEY_LEN]>::try_from(bytes.as_slice())
        .map(Some)
        .map_err(|_| KeyFileProblem::InvalidLength)
}

/// Reuse a valid key file, replace an unusable one, or create one.
fn from_file(
    file: &Path,
    random: &mut dyn FnMut(&mut [u8]) -> Result<(), G9Error>,
    warnings: &mut Vec<String>,
) -> Result<BindingKey, G9Error> {
    if !platform::FILE_STORE_SUPPORTED {
        return Err(G9Error::precondition_failed(
            "binding_key.file_store_unsupported",
            "the binding key file store needs Unix permissions and no OS keyring is configured",
        ));
    }
    match read_key_file(file) {
        Ok(Some(bytes)) => return Ok(BindingKey::from_bytes(bytes)),
        Ok(None) => {}
        Err(KeyFileProblem::NotRegular) => {
            return Err(G9Error::data_corruption(
                "binding_key.file_not_regular",
                "the binding key path in the app data directory is not a regular file; \
                 inspect it, remove it and restart",
            ))
        }
        Err(KeyFileProblem::Unreadable) => {
            return Err(G9Error::resource_unavailable(
                "binding_key.file_unreadable",
                "the binding key file in the app data directory cannot be read",
            ))
        }
        Err(problem @ (KeyFileProblem::PermissiveMode | KeyFileProblem::InvalidLength)) => {
            let reason = if problem == KeyFileProblem::PermissiveMode {
                "was readable by other users"
            } else {
                "did not hold a 32-byte key"
            };
            fs::remove_file(file).map_err(|_| {
                G9Error::resource_unavailable(
                    "binding_key.file_not_replaceable",
                    "an unusable binding key file could not be removed",
                )
            })?;
            warnings.push(warning(
                WARN_FILE_ROTATED,
                &format!(
                    "the binding key file {reason} and was replaced; approvals granted \
                     earlier must be requested again"
                ),
            ));
        }
    }
    let bytes = fresh_key(random)?;
    if create_key_file(file, &bytes, random, warnings)? {
        return Ok(BindingKey::from_bytes(bytes));
    }
    // Another process created the file between our check and our link: use its key.
    match read_key_file(file) {
        Ok(Some(existing)) => Ok(BindingKey::from_bytes(existing)),
        _ => Err(G9Error::conflict(
            "binding_key.concurrent_creation",
            "another process is creating the binding key; restart the application",
        )),
    }
}

/// Write `bytes` to a private temporary file and link it to `path`.
/// Returns `false` when `path` already exists (nothing is replaced).
fn create_key_file(
    path: &Path,
    bytes: &[u8; BINDING_KEY_LEN],
    random: &mut dyn FnMut(&mut [u8]) -> Result<(), G9Error>,
    warnings: &mut Vec<String>,
) -> Result<bool, G9Error> {
    let create_failed = || {
        G9Error::resource_unavailable(
            "binding_key.file_create_failed",
            "the binding key file could not be created in the app data directory",
        )
    };
    let dir = path.parent().ok_or_else(create_failed)?;
    let mut nonce = [0u8; 8];
    random(&mut nonce)?;
    let suffix: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let tmp = dir.join(format!(".{KEY_FILE_NAME}.{suffix}.tmp"));
    if let Err(e) = platform::write_private_file(&tmp, bytes) {
        // A partially written temporary file we created must not linger; a
        // pre-existing path (`AlreadyExists`) is not ours and is left alone.
        if e.kind() != io::ErrorKind::AlreadyExists {
            let _ = fs::remove_file(&tmp);
        }
        return Err(create_failed());
    }
    let linked = fs::hard_link(&tmp, path);
    if fs::remove_file(&tmp).is_err() {
        warnings.push(warning(
            WARN_STALE_FILE,
            "a temporary key file could not be removed from the app data directory",
        ));
    }
    match linked {
        Ok(()) => {
            platform::sync_dir(dir);
            Ok(true)
        }
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Ok(false),
        Err(_) => Err(create_failed()),
    }
}

#[cfg(unix)]
mod platform {
    use std::fs::{self, File, OpenOptions};
    use std::io::{self, Write};
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
    use std::path::Path;

    pub(super) const FILE_STORE_SUPPORTED: bool = true;

    /// No group/other permission bits.
    pub(super) fn is_private(meta: &fs::Metadata) -> bool {
        meta.permissions().mode() & 0o077 == 0
    }

    /// Same device and inode, and still a private regular file.
    pub(super) fn same_file(inspected: &fs::Metadata, opened: &fs::Metadata) -> bool {
        opened.file_type().is_file()
            && is_private(opened)
            && inspected.dev() == opened.dev()
            && inspected.ino() == opened.ino()
    }

    /// Create a new file with mode `0600` (never follows or replaces an
    /// existing path), write `bytes` and flush them to disk.
    pub(super) fn write_private_file(path: &Path, bytes: &[u8]) -> io::Result<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(bytes)?;
        file.sync_all()
    }

    /// Persist the directory entry (best effort; the key file itself is synced).
    pub(super) fn sync_dir(dir: &Path) {
        if let Ok(handle) = File::open(dir) {
            let _ = handle.sync_all();
        }
    }
}

#[cfg(not(unix))]
mod platform {
    use std::fs;
    use std::io;
    use std::path::Path;

    pub(super) const FILE_STORE_SUPPORTED: bool = false;

    pub(super) fn is_private(_meta: &fs::Metadata) -> bool {
        false
    }

    pub(super) fn same_file(_inspected: &fs::Metadata, _opened: &fs::Metadata) -> bool {
        false
    }

    pub(super) fn write_private_file(_path: &Path, _bytes: &[u8]) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    pub(super) fn sync_dir(_dir: &Path) {}
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use g9_domain::ErrorCategory;
    use std::os::unix::fs::PermissionsExt;

    /// Deterministic byte source: 0, 1, 2, ... across calls.
    fn counter() -> impl FnMut(&mut [u8]) -> Result<(), G9Error> {
        let mut next: u8 = 0;
        move |buf: &mut [u8]| {
            for b in buf.iter_mut() {
                *b = next;
                next = next.wrapping_add(1);
            }
            Ok(())
        }
    }

    fn no_random() -> impl FnMut(&mut [u8]) -> Result<(), G9Error> {
        |_: &mut [u8]| {
            Err(G9Error::resource_unavailable(
                "binding_key.os_random_unavailable",
                "the operating system random source is unavailable",
            ))
        }
    }

    fn first_key() -> [u8; 32] {
        let mut k = [0u8; 32];
        for (i, b) in k.iter_mut().enumerate() {
            *b = u8::try_from(i).unwrap();
        }
        k
    }

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    fn key_file(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join(KEY_FILE_NAME)
    }

    fn names(dir: &tempfile::TempDir) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn creates_0600_file_with_32_bytes_and_warns() {
        let dir = tempfile::tempdir().unwrap();
        let mut random = counter();
        let p = provision(dir.path(), &mut random).unwrap();
        assert_eq!(p.key, BindingKey::from_bytes(first_key()));
        let path = key_file(&dir);
        assert_eq!(fs::read(&path).unwrap(), first_key().to_vec());
        assert_eq!(mode(&path), 0o600);
        assert_eq!(p.warnings.len(), 1);
        assert!(p.warnings[0].starts_with("WARNING binding_key.file_fallback:"));
        assert!(p.warnings[0].contains("<app data>/binding.key with mode 0600"));
        // No temporary file is left behind.
        assert_eq!(names(&dir), vec![KEY_FILE_NAME.to_string()]);
    }

    #[test]
    fn existing_key_is_reused_unchanged_without_randomness() {
        let dir = tempfile::tempdir().unwrap();
        let mut random = counter();
        let first = provision(dir.path(), &mut random).unwrap();
        let mut never = no_random();
        let second = provision(dir.path(), &mut never).unwrap();
        assert_eq!(first.key, second.key);
        assert_eq!(second.warnings.len(), 1, "only the fallback warning");
        assert_eq!(fs::read(key_file(&dir)).unwrap(), first_key().to_vec());
    }

    #[test]
    fn permissive_file_is_replaced_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        let path = key_file(&dir);
        fs::write(&path, [9u8; 32]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let mut random = counter();
        let p = provision(dir.path(), &mut random).unwrap();
        assert_ne!(p.key, BindingKey::from_bytes([9u8; 32]));
        assert_eq!(p.key, BindingKey::from_bytes(first_key()));
        assert_eq!(mode(&path), 0o600);
        assert_eq!(fs::read(&path).unwrap(), first_key().to_vec());
        assert!(p
            .warnings
            .iter()
            .any(|w| w.starts_with("WARNING binding_key.file_rotated:")
                && w.contains("readable by other users")));
    }

    #[test]
    fn truncated_or_oversized_file_is_replaced_with_warning() {
        for len in [0usize, 31, 33, 64] {
            let dir = tempfile::tempdir().unwrap();
            let path = key_file(&dir);
            fs::write(&path, vec![5u8; len]).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
            let mut random = counter();
            let p = provision(dir.path(), &mut random).unwrap();
            assert_eq!(p.key, BindingKey::from_bytes(first_key()), "len {len}");
            assert_eq!(fs::read(&path).unwrap(), first_key().to_vec());
            assert!(p
                .warnings
                .iter()
                .any(|w| w.contains("did not hold a 32-byte key")));
        }
    }

    #[test]
    fn symlink_at_key_path_is_refused_and_not_followed() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("elsewhere");
        fs::write(&target, [1u8; 32]).unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
        std::os::unix::fs::symlink(&target, key_file(&dir)).unwrap();
        let mut random = counter();
        let err = provision(dir.path(), &mut random).unwrap_err();
        assert_eq!(err.category, ErrorCategory::DataCorruption);
        assert_eq!(err.code, "binding_key.file_not_regular");
        assert_eq!(
            fs::read(&target).unwrap(),
            vec![1u8; 32],
            "target untouched"
        );
        assert!(fs::symlink_metadata(key_file(&dir))
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[test]
    fn directory_at_key_path_is_refused_and_left_in_place() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(key_file(&dir)).unwrap();
        let mut random = counter();
        let err = provision(dir.path(), &mut random).unwrap_err();
        assert_eq!(err.category, ErrorCategory::DataCorruption);
        assert!(key_file(&dir).is_dir());
    }

    #[test]
    fn same_file_detects_a_swapped_inode() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        for p in [&a, &b] {
            fs::write(p, [2u8; 32]).unwrap();
            fs::set_permissions(p, fs::Permissions::from_mode(0o600)).unwrap();
        }
        let meta_a = fs::symlink_metadata(&a).unwrap();
        assert!(platform::same_file(
            &meta_a,
            &fs::File::open(&a).unwrap().metadata().unwrap()
        ));
        assert!(!platform::same_file(
            &meta_a,
            &fs::File::open(&b).unwrap().metadata().unwrap()
        ));
        // A swap that loosened the mode is refused even on the same inode.
        fs::set_permissions(&a, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!platform::same_file(
            &meta_a,
            &fs::File::open(&a).unwrap().metadata().unwrap()
        ));
    }

    #[test]
    fn missing_randomness_is_resource_unavailable_and_creates_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut never = no_random();
        let err = provision(dir.path(), &mut never).unwrap_err();
        assert_eq!(err.category, ErrorCategory::ResourceUnavailable);
        assert_eq!(err.code, "binding_key.os_random_unavailable");
        assert!(names(&dir).is_empty());
    }

    #[test]
    fn existing_path_is_never_replaced_by_create() {
        let dir = tempfile::tempdir().unwrap();
        let path = key_file(&dir);
        fs::write(&path, [6u8; 32]).unwrap();
        let mut random = counter();
        let mut warnings = Vec::new();
        let created = create_key_file(&path, &[7u8; 32], &mut random, &mut warnings).unwrap();
        assert!(!created);
        assert_eq!(fs::read(&path).unwrap(), vec![6u8; 32]);
        assert_eq!(names(&dir), vec![KEY_FILE_NAME.to_string()], "temp removed");
        assert!(warnings.is_empty());
    }

    #[test]
    fn foreign_file_at_temporary_path_is_not_deleted() {
        let dir = tempfile::tempdir().unwrap();
        // The counter stream yields nonce 00..07 for the first call.
        let tmp = dir
            .path()
            .join(format!(".{KEY_FILE_NAME}.0001020304050607.tmp"));
        fs::write(&tmp, b"not ours").unwrap();
        let mut random = counter();
        let mut warnings = Vec::new();
        let err =
            create_key_file(&key_file(&dir), &[7u8; 32], &mut random, &mut warnings).unwrap_err();
        assert_eq!(err.code, "binding_key.file_create_failed");
        assert_eq!(fs::read(&tmp).unwrap(), b"not ours".to_vec());
        assert!(!key_file(&dir).exists());
    }

    #[test]
    fn warnings_and_errors_carry_no_key_bytes_or_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = key_file(&dir);
        fs::write(&path, [0xabu8; 32]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let mut random = counter();
        let p = provision(dir.path(), &mut random).unwrap();
        let dir_text = dir.path().to_string_lossy().to_string();
        for w in &p.warnings {
            assert!(!w.contains(&dir_text), "{w}");
            assert!(!w.to_lowercase().contains("abab"), "{w}");
            assert!(!w.contains("0001020304"), "{w}");
        }
        assert_eq!(format!("{:?}", p.key), "BindingKey(<redacted>)");
        assert!(!format!("{p:?}").contains("171"));
        // Errors carry no host path either.
        fs::remove_file(&path).unwrap();
        fs::create_dir(&path).unwrap();
        let err = provision(dir.path(), &mut counter()).unwrap_err();
        let text = serde_json::to_string(&err).unwrap();
        assert!(!text.contains(&dir_text), "{text}");
    }

    #[test]
    fn os_random_source_is_available_on_this_host() {
        // Only the outcome is asserted (deterministic); the bytes are not inspected.
        let mut buf = [0u8; 32];
        assert!(os_random(&mut buf).is_ok());
        assert!(os_random(&mut []).is_ok());
    }
}
