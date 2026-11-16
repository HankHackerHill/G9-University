//! The operations log (plan steps OBS-1 and OBS-2, `docs/release/path-to-10.md`):
//! a local, size-capped record of what the backend did, for diagnosis without a
//! debugger. Before it existed the `tracing` calls of the backend wrote nowhere.
//!
//! Two kinds of line, one JSON object each, in `<data>/logs/g9.log`:
//!
//! * `"kind":"command"`: one per dispatched command: time, operation, request
//!   id, outcome category, error code, duration. The request id is the one the
//!   audit row of the same command carries (`audit_metadata.request_id`), so
//!   one id finds both.
//! * `"kind":"event"`: one per diagnostic event the backend raises through
//!   `tracing` (a storage I/O failure, a recovery finding): level, source
//!   location, the message, and the fields of [`ALLOWED_FIELDS`].
//!
//! What a line can hold is decided here, not at the call site:
//!
//! * a command line holds an operation name only when the registry knows it
//!   (the caller's own text is never written), and a request id or an error
//!   code only when it is code-shaped ([`token`]);
//! * an event line holds a field only when its name is on the allow-list and
//!   its value is short and made of the characters codes, numbers and enum
//!   names are made of: no slash, so no path; anything else is written as
//!   `<redacted>`;
//! * the message of an event is the literal of the call site. The source gate
//!   `crates/g9-app/tests/log_call_sites.rs` fails when a call site formats a
//!   value into its message, names a field that is not on the allow-list, or
//!   is not in the reviewed register of call sites.
//!
//! So no payload, title, file name, path or free text reaches the log, and a
//! new logging call cannot be added without the register changing in the same
//! change. The log is metadata about activity (which operation, when, how
//! long): it stays in the data directory, mode `0600`, is not part of a backup
//! or an export, and is bounded to [`KEPT_FILES`] files of [`MAX_FILE_BYTES`].
//!
//! Writing never fails a command: a line that cannot be written is counted
//! ([`OpLog::dropped`]) and dropped. Lines are not synced one by one; the last
//! lines before a power loss can be missing. The audit ledger, not this file,
//! is the durable record.

use g9_domain::{ErrorCategory, G9Error, UtcMillis};
use serde_json::{json, Map, Value};
use std::fmt;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// Directory of the log under the data directory.
pub const LOG_DIR_NAME: &str = "logs";
/// The file being written; older ones are `g9.log.1` (newest) upwards.
pub const LOG_FILE_NAME: &str = "g9.log";
/// Size at which the current file is rotated.
pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
/// Files kept, the current one included: the log never exceeds
/// `KEPT_FILES * MAX_FILE_BYTES` plus one line.
pub const KEPT_FILES: u32 = 4;

/// The only field names an event line can carry. Each is a code, a number, an
/// identifier the backend made, or the name of an enum variant.
pub const ALLOWED_FIELDS: &[&str] = &[
    "code",
    "context",
    "differences",
    "extended_code",
    "first_bad_seq",
    "kind",
    "migration",
    "operation",
    "original",
    "pending",
    "schema",
    "session",
    "sqlite",
    "version",
    "what",
];

/// Written in place of a value that is not of the allowed shape.
pub const REDACTED: &str = "<redacted>";
/// Written in place of an operation name the registry does not know.
pub const UNKNOWN_OPERATION: &str = "unknown";

const MAX_TOKEN_BYTES: usize = 128;
const MAX_VALUE_BYTES: usize = 160;
const MAX_MESSAGE_BYTES: usize = 200;

/// `value` when it is code-shaped: 1 to 128 characters of `[A-Za-z0-9._:-]`.
/// Operation names, error codes, request ids and record ids are; a title, a
/// path or a sentence is not.
pub fn token(value: &str) -> Option<&str> {
    let ok = (1..=MAX_TOKEN_BYTES).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b':' | b'-'));
    ok.then_some(value)
}

fn token_or_redacted(value: &str) -> &str {
    token(value).unwrap_or(REDACTED)
}

/// A rendered field value: what `Debug` and `Display` give for numbers, enum
/// variants, options and lists of those (`NotFound`, `Some(12)`, `[14, 15]`).
/// No slash or backslash, so never a path.
fn field_value(value: &str) -> &str {
    let ok = (1..=MAX_VALUE_BYTES).contains(&value.len())
        && value.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b' ' | b'_' | b'.' | b':' | b',' | b'-' | b'[' | b']' | b'(' | b')' | b'"'
                )
        });
    if ok {
        value
    } else {
        REDACTED
    }
}

/// The message of an event: a plain English literal.
fn message_text(value: &str) -> &str {
    let ok = (1..=MAX_MESSAGE_BYTES).contains(&value.len())
        && value.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b' ' | b'_' | b'.' | b':' | b',' | b';' | b'\'' | b'-' | b'(' | b')'
                )
        });
    if ok {
        value
    } else {
        REDACTED
    }
}

/// `file:line` of a call site, for a source file of this workspace (a relative
/// path as the compiler was given it). An absolute path is reduced to its file
/// name: the directory a build ran in is not the log's business.
fn source_location(file: Option<&str>, line: Option<u32>) -> Option<String> {
    let file = file?;
    let file = if file.starts_with('/') {
        file.rsplit('/').next().unwrap_or(file)
    } else {
        file
    };
    let ok = (1..=MAX_VALUE_BYTES).contains(&file.len())
        && file
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/'));
    ok.then(|| match line {
        Some(line) => format!("{file}:{line}"),
        None => file.to_string(),
    })
}

fn category_name(category: ErrorCategory) -> String {
    serde_json::to_value(category)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_else(|| "INTERNAL".to_string())
}

#[derive(Debug)]
struct State {
    file: Option<File>,
    size: u64,
    written: u64,
    dropped: u64,
}

/// The log of one data directory. Cheap to share: the context that dispatches
/// commands and the `tracing` subscriber of the process hold the same one.
#[derive(Debug)]
pub struct OpLog {
    dir: PathBuf,
    state: Mutex<State>,
}

impl OpLog {
    /// Open (and create when needed) the log in `dir`, which must be absolute
    /// and whose parent must exist. The directory is made `0700` and the file
    /// `0600`. A symbolic link or anything else that is not a directory (a
    /// regular file, for the log file) is refused and never followed.
    pub fn open(dir: &Path) -> Result<Arc<Self>, G9Error> {
        if !dir.is_absolute() {
            return Err(G9Error::precondition_failed(
                "log.dir_not_absolute",
                "the log directory must be an absolute path",
            ));
        }
        let unavailable = |_: io::Error| {
            G9Error::resource_unavailable(
                "log.unavailable",
                "the log directory could not be used; the application runs without a log",
            )
        };
        platform::ensure_private_dir(dir).map_err(unavailable)?;
        let (file, size) =
            platform::open_private_append(&dir.join(LOG_FILE_NAME)).map_err(unavailable)?;
        Ok(Arc::new(Self {
            dir: dir.to_path_buf(),
            state: Mutex::new(State {
                file: Some(file),
                size,
                written: 0,
                dropped: 0,
            }),
        }))
    }

    /// The directory this log writes to.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Lines written by this process.
    pub fn written(&self) -> u64 {
        self.lock().written
    }

    /// Lines this process could not write (the disk, a removed directory).
    pub fn dropped(&self) -> u64 {
        self.lock().dropped
    }

    /// One dispatched command. `operation` must be a name the registry knows,
    /// or [`UNKNOWN_OPERATION`]; `request_id` is written only when it is
    /// code-shaped.
    pub fn command(
        &self,
        at: UtcMillis,
        operation: &str,
        request_id: Option<&str>,
        outcome: &Result<Value, G9Error>,
        duration_ms: u64,
    ) {
        let (category, code, replayed) = match outcome {
            Ok(value) => (
                "OK".to_string(),
                Value::Null,
                value
                    .get("replayed")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            ),
            Err(e) => (
                category_name(e.category),
                Value::from(token_or_redacted(&e.code)),
                false,
            ),
        };
        self.write(&json!({
            "ts": at.as_millis(),
            "kind": "command",
            "op": token_or_redacted(operation),
            "request_id": request_id.and_then(token),
            "outcome": category,
            "code": code,
            "duration_ms": duration_ms,
            "replayed": replayed,
        }));
    }

    /// One diagnostic event. `fields` are `(name, rendered value)`; a name that
    /// is not on the allow-list is counted, not written.
    pub fn event(
        &self,
        at: UtcMillis,
        level: &str,
        target: &str,
        source: Option<String>,
        message: Option<&str>,
        fields: &[(&str, String)],
    ) {
        let mut kept = Map::new();
        let mut not_allowed: u64 = 0;
        for (name, value) in fields {
            if ALLOWED_FIELDS.contains(name) {
                kept.insert((*name).to_string(), Value::from(field_value(value)));
            } else {
                not_allowed += 1;
            }
        }
        self.write(&json!({
            "ts": at.as_millis(),
            "kind": "event",
            "level": token_or_redacted(level),
            "target": token_or_redacted(target),
            "src": source,
            "message": message.map(message_text),
            "fields": kept,
            "fields_not_allowed": not_allowed,
        }));
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while the lock was held leaves counters and a file handle:
        // both are still usable.
        match self.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    fn write(&self, line: &Value) {
        let mut text = line.to_string();
        text.push('\n');
        let mut state = self.lock();
        match self.put(&mut state, text.as_bytes()) {
            Ok(()) => state.written = state.written.saturating_add(1),
            Err(_) => {
                state.dropped = state.dropped.saturating_add(1);
                // Reopen on the next line: the directory may be back by then.
                state.file = None;
            }
        }
    }

    fn put(&self, state: &mut State, bytes: &[u8]) -> io::Result<()> {
        let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if state.file.is_none() {
            let (file, size) = platform::open_private_append(&self.file_name(0))?;
            state.file = Some(file);
            state.size = size;
        }
        if state.size > 0 && state.size.saturating_add(len) > MAX_FILE_BYTES {
            self.rotate(state)?;
        }
        let Some(file) = state.file.as_mut() else {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        };
        file.write_all(bytes)?;
        state.size = state.size.saturating_add(len);
        Ok(())
    }

    fn file_name(&self, index: u32) -> PathBuf {
        if index == 0 {
            self.dir.join(LOG_FILE_NAME)
        } else {
            self.dir.join(format!("{LOG_FILE_NAME}.{index}"))
        }
    }

    /// `g9.log` becomes `g9.log.1`, `.1` becomes `.2`, and the oldest is
    /// removed. A rename moves a directory entry: it never follows a link.
    fn rotate(&self, state: &mut State) -> io::Result<()> {
        fn absent_is_fine(result: io::Result<()>) -> io::Result<()> {
            match result {
                Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        }
        state.file = None;
        let oldest = KEPT_FILES.saturating_sub(1);
        if oldest == 0 {
            absent_is_fine(fs::remove_file(self.file_name(0)))?;
        } else {
            absent_is_fine(fs::remove_file(self.file_name(oldest)))?;
            for index in (0..oldest).rev() {
                absent_is_fine(fs::rename(self.file_name(index), self.file_name(index + 1)))?;
            }
        }
        let (file, size) = platform::open_private_append(&self.file_name(0))?;
        state.file = Some(file);
        state.size = size;
        Ok(())
    }
}

/// Clock of the event lines.
pub type EventClock = Arc<dyn Fn() -> UtcMillis + Send + Sync>;

/// The `tracing` subscriber that writes the backend's diagnostic events to an
/// [`OpLog`]. Levels INFO, WARN and ERROR; spans are not recorded (the backend
/// opens none).
pub struct EventSubscriber {
    log: Arc<OpLog>,
    clock: EventClock,
}

impl EventSubscriber {
    /// Events of `log`, stamped by `clock`.
    pub fn new(log: Arc<OpLog>, clock: EventClock) -> Self {
        Self { log, clock }
    }
}

impl fmt::Debug for EventSubscriber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventSubscriber")
            .field("log", &self.log.dir)
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct Fields {
    message: Option<String>,
    fields: Vec<(&'static str, String)>,
}

impl Fields {
    fn take(&mut self, field: &tracing::field::Field, rendered: String) {
        if field.name() == "message" {
            self.message = Some(rendered);
        } else {
            self.fields.push((field.name(), rendered));
        }
    }
}

impl tracing::field::Visit for Fields {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.take(field, value.to_string());
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
        self.take(field, format!("{value:?}"));
    }
}

impl tracing::Subscriber for EventSubscriber {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        // More verbose levels compare greater.
        *metadata.level() <= tracing::Level::INFO
    }

    fn new_span(&self, _attributes: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }

    fn record(&self, _span: &tracing::span::Id, _values: &tracing::span::Record<'_>) {}

    fn record_follows_from(&self, _span: &tracing::span::Id, _follows: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut seen = Fields::default();
        event.record(&mut seen);
        let metadata = event.metadata();
        self.log.event(
            (self.clock)(),
            metadata.level().as_str(),
            metadata.target(),
            source_location(metadata.file(), metadata.line()),
            seen.message.as_deref(),
            &seen.fields,
        );
    }

    fn enter(&self, _span: &tracing::span::Id) {}

    fn exit(&self, _span: &tracing::span::Id) {}
}

/// Send the diagnostic events of this process to `log`, stamped by the host
/// clock. Returns `false` when the process already has a subscriber (the first
/// one stays). Call it once, before the backend is opened: recovery raises
/// events too.
pub fn install_event_log(log: Arc<OpLog>) -> bool {
    tracing::subscriber::set_global_default(EventSubscriber::new(log, Arc::new(UtcMillis::now)))
        .is_ok()
}

/// Run `f` with the diagnostic events of the current thread going to `log`
/// (tests, and tools that open more than one data directory in one process).
pub fn with_event_log<T>(log: Arc<OpLog>, clock: EventClock, f: impl FnOnce() -> T) -> T {
    tracing::subscriber::with_default(EventSubscriber::new(log, clock), f)
}

#[cfg(unix)]
mod platform {
    use std::fs::{self, DirBuilder, File, OpenOptions, Permissions};
    use std::io;
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    use std::path::Path;

    fn refused(what: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidInput, what)
    }

    /// `dir` as a directory only its owner can enter. A link at `dir` is
    /// refused, not followed.
    pub(super) fn ensure_private_dir(dir: &Path) -> io::Result<()> {
        match fs::symlink_metadata(dir) {
            Ok(meta) if meta.file_type().is_dir() => {
                if meta.permissions().mode() & 0o077 != 0 {
                    fs::set_permissions(dir, Permissions::from_mode(0o700))?;
                }
                Ok(())
            }
            Ok(_) => Err(refused("the log directory is not a directory")),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                DirBuilder::new().mode(0o700).create(dir)
            }
            Err(e) => Err(e),
        }
    }

    /// `path` opened for appending, created `0600` when absent, with its
    /// current size. A link or any other non-regular file at `path` is refused.
    /// (Between the check and the open another process of the same user could
    /// plant a link: that user already owns the data directory.)
    pub(super) fn open_private_append(path: &Path) -> io::Result<(File, u64)> {
        match fs::symlink_metadata(path) {
            Ok(meta) if meta.file_type().is_file() => {}
            Ok(_) => return Err(refused("the log file is not a regular file")),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .mode(0o600)
            .open(path)?;
        let meta = file.metadata()?;
        if !meta.file_type().is_file() {
            return Err(refused("the log file is not a regular file"));
        }
        if meta.permissions().mode() & 0o077 != 0 {
            file.set_permissions(Permissions::from_mode(0o600))?;
        }
        Ok((file, meta.len()))
    }
}

#[cfg(not(unix))]
mod platform {
    use std::fs::File;
    use std::io;
    use std::path::Path;

    // Private files are implemented for Unix only (as for the binding key,
    // `keyfile.rs`): elsewhere the application runs without a log.
    pub(super) fn ensure_private_dir(_dir: &Path) -> io::Result<()> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    pub(super) fn open_private_append(_path: &Path) -> io::Result<(File, u64)> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const T: i64 = 1_795_336_200_000;

    fn at(offset: i64) -> UtcMillis {
        UtcMillis::from_millis(T + offset)
    }

    fn lines(dir: &Path, name: &str) -> Vec<Value> {
        fs::read_to_string(dir.join(name))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    fn mode(path: &Path) -> u32 {
        fs::symlink_metadata(path).unwrap().permissions().mode() & 0o777
    }

    fn opened() -> (tempfile::TempDir, Arc<OpLog>) {
        let root = tempfile::tempdir().unwrap();
        let log = OpLog::open(&root.path().join(LOG_DIR_NAME)).unwrap();
        (root, log)
    }

    #[test]
    fn the_directory_and_the_file_are_private_whatever_the_umask_left() {
        let (root, log) = opened();
        assert_eq!(mode(log.dir()), 0o700);
        assert_eq!(mode(&log.dir().join(LOG_FILE_NAME)), 0o600);
        // A directory and a file someone widened are narrowed again at the next open.
        fs::set_permissions(log.dir(), fs::Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(
            log.dir().join(LOG_FILE_NAME),
            fs::Permissions::from_mode(0o644),
        )
        .unwrap();
        drop(log);
        let log = OpLog::open(&root.path().join(LOG_DIR_NAME)).unwrap();
        assert_eq!(mode(log.dir()), 0o700);
        assert_eq!(mode(&log.dir().join(LOG_FILE_NAME)), 0o600);
    }

    #[test]
    fn a_relative_directory_is_refused() {
        let e = OpLog::open(Path::new("logs")).unwrap_err();
        assert_eq!(e.code, "log.dir_not_absolute");
    }

    #[test]
    fn a_link_in_place_of_the_directory_or_the_file_is_refused_and_not_followed() {
        let root = tempfile::tempdir().unwrap();
        let elsewhere = root.path().join("elsewhere");
        fs::create_dir(&elsewhere).unwrap();
        std::os::unix::fs::symlink(&elsewhere, root.path().join(LOG_DIR_NAME)).unwrap();
        let e = OpLog::open(&root.path().join(LOG_DIR_NAME)).unwrap_err();
        assert_eq!(e.code, "log.unavailable");
        assert_eq!(e.category, ErrorCategory::ResourceUnavailable);
        assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);

        let dir = root.path().join("real");
        fs::create_dir(&dir).unwrap();
        let target = root.path().join("target.txt");
        fs::write(&target, "untouched").unwrap();
        std::os::unix::fs::symlink(&target, dir.join(LOG_FILE_NAME)).unwrap();
        let e = OpLog::open(&dir).unwrap_err();
        assert_eq!(e.code, "log.unavailable");
        assert_eq!(fs::read_to_string(&target).unwrap(), "untouched");
    }

    #[test]
    fn a_command_line_has_exactly_these_fields() {
        let (_root, log) = opened();
        log.command(
            at(0),
            "notes.create",
            Some("req-1"),
            &Ok(json!({ "result": {}, "replayed": false })),
            12,
        );
        log.command(
            at(5),
            "notes.create",
            Some("req-2"),
            &Err(G9Error::invalid_input(
                "ipc.payload_invalid",
                "a message that is not logged",
            )),
            1,
        );
        log.command(
            at(9),
            "notes.create",
            Some("req-3"),
            &Ok(json!({ "replayed": true })),
            0,
        );
        assert_eq!(
            fs::read_to_string(log.dir().join(LOG_FILE_NAME)).unwrap(),
            format!(
                "{{\"ts\":{T},\"kind\":\"command\",\"op\":\"notes.create\",\"request_id\":\"req-1\",\"outcome\":\"OK\",\"code\":null,\"duration_ms\":12,\"replayed\":false}}\n\
                 {{\"ts\":{},\"kind\":\"command\",\"op\":\"notes.create\",\"request_id\":\"req-2\",\"outcome\":\"INVALID_INPUT\",\"code\":\"ipc.payload_invalid\",\"duration_ms\":1,\"replayed\":false}}\n\
                 {{\"ts\":{},\"kind\":\"command\",\"op\":\"notes.create\",\"request_id\":\"req-3\",\"outcome\":\"OK\",\"code\":null,\"duration_ms\":0,\"replayed\":true}}\n",
                T + 5,
                T + 9
            )
        );
        assert_eq!((log.written(), log.dropped()), (3, 0));
    }

    #[test]
    fn text_that_is_not_code_shaped_never_reaches_a_command_line() {
        let (_root, log) = opened();
        log.command(
            at(0),
            "notes.create; a title: SECRET-MARKER",
            Some("a request id with spaces SECRET-MARKER"),
            &Err(
                G9Error::invalid_input("a code/with/a/path SECRET-MARKER", "SECRET-MARKER")
                    .with_details(json!({ "field": "SECRET-MARKER" })),
            ),
            3,
        );
        let text = fs::read_to_string(log.dir().join(LOG_FILE_NAME)).unwrap();
        assert!(!text.contains("SECRET-MARKER"), "{text}");
        let line = &lines(log.dir(), LOG_FILE_NAME)[0];
        assert_eq!(line["op"], REDACTED);
        assert_eq!(line["request_id"], Value::Null);
        assert_eq!(line["code"], REDACTED);
        assert_eq!(line["outcome"], "INVALID_INPUT");
    }

    #[test]
    fn an_event_keeps_allow_listed_fields_of_the_allowed_shape_only() {
        let (_root, log) = opened();
        let clock: EventClock = Arc::new(|| at(42));
        with_event_log(log.clone(), clock, || {
            tracing::warn!(kind = ?io::ErrorKind::NotFound, what = "rename", "restore swap failed");
            tracing::error!(
                code = %"storage.io",
                first_bad_seq = ?Some(12_u64),
                pending = ?[14, 15],
                "audit chain verification failed; mutations are refused"
            );
            // Not on the allow-list: counted, never written.
            tracing::error!(title = "SECRET-MARKER", details = ?"SECRET-MARKER", "refused");
            // On the allow-list but not of the allowed shape: a path, a sentence with a marker.
            tracing::warn!(what = "/home/someone/SECRET-MARKER.txt", "a path");
            tracing::warn!(context = "note says: SECRET-MARKER!", "a sentence");
            // A message a value was formatted into, with characters a literal here never has.
            tracing::warn!("title was {}", "<SECRET-MARKER>");
            // Below the recorded levels.
            tracing::debug!(code = "x", "not recorded");
        });
        let text = fs::read_to_string(log.dir().join(LOG_FILE_NAME)).unwrap();
        assert!(!text.contains("SECRET-MARKER"), "{text}");
        let got = lines(log.dir(), LOG_FILE_NAME);
        assert_eq!(got.len(), 6, "{text}");
        assert_eq!(
            got[0],
            json!({
                "ts": T + 42, "kind": "event", "level": "WARN", "target": "g9_app::oplog::tests",
                "src": got[0]["src"], "message": "restore swap failed",
                "fields": { "kind": "NotFound", "what": "rename" }, "fields_not_allowed": 0
            })
        );
        let src = got[0]["src"].as_str().unwrap();
        assert!(
            src.starts_with("crates/g9-app/src/oplog.rs:") && !src.starts_with('/'),
            "{src}"
        );
        assert_eq!(
            got[1]["fields"],
            json!({ "code": "storage.io", "first_bad_seq": "Some(12)", "pending": "[14, 15]" })
        );
        assert_eq!(got[2]["fields"], json!({}));
        assert_eq!(got[2]["fields_not_allowed"], 2);
        assert_eq!(got[3]["fields"], json!({ "what": REDACTED }));
        assert_eq!(got[4]["fields"], json!({ "context": REDACTED }));
        assert_eq!(got[5]["message"], REDACTED);
    }

    #[test]
    fn the_log_is_bounded_and_the_newest_lines_are_in_the_current_file() {
        let (_root, log) = opened();
        let outcome = Ok(json!({}));
        // About 130 bytes a line: enough lines for more than KEPT_FILES files.
        let per_file = MAX_FILE_BYTES / 100;
        let total = per_file * u64::from(KEPT_FILES + 2);
        for n in 0..total {
            log.command(
                at(0),
                "notes.create",
                Some(&format!("req-{n}")),
                &outcome,
                n,
            );
        }
        assert_eq!((log.written(), log.dropped()), (total, 0));
        let mut names: Vec<String> = fs::read_dir(log.dir())
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        assert_eq!(names, vec!["g9.log", "g9.log.1", "g9.log.2", "g9.log.3"]);
        let mut bytes = 0;
        for name in &names {
            let len = fs::metadata(log.dir().join(name)).unwrap().len();
            assert!(len <= MAX_FILE_BYTES, "{name}: {len}");
            assert_eq!(mode(&log.dir().join(name)), 0o600, "{name}");
            bytes += len;
        }
        assert!(bytes <= u64::from(KEPT_FILES) * MAX_FILE_BYTES);
        let current = lines(log.dir(), LOG_FILE_NAME);
        assert_eq!(
            current.last().unwrap()["request_id"],
            format!("req-{}", total - 1)
        );
        // Older lines are in the higher-numbered files, in order.
        let older = lines(log.dir(), "g9.log.1");
        assert!(
            older.last().unwrap()["duration_ms"].as_u64().unwrap()
                < current[0]["duration_ms"].as_u64().unwrap()
        );
    }

    #[test]
    fn a_log_that_cannot_be_written_drops_lines_and_recovers() {
        let (_root, log) = opened();
        let outcome = Ok(json!({}));
        log.command(at(0), "system.status", Some("req-1"), &outcome, 1);
        // The directory disappears under the open file (a cleanup tool, a mistake).
        fs::remove_dir_all(log.dir()).unwrap();
        // The handle still accepts bytes for the unlinked file; force a reopen.
        log.lock().file = None;
        log.command(at(1), "system.status", Some("req-2"), &outcome, 1);
        assert_eq!((log.written(), log.dropped()), (1, 1));
        // The directory comes back: the next line is written again.
        fs::create_dir(log.dir()).unwrap();
        log.command(at(2), "system.status", Some("req-3"), &outcome, 1);
        assert_eq!((log.written(), log.dropped()), (2, 1));
        assert_eq!(lines(log.dir(), LOG_FILE_NAME)[0]["request_id"], "req-3");
    }

    #[test]
    fn shapes() {
        assert_eq!(
            token("approval.operation_mismatch"),
            Some("approval.operation_mismatch")
        );
        assert_eq!(token("019a-uuid:7"), Some("019a-uuid:7"));
        for bad in ["", "a b", "a/b", "caf\u{e9}", &"x".repeat(129)] {
            assert_eq!(token(bad), None, "{bad:?}");
        }
        assert_eq!(field_value("Some(12)"), "Some(12)");
        assert_eq!(
            field_value("[\"0014_x\", \"0015_y\"]"),
            "[\"0014_x\", \"0015_y\"]"
        );
        for bad in ["", "a/b", "a\\b", "a=b", "a\nb", "<x>", &"x".repeat(161)] {
            assert_eq!(field_value(bad), REDACTED, "{bad:?}");
        }
        assert_eq!(
            message_text("schema differs from the embedded migrations; mutations are refused"),
            "schema differs from the embedded migrations; mutations are refused"
        );
        assert_eq!(message_text("a \"quoted\" title"), REDACTED);
        assert_eq!(
            source_location(Some("crates/g9-storage/src/restore.rs"), Some(785)).as_deref(),
            Some("crates/g9-storage/src/restore.rs:785")
        );
        assert_eq!(
            source_location(Some("/home/builder/work/src/lib.rs"), Some(3)).as_deref(),
            Some("lib.rs:3")
        );
        assert_eq!(source_location(Some("a b.rs"), Some(1)), None);
        assert_eq!(source_location(None, Some(1)), None);
    }
}
