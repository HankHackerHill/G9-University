//! `g9 crash-drill`: prepare and start a session, then leave the process
//! without completing it. The open monotonic interval and the `RUNNING`
//! session are the bytes a killed process leaves behind; the next open
//! (`g9 init`) must close the interval `UNCERTAIN` and move the session to
//! `INTERRUPTED` (ARCHITECTURE.md §7, gate A03, `g9_app::recovery`).
//!
//! Two-step drill:
//!
//! ```text
//! g9 crash-drill --scratch --data-dir DIR   # prints the RUNNING session, exits without cleanup
//! g9 init --data-dir DIR                    # recovery: interrupted_sessions = [that id], interval UNCERTAIN
//! ```
//!
//! The drill writes a seed row directly to storage, so it refuses to run
//! without `--scratch` (see [`super::require_scratch`]): point it at a throwaway
//! data directory only.
//!
//! The session row itself is materialized through the storage repository
//! (`seed::materialize_drill_session`) because planned sessions have no IPC
//! creation path yet; the two transitions go through `dispatch`.

use crate::bootstrap::{self, BootOptions};
use crate::client::Client;
use crate::output::{CliError, Completion};
use crate::seed;
use serde_json::json;

/// Run `crash-drill`. The returned completion carries the live client so that
/// `main` exits without running its destructor.
pub fn run(boot: &BootOptions) -> Result<Completion, CliError> {
    let booted = bootstrap::open(boot)?;
    let mut client = Client::new(booted.ctx);
    let session_id =
        seed::materialize_drill_session(client.ctx.database(), &booted.zone, client.now())?;
    let id = session_id.as_str().to_string();
    client.call("session.prepare", json!({ "session_id": id }))?;
    let started = client.call("session.start", json!({ "session_id": id }))?;
    let view = &started["result"];
    let (next, next_argv) = crate::output::next_command(&[
        "g9".to_string(),
        "init".to_string(),
        "--data-dir".to_string(),
        booted.layout.root.to_string_lossy().to_string(),
    ]);
    let value = json!({
        "drill": "crash",
        "session_id": id,
        "state": view["state"],
        "open_interval_started_at": view["open_interval_started_at"],
        "intervals": view["intervals"],
        "exit": "process exits now without completing the session or closing the database",
        "next": next,
        "next_argv": next_argv,
        "next_note": crate::output::NEXT_COMMAND_NOTE,
        "expect_after_init": {
            "recovery.interrupted_sessions": [id],
            "interrupted_sessions[].state": "INTERRUPTED",
            "interrupted_sessions[].intervals[].continuity": "UNCERTAIN",
        },
    });
    Ok(Completion {
        value,
        abandon: Some(client),
    })
}
