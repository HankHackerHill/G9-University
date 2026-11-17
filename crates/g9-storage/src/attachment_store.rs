//! Content-addressed attachment store on disk (M05; table `attachments`).
//!
//! Bytes live at `<root>/<ab>/<cd>/<sha256>`: the path is derived from the hash
//! of the bytes, never from a caller-supplied name, so a file can only be
//! written where its content says. The same layout is used for the live store
//! (`AppConfig::attachment_dir`), the copy kept beside the backups
//! (`<backups>/attachments`) and the copy an export carries.
//!
//! Every write is durable and never exposes a partial file: the bytes go to a
//! hidden `.<sha256>.partial` file opened `create_new`, which is flushed,
//! renamed into place, and the directory is flushed (the backup snapshot
//! sequence). An existing file with the right hash is kept (identical content
//! is stored once); an existing file whose bytes do not match its name is
//! replaced. Anything that is not a regular file at a store path (a symlink, a
//! directory) is refused, and every directory is canonicalized and checked to
//! lie under the canonical root, so a symlinked shard directory cannot move a
//! write outside the store (`g9_evidence::attachments` module docs).

use crate::backup::{sha256_of_file, sync_dir};
use crate::error::{StorageError, StorageResult};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

/// Outcome of [`put`] and [`copy_between`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stored {
    /// The bytes were written.
    Written,
    /// A file with the same hash was already in place; nothing was written.
    AlreadyPresent,
}

/// Whether `s` is a lowercase hex SHA-256 digest.
pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The store-relative path of a digest: `ab/cd/<sha256>`.
pub fn relpath(sha256: &str) -> StorageResult<String> {
    if !is_sha256_hex(sha256) {
        return Err(StorageError::InvalidInput(
            "attachment digest must be 64 lowercase hex characters".to_string(),
        ));
    }
    Ok(format!("{}/{}/{}", &sha256[0..2], &sha256[2..4], sha256))
}

/// Check a stored `storage_ref` against its digest: it must be exactly the
/// content-addressed path of `sha256` (no other relative path is accepted).
pub fn check_storage_ref(storage_ref: &str, sha256: &str) -> StorageResult<()> {
    if relpath(sha256)? != storage_ref {
        return Err(StorageError::Corrupt(
            "attachment storage_ref does not match its content hash".to_string(),
        ));
    }
    Ok(())
}

fn canonical_root(root: &Path) -> StorageResult<PathBuf> {
    let canonical =
        fs::canonicalize(root).map_err(|e| StorageError::io("open the attachment store", &e))?;
    if !canonical.is_dir() {
        return Err(StorageError::Precondition(
            "the attachment store is not a directory".to_string(),
        ));
    }
    Ok(canonical)
}

/// Create (if needed) and canonicalize the shard directory of `sha256`,
/// refusing one that resolves outside `root` (which must be canonical). Each
/// level is inspected without following links before it is created, so a
/// symlink planted at `ab` is refused before anything is created beneath it.
fn shard_dir(root: &Path, sha256: &str) -> StorageResult<PathBuf> {
    let mut dir = root.to_path_buf();
    for part in [&sha256[0..2], &sha256[2..4]] {
        dir.push(part);
        let is_dir = |dir: &Path| match fs::symlink_metadata(dir) {
            Ok(meta) if meta.file_type().is_dir() => Ok(true),
            Ok(_) => Err(StorageError::Conflict(
                "a non-directory occupies an attachment directory path".to_string(),
            )),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(StorageError::io("inspect an attachment directory", &e)),
        };
        if is_dir(&dir)? {
            continue;
        }
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        match builder.create(&dir) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && is_dir(&dir)? => {}
            Err(e) => return Err(StorageError::io("create an attachment directory", &e)),
        }
    }
    let canonical =
        fs::canonicalize(&dir).map_err(|e| StorageError::io("open an attachment directory", &e))?;
    if !canonical.starts_with(root) {
        return Err(StorageError::Conflict(
            "an attachment directory resolves outside the store".to_string(),
        ));
    }
    Ok(canonical)
}

/// Whether `path` holds a regular file whose bytes hash to `sha256`.
/// `Ok(false)` when absent or mismatching; a non-file is a conflict.
fn present_and_intact(path: &Path, sha256: &str) -> StorageResult<bool> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_file() => Ok(sha256_of_file(path)? == sha256),
        Ok(_) => Err(StorageError::Conflict(
            "a non-file occupies an attachment path".to_string(),
        )),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(StorageError::io("inspect an attachment", &e)),
    }
}

fn open_partial(dir: &Path, sha256: &str) -> StorageResult<(PathBuf, fs::File)> {
    let partial = dir.join(format!(".{sha256}.partial"));
    match fs::symlink_metadata(&partial) {
        Ok(meta) if meta.file_type().is_file() => fs::remove_file(&partial)
            .map_err(|e| StorageError::io("remove a stale partial attachment", &e))?,
        Ok(_) => {
            return Err(StorageError::Conflict(
                "a non-file occupies an attachment partial path".to_string(),
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(StorageError::io("inspect a partial attachment", &e)),
    }
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(&partial)
        .map_err(|e| StorageError::io("create an attachment file", &e))?;
    Ok((partial, file))
}

fn publish(partial: &Path, file: fs::File, target: &Path, dir: &Path) -> StorageResult<()> {
    file.sync_all()
        .map_err(|e| StorageError::io("flush an attachment file", &e))?;
    drop(file);
    fs::rename(partial, target).map_err(|e| StorageError::io("place an attachment file", &e))?;
    sync_dir(dir)
}

/// Store `bytes` under `root` at the content address of their SHA-256, which
/// must equal `sha256` (the caller's digest is re-checked).
pub fn put(root: &Path, sha256: &str, bytes: &[u8]) -> StorageResult<Stored> {
    relpath(sha256)?;
    if hex::encode(Sha256::digest(bytes)) != sha256 {
        return Err(StorageError::InvalidInput(
            "attachment bytes do not match the digest".to_string(),
        ));
    }
    let root = canonical_root(root)?;
    let dir = shard_dir(&root, sha256)?;
    let target = dir.join(sha256);
    if present_and_intact(&target, sha256)? {
        return Ok(Stored::AlreadyPresent);
    }
    let (partial, mut file) = open_partial(&dir, sha256)?;
    let written = file
        .write_all(bytes)
        .map_err(|e| StorageError::io("write an attachment file", &e));
    if let Err(e) = written {
        let _ = fs::remove_file(&partial);
        return Err(e);
    }
    publish(&partial, file, &target, &dir)
        .inspect_err(|_| {
            let _ = fs::remove_file(&partial);
        })
        .map(|()| Stored::Written)
}

/// Path of `sha256` under `root` after the same checks as a write (canonical
/// root, shard directory inside it, a regular file). The file must exist.
fn existing_file(root: &Path, sha256: &str) -> StorageResult<PathBuf> {
    let rel = relpath(sha256)?;
    let root = canonical_root(root)?;
    let path = root.join(&rel);
    let meta = fs::symlink_metadata(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            StorageError::NotFound("attachment file")
        } else {
            StorageError::io("inspect an attachment", &e)
        }
    })?;
    if !meta.file_type().is_file() {
        return Err(StorageError::Conflict(
            "a non-file occupies an attachment path".to_string(),
        ));
    }
    let canonical =
        fs::canonicalize(&path).map_err(|e| StorageError::io("open an attachment", &e))?;
    if !canonical.starts_with(&root) {
        return Err(StorageError::Conflict(
            "an attachment path resolves outside the store".to_string(),
        ));
    }
    Ok(canonical)
}

/// Open `path` for reading only if it is a regular file, checked on the open
/// handle: a FIFO or device swapped in after the path was inspected is refused
/// instead of blocking the read (`O_NONBLOCK`, which a regular file ignores),
/// and a final symlink is not followed (`O_NOFOLLOW`).
fn open_regular(path: &Path) -> StorageResult<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        let flags = rustix::fs::OFlags::NOFOLLOW | rustix::fs::OFlags::NONBLOCK;
        options.custom_flags(i32::try_from(flags.bits()).unwrap_or(0));
    }
    let file = options
        .open(path)
        .map_err(|e| StorageError::io("open an attachment", &e))?;
    let meta = file
        .metadata()
        .map_err(|e| StorageError::io("inspect an attachment", &e))?;
    if !meta.is_file() {
        return Err(StorageError::Conflict(
            "a non-file occupies an attachment path".to_string(),
        ));
    }
    Ok(file)
}

/// Copy the bytes of `sha256` from the store at `from` to the store at `to`,
/// hashing while copying: a source whose bytes do not match is
/// [`StorageError::Corrupt`] and nothing is placed at the destination.
pub fn copy_between(from: &Path, to: &Path, sha256: &str) -> StorageResult<Stored> {
    relpath(sha256)?;
    // The destination first: an intact file there needs no source. Nothing is
    // created at the destination until the source is known to exist (a hostile
    // snapshot listing many digests must not leave empty directories behind).
    let to_root = canonical_root(to)?;
    match existing_file(&to_root, sha256) {
        Ok(path) if sha256_of_file(&path)? == sha256 => return Ok(Stored::AlreadyPresent),
        Ok(_) | Err(StorageError::NotFound(_)) => {}
        Err(e) => return Err(e),
    }
    let source = existing_file(from, sha256)?;
    let mut input = open_regular(&source)?;
    let dir = shard_dir(&to_root, sha256)?;
    let target = dir.join(sha256);
    if present_and_intact(&target, sha256)? {
        return Ok(Stored::AlreadyPresent);
    }
    let (partial, mut output) = open_partial(&dir, sha256)?;
    let copied = (|| {
        let mut hasher = Sha256::new();
        let mut buf = vec![0_u8; 64 * 1024];
        loop {
            let n = input
                .read(&mut buf)
                .map_err(|e| StorageError::io("read an attachment", &e))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
            output
                .write_all(&buf[..n])
                .map_err(|e| StorageError::io("write an attachment copy", &e))?;
        }
        if hex::encode(hasher.finalize()) != sha256 {
            return Err(StorageError::Corrupt(
                "an attachment's bytes do not match its content hash".to_string(),
            ));
        }
        Ok(())
    })();
    if let Err(e) = copied {
        drop(output);
        let _ = fs::remove_file(&partial);
        return Err(e);
    }
    publish(&partial, output, &target, &dir)
        .inspect_err(|_| {
            let _ = fs::remove_file(&partial);
        })
        .map(|()| Stored::Written)
}

/// Whether the store at `root` holds an intact copy of `sha256` (the bytes are
/// hashed). A missing file or a missing store is `Ok(false)`.
pub fn holds_intact(root: &Path, sha256: &str) -> StorageResult<bool> {
    match existing_file(root, sha256) {
        Ok(path) => Ok(sha256_of_file(&path)? == sha256),
        Err(StorageError::NotFound(_)) => Ok(false),
        Err(StorageError::Io {
            kind: std::io::ErrorKind::NotFound,
            ..
        }) => Ok(false),
        Err(e) => Err(e),
    }
}

/// What verification found at a digest's content address ([`inspect`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    /// A regular file inside the store whose bytes hash to the digest.
    Intact,
    /// Nothing there, the store itself is absent, the digest is not a
    /// lowercase hex SHA-256, or the file could not be read.
    Missing,
    /// Something is there but it is not a copy a restore can use: bytes that
    /// hash to another digest, or not a regular file inside the store (a
    /// link, a directory, a shard directory that resolves outside the store;
    /// [`copy_between`] refuses each of these as a source).
    Damaged,
}

/// Hash the file at the content address of `sha256` under `root` and say
/// whether a restore could copy it from there. Never writes; a read failure is
/// [`FileState::Missing`] (the file cannot be vouched for), never an error, so
/// one unreadable file does not hide the state of the others.
pub fn inspect(root: &Path, sha256: &str) -> FileState {
    let Ok(rel) = relpath(sha256) else {
        return FileState::Missing;
    };
    let Ok(root) = fs::canonicalize(root) else {
        return FileState::Missing;
    };
    let path = root.join(rel);
    match fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_file() => {}
        Ok(_) => return FileState::Damaged,
        Err(_) => return FileState::Missing,
    }
    match fs::canonicalize(&path) {
        Ok(canonical) if canonical.starts_with(&root) => {}
        Ok(_) => return FileState::Damaged,
        Err(_) => return FileState::Missing,
    }
    match sha256_of_file(&path) {
        Ok(actual) if actual == sha256 => FileState::Intact,
        Ok(_) => FileState::Damaged,
        Err(_) => FileState::Missing,
    }
}

/// What [`copy_all`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CopyReport {
    /// Files written at the destination.
    pub written: usize,
    /// Files already intact at the destination.
    pub already_present: usize,
    /// Digests with no file in the source store.
    pub missing: Vec<String>,
    /// Digests whose source bytes do not match (nothing was placed).
    pub corrupt: Vec<String>,
}

impl CopyReport {
    /// Whether every digest is now intact at the destination.
    pub fn complete(&self) -> bool {
        self.missing.is_empty() && self.corrupt.is_empty()
    }
}

/// Copy every digest from the store at `from` to the store at `to` (created
/// when absent, mode 0700). A digest that is missing or damaged at the source
/// is reported, not fatal; a failure to write the destination is an error.
pub fn copy_all(from: &Path, to: &Path, digests: &[String]) -> StorageResult<CopyReport> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(to)
        .map_err(|e| StorageError::io("create an attachment store", &e))?;
    let mut report = CopyReport::default();
    for sha in digests {
        match copy_between(from, to, sha) {
            Ok(Stored::Written) => report.written += 1,
            Ok(Stored::AlreadyPresent) => report.already_present += 1,
            Err(StorageError::NotFound(_)) => report.missing.push(sha.clone()),
            Err(StorageError::Io {
                kind: std::io::ErrorKind::NotFound,
                ..
            }) => report.missing.push(sha.clone()),
            Err(StorageError::Corrupt(_)) => report.corrupt.push(sha.clone()),
            Err(e) => return Err(e),
        }
    }
    Ok(report)
}

/// Whether the database has the `attachments` table: a schema older than
/// migration `0005` has none, and so references no file (a pre-migration
/// backup of such a database is verified before `0005` is applied; REL-5).
pub fn has_attachment_table(conn: &rusqlite::Connection) -> StorageResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'attachments')",
        [],
        |r| r.get::<_, bool>(0),
    )?)
}

/// Distinct content digests of every non-erased attachment row, sorted (none
/// before migration `0005`).
pub fn referenced_digests(conn: &rusqlite::Connection) -> StorageResult<Vec<String>> {
    if !has_attachment_table(conn)? {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT DISTINCT content_sha256 FROM attachments WHERE erased = 0 ORDER BY content_sha256",
    )?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
    let mut out = Vec::new();
    for row in rows {
        let sha = row?;
        if is_sha256_hex(&sha) {
            out.push(sha);
        }
    }
    Ok(out)
}

/// Remove the stored file at the store-relative path `storage_ref` (the path an
/// attachment row recorded: `<2 hex>/<2 hex>/<64 hex>`). Returns whether a file
/// was removed; a file that is already gone is not an error.
///
/// This is how the bytes of an erased attachment leave the store. It removes
/// exactly the file it is given: the store can hold files the open database does
/// not reference (after a restore, a quarantined database still does), so
/// nothing here walks the tree or infers what is unused. A path that is not the
/// content-addressed shape, a path through a symlink and anything that is not a
/// regular file are refused.
pub fn remove(root: &Path, storage_ref: &str) -> StorageResult<bool> {
    let name = Path::new(storage_ref)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default()
        .to_string();
    if !is_sha256_hex(&name) || relpath(&name)? != storage_ref {
        return Err(StorageError::Precondition(
            "the stored path of an attachment is not content-addressed".to_string(),
        ));
    }
    match fs::symlink_metadata(root) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(StorageError::io("inspect the attachment store", &e)),
        Ok(_) => {}
    }
    let root = canonical_root(root)?;
    let dir = root.join(&name[0..2]).join(&name[2..4]);
    // Each component must be a real directory of the store, never a link out of it.
    for part in [root.join(&name[0..2]), dir.clone()] {
        match fs::symlink_metadata(&part) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(StorageError::io("inspect the attachment store", &e)),
            Ok(meta) if !meta.file_type().is_dir() => {
                return Err(StorageError::Precondition(
                    "the attachment store holds something that is not a folder where a folder is expected"
                        .to_string(),
                ))
            }
            Ok(_) => {}
        }
    }
    let file = dir.join(&name);
    match fs::symlink_metadata(&file) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(StorageError::io("inspect an attachment", &e)),
        Ok(meta) if !meta.file_type().is_file() => Err(StorageError::Precondition(
            "the stored attachment is not a regular file".to_string(),
        )),
        Ok(_) => {
            fs::remove_file(&file).map_err(|e| StorageError::io("remove an attachment", &e))?;
            sync_dir(&dir)?;
            Ok(true)
        }
    }
}

/// [`referenced_digests`] of a database file opened read-only (a snapshot or
/// a stopped live database); nothing is written.
pub fn referenced_digests_in(path: &Path) -> StorageResult<Vec<String>> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    referenced_digests(&conn)
}

/// Whether a store-relative path is made only of normal components (used by
/// verification before joining a stored `storage_ref` to a root).
pub fn is_plain_relative(rel: &str) -> bool {
    let path = Path::new(rel);
    !rel.is_empty()
        && !path.is_absolute()
        && path.components().all(|c| matches!(c, Component::Normal(_)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(bytes: &[u8]) -> String {
        hex::encode(Sha256::digest(bytes))
    }

    #[test]
    fn a_copy_whose_source_is_missing_creates_nothing_at_the_destination() {
        // Final security review (ipc hardening): the destination's shard
        // directories were created before the source was looked up, so a hostile
        // snapshot listing many digests created empty directories in the live
        // store.
        let from = tempfile::tempdir().unwrap();
        let to = tempfile::tempdir().unwrap();
        let sha = digest(b"never stored");
        assert!(matches!(
            copy_between(from.path(), to.path(), &sha),
            Err(StorageError::NotFound(_))
        ));
        assert_eq!(fs::read_dir(to.path()).unwrap().count(), 0);
    }

    #[cfg(unix)]
    #[test]
    fn a_copy_never_reads_a_source_that_is_not_a_regular_file() {
        // A FIFO at the source address is refused, never opened for reading
        // (which would block until a writer appears).
        let from = tempfile::tempdir().unwrap();
        let to = tempfile::tempdir().unwrap();
        let sha = digest(b"fifo");
        let path = from.path().join(relpath(&sha).unwrap());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        rustix::fs::mknodat(
            rustix::fs::CWD,
            &path,
            rustix::fs::FileType::Fifo,
            rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
            0,
        )
        .unwrap();
        assert!(copy_between(from.path(), to.path(), &sha).is_err());
    }

    #[test]
    fn a_put_lands_at_the_content_address_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let bytes = b"lab notes";
        let sha = digest(bytes);
        assert_eq!(put(dir.path(), &sha, bytes).unwrap(), Stored::Written);
        let path = dir.path().join(relpath(&sha).unwrap());
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(
            put(dir.path(), &sha, bytes).unwrap(),
            Stored::AlreadyPresent
        );
        assert!(holds_intact(dir.path(), &sha).unwrap());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn remove_takes_exactly_the_file_it_is_given_and_nothing_reached_another_way() {
        let dir = tempfile::tempdir().unwrap();
        let (one, two) = (digest(b"one"), digest(b"two"));
        put(dir.path(), &one, b"one").unwrap();
        put(dir.path(), &two, b"two").unwrap();
        let rel = relpath(&one).unwrap();
        assert!(remove(dir.path(), &rel).unwrap());
        assert!(!dir.path().join(&rel).exists());
        assert!(
            holds_intact(dir.path(), &two).unwrap(),
            "another file was touched"
        );
        // Gone already, or a store that does not exist: nothing to do, not an error.
        assert!(!remove(dir.path(), &rel).unwrap());
        assert!(!remove(&dir.path().join("missing"), &rel).unwrap());
        // A stored path that is not the content address is never followed.
        for bad in [
            "../outside",
            "/etc/passwd",
            "ab/cd/not-a-digest",
            &format!("zz/zz/{one}"),
            &format!("{}/{}/../../{one}", &one[0..2], &one[2..4]),
            "",
        ] {
            assert!(
                matches!(remove(dir.path(), bad), Err(StorageError::Precondition(_))),
                "{bad:?}"
            );
        }
        assert!(holds_intact(dir.path(), &two).unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn remove_does_not_follow_a_link_out_of_the_store() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let sha = digest(b"victim");
        // The shard folder is a link to a folder outside the store that holds a file with the right name.
        let shard = outside.path().join(&sha[2..4]);
        fs::create_dir_all(&shard).unwrap();
        fs::write(shard.join(&sha), b"victim").unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join(&sha[0..2])).unwrap();
        assert!(matches!(
            remove(dir.path(), &relpath(&sha).unwrap()),
            Err(StorageError::Precondition(_))
        ));
        assert!(
            shard.join(&sha).is_file(),
            "a file outside the store was removed"
        );
        // The file itself is a link.
        let dir = tempfile::tempdir().unwrap();
        let target = outside.path().join("target");
        fs::write(&target, b"victim").unwrap();
        let inner = dir.path().join(&sha[0..2]).join(&sha[2..4]);
        fs::create_dir_all(&inner).unwrap();
        std::os::unix::fs::symlink(&target, inner.join(&sha)).unwrap();
        assert!(matches!(
            remove(dir.path(), &relpath(&sha).unwrap()),
            Err(StorageError::Precondition(_))
        ));
        assert!(target.is_file());
    }

    #[test]
    fn bytes_that_do_not_match_the_digest_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let sha = digest(b"one");
        assert!(matches!(
            put(dir.path(), &sha, b"two"),
            Err(StorageError::InvalidInput(_))
        ));
        assert!(matches!(
            put(dir.path(), "../../etc/passwd", b"two"),
            Err(StorageError::InvalidInput(_))
        ));
    }

    #[test]
    fn a_damaged_file_is_replaced_and_a_damaged_source_is_not_copied() {
        let live = tempfile::tempdir().unwrap();
        let backup = tempfile::tempdir().unwrap();
        let bytes = b"evidence";
        let sha = digest(bytes);
        put(live.path(), &sha, bytes).unwrap();
        let path = live.path().join(relpath(&sha).unwrap());
        fs::write(&path, b"tampered").unwrap();
        assert!(!holds_intact(live.path(), &sha).unwrap());
        assert!(matches!(
            copy_between(live.path(), backup.path(), &sha),
            Err(StorageError::Corrupt(_))
        ));
        assert!(!holds_intact(backup.path(), &sha).unwrap());
        // The right bytes repair the damaged file.
        assert_eq!(put(live.path(), &sha, bytes).unwrap(), Stored::Written);
        assert_eq!(
            copy_between(live.path(), backup.path(), &sha).unwrap(),
            Stored::Written
        );
        assert!(holds_intact(backup.path(), &sha).unwrap());
    }

    #[test]
    fn a_file_already_intact_at_the_destination_needs_no_source() {
        // Review finding 8: the source was required before the destination
        // was looked at, so a file already in place was reported missing.
        let from = tempfile::tempdir().unwrap();
        let to = tempfile::tempdir().unwrap();
        let bytes = b"already there";
        let sha = digest(bytes);
        put(to.path(), &sha, bytes).unwrap();
        assert_eq!(
            copy_between(from.path(), to.path(), &sha).unwrap(),
            Stored::AlreadyPresent
        );
        assert!(copy_all(from.path(), to.path(), &[sha]).unwrap().complete());
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_shard_directory_cannot_move_a_write_outside_the_store() {
        let store = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let bytes = b"x";
        let sha = digest(bytes);
        std::os::unix::fs::symlink(outside.path(), store.path().join(&sha[0..2])).unwrap();
        assert!(matches!(
            put(store.path(), &sha, bytes),
            Err(StorageError::Conflict(_))
        ));
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
    }

    #[test]
    fn storage_refs_must_be_the_content_address() {
        let sha = digest(b"y");
        check_storage_ref(&relpath(&sha).unwrap(), &sha).unwrap();
        assert!(check_storage_ref("../x", &sha).is_err());
        assert!(is_plain_relative("ab/cd/ef"));
        assert!(!is_plain_relative("/abs"));
        assert!(!is_plain_relative("a/../b"));
    }
}
