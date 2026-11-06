//! Approval binding key provisioning (ARCHITECTURE.md §11; ADR-0003;
//! `docs/privacy/data-lifecycle.md`, row "Approval binding key").
//!
//! The binding key is the per-installation HMAC secret that `g9-policy` uses to
//! bind an approval to the exact operation, resource and expected revision. It
//! lives only in the backend process.
//!
//! # Order of sources
//!
//! 1. **OS keyring** — the Secret Service on Linux, through the `keyring` crate
//!    (service [`KEYRING_SERVICE`], account [`KEYRING_ACCOUNT`]). A new key is
//!    written and read back before it is used. When the keyring holds the key,
//!    a fallback file left by an earlier session is deleted (it holds an unused
//!    key); a valid fallback key found while the keyring is still empty is moved
//!    into the keyring so that approvals already granted stay verifiable.
//! 2. **Fallback file** — `<app data>/binding.key`: exactly 32 bytes from the OS
//!    CSPRNG (`getrandom`), created atomically with mode `0600` (temporary file +
//!    `hard_link`, so a second process can never observe a partial key). Using the
//!    fallback adds the warning [`WARN_FILE_FALLBACK`], which the bridge reports in
//!    `system.status` (`CommandOutcome.warnings`).
//!
//! # Integrity rules
//!
//! * Key bytes are never logged, serialized, displayed or placed in an error or
//!   warning. Keyring errors are reduced to the fieldless [`StoreFailure`]
//!   because `keyring` errors can carry secret bytes (`BadEncoding`) or host
//!   paths (`PlatformFailure`).
//! * A key file that is group/other accessible or has the wrong length is
//!   replaced (warning [`WARN_FILE_ROTATED`]); approvals bound to the old key then
//!   fail verification, which is the fail-closed outcome. A path that is not a
//!   regular file (symlink, directory) is refused with `DATA_CORRUPTION` and is
//!   never followed or deleted.
//! * Randomness is injected (`random`), so tests are deterministic; production
//!   passes [`os_random`].
//!
//! The file fallback needs Unix permissions; on other platforms it is refused
//! (`PRECONDITION_FAILED binding_key.file_fallback_unsupported`) and only the OS
//! keyring is used.

use g9_domain::G9Error;
use g9_policy::{BindingKey, BINDING_KEY_LEN};
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

/// File name of the fallback key inside the app data directory.
pub const KEY_FILE_NAME: &str = "binding.key";
/// Keyring service name (the Tauri bundle identifier).
pub const KEYRING_SERVICE: &str = "com.g9.mentor";
/// Keyring account name.
pub const KEYRING_ACCOUNT: &str = "approval-binding-key";

/// Warning code: the key is kept in the `0600` fallback file.
pub const WARN_FILE_FALLBACK: &str = "binding_key.file_fallback";
/// Warning code: an unusable key file was replaced by a new key.
pub const WARN_FILE_ROTATED: &str = "binding_key.file_rotated";
/// Warning code: an unusable keyring entry was replaced by a new key.
pub const WARN_KEYRING_ROTATED: &str = "binding_key.keyring_rotated";
/// Warning code: a stale fallback file could not be removed.
pub const WARN_STALE_FILE: &str = "binding_key.stale_file_not_removed";

/// Format a warning string as reported in `CommandOutcome.warnings`.
pub fn warning(code: &str, message: &str) -> String {
    format!("WARNING {code}: {message}")
}

/// Where the binding key of this process came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeySource {
    /// The OS keyring (Secret Service).
    OsKeyring,
    /// The `0600` fallback file in the app data directory.
    File,
}

impl KeySource {
    /// Stable spelling for logs and the README.
    pub const fn as_str(self) -> &'static str {
        match self {
            KeySource::OsKeyring => "OS_KEYRING",
            KeySource::File => "FILE",
        }
    }
}

/// Why the OS keyring could not be used. Fieldless on purpose (see module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreFailure {
    /// No Secret Service answered (no session bus, no provider).
    Unavailable,
    /// The provider refused access (locked collection, dismissed prompt).
    AccessDenied,
    /// More than one matching keyring item exists.
    Ambiguous,
    /// The key could not be stored and read back unchanged.
    WriteNotVerified,
    /// Any other provider failure.
    Failed,
}

impl StoreFailure {
    /// Machine-readable reason (used inside warnings).
    pub const fn code(self) -> &'static str {
        match self {
            StoreFailure::Unavailable => "keyring_unavailable",
            StoreFailure::AccessDenied => "keyring_access_denied",
            StoreFailure::Ambiguous => "keyring_ambiguous",
            StoreFailure::WriteNotVerified => "keyring_write_not_verified",
            StoreFailure::Failed => "keyring_failed",
        }
    }
}

/// A store that can hold the binding key: the OS keyring in production, an
/// in-memory store in tests.
pub trait SecretStore {
    /// Read the stored secret; `Ok(None)` when no entry exists.
    fn read(&self) -> Result<Option<Vec<u8>>, StoreFailure>;
    /// Create or replace the stored secret.
    fn write(&self, secret: &[u8]) -> Result<(), StoreFailure>;
}

/// The OS keyring entry for the binding key (`keyring` 4.2, `v1` API).
pub struct OsKeyring {
    entry: os_keyring::Entry,
}

impl fmt::Debug for OsKeyring {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OsKeyring")
            .field("service", &KEYRING_SERVICE)
            .field("account", &KEYRING_ACCOUNT)
            .finish_non_exhaustive()
    }
}

impl OsKeyring {
    /// Connect to the platform credential store. Fails with
    /// [`StoreFailure::Unavailable`] when no Secret Service answers.
    pub fn connect() -> Result<Self, StoreFailure> {
        os_keyring::Entry::new(KEYRING_SERVICE, KEYRING_ACCOUNT)
            .map(|entry| Self { entry })
            .map_err(|e| classify(&e))
    }
}

/// Reduce a keyring error to a secret-free reason.
fn classify(error: &os_keyring::Error) -> StoreFailure {
    match error {
        os_keyring::Error::NoDefaultStore | os_keyring::Error::PlatformFailure(_) => {
            StoreFailure::Unavailable
        }
        os_keyring::Error::NoStorageAccess(_) => StoreFailure::AccessDenied,
        os_keyring::Error::Ambiguous(_) => StoreFailure::Ambiguous,
        _ => StoreFailure::Failed,
    }
}

impl SecretStore for OsKeyring {
    fn read(&self) -> Result<Option<Vec<u8>>, StoreFailure> {
        match self.entry.get_secret() {
            Ok(secret) => Ok(Some(secret)),
            Err(os_keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(classify(&e)),
        }
    }

    fn write(&self, secret: &[u8]) -> Result<(), StoreFailure> {
        self.entry.set_secret(secret).map_err(|e| classify(&e))
    }
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

/// The provisioned key and how it was obtained. `BindingKey`'s `Debug` output
/// is redacted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionedKey {
    /// The per-installation HMAC key.
    pub key: BindingKey,
    /// Where it came from.
    pub source: KeySource,
    /// Warnings for `system.status` (secret-free, no host paths).
    pub warnings: Vec<String>,
}

/// Provision the binding key: OS keyring first, then the `0600` file in
/// `data_dir` (which must already exist). See the module docs for the rules.
pub fn provision(
    store: Result<&dyn SecretStore, StoreFailure>,
    data_dir: &Path,
    random: &mut dyn FnMut(&mut [u8]) -> Result<(), G9Error>,
) -> Result<ProvisionedKey, G9Error> {
    let file = data_dir.join(KEY_FILE_NAME);
    let mut warnings = Vec::new();
    let failure = match store {
        Ok(store) => match from_store(store, &file, random, &mut warnings)? {
            Ok(key) => {
                return Ok(ProvisionedKey {
                    key,
                    source: KeySource::OsKeyring,
                    warnings,
                })
            }
            Err(failure) => failure,
        },
        Err(failure) => failure,
    };
    let key = from_file(&file, random, &mut warnings)?;
    warnings.insert(
        0,
        warning(
            WARN_FILE_FALLBACK,
            &format!(
                "the OS keyring is not usable ({}); the approval binding key is kept in \
                 <app data>/{KEY_FILE_NAME} with mode 0600",
                failure.code()
            ),
        ),
    );
    Ok(ProvisionedKey {
        key,
        source: KeySource::File,
        warnings,
    })
}

/// Keyring path. The outer `Result` carries hard errors (no randomness); the
/// inner one says whether the keyring produced the key.
fn from_store(
    store: &dyn SecretStore,
    file: &Path,
    random: &mut dyn FnMut(&mut [u8]) -> Result<(), G9Error>,
    warnings: &mut Vec<String>,
) -> Result<Result<BindingKey, StoreFailure>, G9Error> {
    let stored = match store.read() {
        Ok(stored) => stored,
        Err(failure) => return Ok(Err(failure)),
    };
    let key = match stored {
        Some(bytes) => match BindingKey::try_from_slice(&bytes) {
            Ok(key) => key,
            Err(_) => {
                let fresh = fresh_key(random)?;
                match store_and_verify(store, &fresh) {
                    Ok(key) => {
                        warnings.push(warning(
                            WARN_KEYRING_ROTATED,
                            "the keyring entry did not hold a valid key and was replaced; \
                             approvals granted earlier must be requested again",
                        ));
                        key
                    }
                    Err(failure) => return Ok(Err(failure)),
                }
            }
        },
        None => {
            // Move a valid fallback key into the keyring (approvals stay
            // verifiable); otherwise start with a fresh key.
            let bytes = match read_key_file(file) {
                Ok(Some(bytes)) => bytes,
                _ => fresh_key(random)?,
            };
            match store_and_verify(store, &bytes) {
                Ok(key) => key,
                Err(failure) => return Ok(Err(failure)),
            }
        }
    };
    remove_stale_file(file, warnings);
    Ok(Ok(key))
}

fn store_and_verify(
    store: &dyn SecretStore,
    bytes: &[u8; BINDING_KEY_LEN],
) -> Result<BindingKey, StoreFailure> {
    store.write(bytes)?;
    match store.read()? {
        Some(back) if back.as_slice() == bytes.as_slice() => Ok(BindingKey::from_bytes(*bytes)),
        _ => Err(StoreFailure::WriteNotVerified),
    }
}

/// The keyring holds the key: a fallback file (or a link at its path) only
/// holds an unused key and is removed.
fn remove_stale_file(file: &Path, warnings: &mut Vec<String>) {
    match fs::symlink_metadata(file) {
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Ok(meta) if !meta.file_type().is_dir() => {
            if fs::remove_file(file).is_err() {
                warnings.push(warning(
                    WARN_STALE_FILE,
                    "an unused fallback key file could not be removed from the app data directory",
                ));
            }
        }
        _ => warnings.push(warning(
            WARN_STALE_FILE,
            "the fallback key path is not a regular file and was left in place",
        )),
    }
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

/// Read the fallback file; `Ok(None)` when it does not exist.
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
    let mut bytes = Vec::with_capacity(BINDING_KEY_LEN + 1);
    // Read at most one byte past the key length to detect oversize files.
    file.take(BINDING_KEY_LEN as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| KeyFileProblem::Unreadable)?;
    <[u8; BINDING_KEY_LEN]>::try_from(bytes.as_slice())
        .map(Some)
        .map_err(|_| KeyFileProblem::InvalidLength)
}

/// File path: reuse a valid key file, replace an unusable one, or create one.
fn from_file(
    file: &Path,
    random: &mut dyn FnMut(&mut [u8]) -> Result<(), G9Error>,
    warnings: &mut Vec<String>,
) -> Result<BindingKey, G9Error> {
    if !platform::FILE_FALLBACK_SUPPORTED {
        return Err(G9Error::precondition_failed(
            "binding_key.file_fallback_unsupported",
            "the OS keyring is not usable and the file fallback needs Unix permissions",
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
    platform::write_private_file(&tmp, bytes).map_err(|_| {
        // A partially written temporary file must not linger.
        let _ = fs::remove_file(&tmp);
        create_failed()
    })?;
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
    use std::fs::{self, OpenOptions};
    use std::io::{self, Write};
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    use std::path::Path;

    pub(super) const FILE_FALLBACK_SUPPORTED: bool = true;

    /// No group/other permission bits.
    pub(super) fn is_private(meta: &fs::Metadata) -> bool {
        meta.permissions().mode() & 0o077 == 0
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
        if let Ok(handle) = fs::File::open(dir) {
            let _ = handle.sync_all();
        }
    }
}

#[cfg(not(unix))]
mod platform {
    use std::fs;
    use std::io;
    use std::path::Path;

    pub(super) const FILE_FALLBACK_SUPPORTED: bool = false;

    pub(super) fn is_private(_meta: &fs::Metadata) -> bool {
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
    use std::cell::{Cell, RefCell};
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

    /// In-memory keyring double.
    #[derive(Default)]
    struct MemoryStore {
        secret: RefCell<Option<Vec<u8>>>,
        read_fails: Option<StoreFailure>,
        write_fails: Option<StoreFailure>,
        corrupt_on_write: bool,
        writes: Cell<u32>,
    }

    impl SecretStore for MemoryStore {
        fn read(&self) -> Result<Option<Vec<u8>>, StoreFailure> {
            match self.read_fails {
                Some(f) => Err(f),
                None => Ok(self.secret.borrow().clone()),
            }
        }
        fn write(&self, secret: &[u8]) -> Result<(), StoreFailure> {
            if let Some(f) = self.write_fails {
                return Err(f);
            }
            self.writes.set(self.writes.get() + 1);
            let mut stored = secret.to_vec();
            if self.corrupt_on_write {
                stored[0] ^= 0xff;
            }
            *self.secret.borrow_mut() = Some(stored);
            Ok(())
        }
    }

    fn mode(path: &Path) -> u32 {
        fs::metadata(path).unwrap().permissions().mode() & 0o777
    }

    fn key_file(dir: &tempfile::TempDir) -> std::path::PathBuf {
        dir.path().join(KEY_FILE_NAME)
    }

    #[test]
    fn file_fallback_creates_0600_file_with_32_bytes_and_warns() {
        let dir = tempfile::tempdir().unwrap();
        let mut random = counter();
        let p = provision(Err(StoreFailure::Unavailable), dir.path(), &mut random).unwrap();
        assert_eq!(p.source, KeySource::File);
        assert_eq!(p.key, BindingKey::from_bytes(first_key()));
        let path = key_file(&dir);
        assert_eq!(fs::read(&path).unwrap(), first_key().to_vec());
        assert_eq!(mode(&path), 0o600);
        assert_eq!(p.warnings.len(), 1);
        assert!(p.warnings[0].starts_with("WARNING binding_key.file_fallback:"));
        assert!(p.warnings[0].contains("keyring_unavailable"));
        // No temporary file is left behind.
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, vec![KEY_FILE_NAME.to_string()]);
    }

    #[test]
    fn existing_file_key_is_reused_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let mut random = counter();
        let first = provision(Err(StoreFailure::Unavailable), dir.path(), &mut random).unwrap();
        let mut never = no_random();
        let second = provision(Err(StoreFailure::Unavailable), dir.path(), &mut never).unwrap();
        assert_eq!(first.key, second.key);
        assert_eq!(second.source, KeySource::File);
        assert_eq!(second.warnings.len(), 1, "only the fallback warning");
    }

    #[test]
    fn permissive_file_is_replaced_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        let path = key_file(&dir);
        fs::write(&path, [9u8; 32]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let mut random = counter();
        let p = provision(Err(StoreFailure::Unavailable), dir.path(), &mut random).unwrap();
        assert_ne!(p.key, BindingKey::from_bytes([9u8; 32]));
        assert_eq!(p.key, BindingKey::from_bytes(first_key()));
        assert_eq!(mode(&path), 0o600);
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
            let p = provision(Err(StoreFailure::Unavailable), dir.path(), &mut random).unwrap();
            assert_eq!(p.key, BindingKey::from_bytes(first_key()), "len {len}");
            assert_eq!(fs::read(&path).unwrap().len(), 32);
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
        let err = provision(Err(StoreFailure::Unavailable), dir.path(), &mut random).unwrap_err();
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
    fn keyring_key_is_preferred_and_stale_file_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = key_file(&dir);
        fs::write(&path, [3u8; 32]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let store = MemoryStore {
            secret: RefCell::new(Some(vec![8u8; 32])),
            ..MemoryStore::default()
        };
        let mut never = no_random();
        let p = provision(Ok(&store), dir.path(), &mut never).unwrap();
        assert_eq!(p.source, KeySource::OsKeyring);
        assert_eq!(p.key, BindingKey::from_bytes([8u8; 32]));
        assert!(p.warnings.is_empty());
        assert!(!path.exists(), "unused fallback key removed");
        assert_eq!(store.writes.get(), 0, "existing entry not rewritten");
    }

    #[test]
    fn valid_file_key_moves_into_empty_keyring() {
        let dir = tempfile::tempdir().unwrap();
        let path = key_file(&dir);
        fs::write(&path, [4u8; 32]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let store = MemoryStore::default();
        let mut never = no_random();
        let p = provision(Ok(&store), dir.path(), &mut never).unwrap();
        assert_eq!(p.source, KeySource::OsKeyring);
        assert_eq!(p.key, BindingKey::from_bytes([4u8; 32]));
        assert_eq!(store.secret.borrow().as_deref(), Some(&[4u8; 32][..]));
        assert!(!path.exists());
        assert!(p.warnings.is_empty());
    }

    #[test]
    fn empty_keyring_gets_fresh_key_without_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore::default();
        let mut random = counter();
        let p = provision(Ok(&store), dir.path(), &mut random).unwrap();
        assert_eq!(p.source, KeySource::OsKeyring);
        assert_eq!(p.key, BindingKey::from_bytes(first_key()));
        assert_eq!(store.writes.get(), 1);
        assert!(!key_file(&dir).exists());
        assert!(p.warnings.is_empty());
    }

    #[test]
    fn invalid_keyring_entry_is_replaced_with_warning() {
        let dir = tempfile::tempdir().unwrap();
        let store = MemoryStore {
            secret: RefCell::new(Some(vec![1u8; 7])),
            ..MemoryStore::default()
        };
        let mut random = counter();
        let p = provision(Ok(&store), dir.path(), &mut random).unwrap();
        assert_eq!(p.source, KeySource::OsKeyring);
        assert_eq!(p.key, BindingKey::from_bytes(first_key()));
        assert!(p.warnings[0].starts_with("WARNING binding_key.keyring_rotated:"));
    }

    #[test]
    fn keyring_failures_fall_back_to_file_with_reason() {
        let cases = [
            (
                MemoryStore {
                    read_fails: Some(StoreFailure::AccessDenied),
                    ..MemoryStore::default()
                },
                "keyring_access_denied",
            ),
            (
                MemoryStore {
                    write_fails: Some(StoreFailure::Failed),
                    ..MemoryStore::default()
                },
                "keyring_failed",
            ),
            (
                MemoryStore {
                    corrupt_on_write: true,
                    ..MemoryStore::default()
                },
                "keyring_write_not_verified",
            ),
        ];
        for (store, reason) in cases {
            let dir = tempfile::tempdir().unwrap();
            let mut random = counter();
            let p = provision(Ok(&store), dir.path(), &mut random).unwrap();
            assert_eq!(p.source, KeySource::File, "{reason}");
            assert!(p.warnings[0].contains(reason), "{reason}: {:?}", p.warnings);
            assert_eq!(mode(&key_file(&dir)), 0o600);
        }
    }

    #[test]
    fn missing_randomness_is_resource_unavailable_and_creates_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut never = no_random();
        let err = provision(Err(StoreFailure::Unavailable), dir.path(), &mut never).unwrap_err();
        assert_eq!(err.category, ErrorCategory::ResourceUnavailable);
        assert_eq!(err.code, "binding_key.os_random_unavailable");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
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
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1, "temp removed");
    }

    #[test]
    fn warnings_and_errors_carry_no_key_bytes_or_paths() {
        let dir = tempfile::tempdir().unwrap();
        let path = key_file(&dir);
        fs::write(&path, [0xabu8; 32]).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640)).unwrap();
        let mut random = counter();
        let p = provision(Err(StoreFailure::Unavailable), dir.path(), &mut random).unwrap();
        let dir_text = dir.path().to_string_lossy().to_string();
        for w in &p.warnings {
            assert!(!w.contains(&dir_text), "{w}");
            assert!(!w.to_lowercase().contains("abab"), "{w}");
            assert!(!w.contains("0001020304"), "{w}");
        }
        assert_eq!(format!("{:?}", p.key), "BindingKey(<redacted>)");
        assert!(!format!("{p:?}").contains("171"));
    }

    #[test]
    fn os_random_source_is_available_on_this_host() {
        // Only the outcome is asserted (deterministic); the bytes are not inspected.
        let mut buf = [0u8; 32];
        assert!(os_random(&mut buf).is_ok());
        assert!(os_random(&mut []).is_ok());
    }
}
