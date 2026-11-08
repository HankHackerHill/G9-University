#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
// `G9Error` is the closed IPC error contract (ADR-0013) and is carried by value.
#![allow(clippy::result_large_err)]
//! # g9-cli — command-line driver for G9 MENTOR (`g9`)
//!
//! A thin client of [`g9_app::dispatch`]. It composes the backend exactly as
//! the Tauri bridge does (`apps/desktop/src-tauri/src/lib.rs`): private data
//! directory, binding key from `<data>/binding.key` ([`g9_app::keyfile`]),
//! `Database::open` of `<data>/g9.sqlite`, `AppContext::builder(..).open()`
//! (which runs recovery), the backend-assigned operator actor and the
//! `NullRuntime`. It never bypasses the command pipeline, never writes SQL and
//! never sets the actor from input.
//!
//! | Module | Contract |
//! |---|---|
//! | [`cli`] | clap definitions (`g9 init`, `status`, `ops`, `call`, `approve`, `reject`, `approvals`, `import-seed`, `simulate-week`, `crash-drill`, `restore`); `simulate-week` and `crash-drill` refuse to run without `--scratch` |
//! | [`meta`] | wire `ClientCommandMeta` built like `apps/desktop/src/ipc/client.ts` (uuid v7 ids, schema version 1.0, default scope per operation, **no actor**) |
//! | [`bootstrap`] | data layout and context construction (mirrors the bridge) |
//! | [`client`] | dispatch wrapper: envelope/response schema checks, approval round trips |
//! | [`clock`] | stepping clock for deterministic runs |
//! | [`seed`] | repository-path materialization for data that has no IPC path yet (labelled `SEEDED`, never through `dispatch`) |
//! | [`commands`] | one function per subcommand; [`commands::simulate`] is the Stage 03 exit criterion script; [`commands::restore`] is the offline restore (`g9_storage::restore`, no SQL here) |
//! | [`output`] | exit codes (0 ok, 2 `G9Error` or scenario `INCOMPLETE`/`FAIL`, 1 usage/IO or an unconfirmed `g9 restore`) and JSON/human rendering |

pub mod bootstrap;
pub mod cli;
pub mod client;
pub mod clock;
pub mod commands;
pub mod meta;
pub mod output;
pub mod seed;

pub use cli::Cli;
pub use output::{CliError, Completion};
