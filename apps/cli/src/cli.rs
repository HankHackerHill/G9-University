//! Command-line surface (clap derive). Global options may be written before or
//! after the subcommand (`g9 init --data-dir D` and `g9 --data-dir D init`).

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// G9 MENTOR command-line driver: a thin client of the backend command pipeline.
#[derive(Debug, Parser)]
#[command(
    name = "g9",
    version,
    about = "G9 MENTOR command-line driver (thin client of g9_app::dispatch)",
    long_about = None,
    propagate_version = true
)]
pub struct Cli {
    /// Human-readable output instead of JSON.
    #[arg(long, global = true)]
    pub human: bool,

    /// Data directory (default: $XDG_DATA_HOME/g9-mentor or ~/.local/share/g9-mentor).
    #[arg(long, global = true, value_name = "DIR")]
    pub data_dir: Option<PathBuf>,

    /// Skill packages directory (default: <repo>/.agents/skills when it exists).
    #[arg(long, global = true, value_name = "DIR", conflicts_with = "no_skills")]
    pub skills_dir: Option<PathBuf>,

    /// Do not load any skill package (skills.invoke answers RESOURCE_UNAVAILABLE).
    #[arg(long, global = true)]
    pub no_skills: bool,

    /// Monday of program week 1 (needed by curriculum.daily_plan).
    #[arg(long, global = true, value_name = "YYYY-MM-DD")]
    pub program_start: Option<String>,

    /// Operator IANA time zone (default: host zone, else UTC; simulate-week: Europe/Berlin).
    #[arg(long, global = true, value_name = "IANA")]
    pub zone: Option<String>,

    /// Backend-assigned operator actor id (the Tauri bridge uses the same default).
    #[arg(long, global = true, value_name = "ID", default_value = crate::bootstrap::OPERATOR_ID)]
    pub operator_id: String,

    #[command(subcommand)]
    pub command: Command,
}

/// Subcommands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create/open the data directory, run recovery, print the RecoveryReport and system.status.
    Init,
    /// system.status.
    Status,
    /// List every operation with action, resource kind, mutating and approval-expected flags.
    Ops {
        /// Also print the request/response schema paths.
        #[arg(long)]
        schemas: bool,
    },
    /// Dispatch one operation with a JSON payload.
    Call(CallArgs),
    /// Approve a pending approval request (approvals.decide APPROVE).
    Approve {
        /// Approval id.
        approval_id: String,
    },
    /// Reject a pending approval request (approvals.decide REJECT).
    Reject {
        /// Approval id.
        approval_id: String,
        /// Operator note; echoed in the output only (the IPC contract carries no reason: PROPOSED).
        #[arg(long)]
        reason: Option<String>,
    },
    /// List approval requests (approvals.list).
    Approvals {
        /// Filter by state (PENDING, APPROVED, REJECTED, EXPIRED, CONSUMED, REVOKED).
        #[arg(long, value_name = "STATE")]
        state: Option<String>,
    },
    /// curriculum.validate then curriculum.import of the seed manifest as PROPOSED_SEED.
    ImportSeed {
        /// Manifest path (default: data/curriculum/g9-ct9-480.seed.v0.json).
        #[arg(long, value_name = "PATH")]
        manifest: Option<PathBuf>,
    },
    /// Deterministic scripted week (Stage 03 exit criterion): sessions, journal, Saturday assessment, backups, audit. Exit 0 only on PASS (INCOMPLETE/FAIL exit 2). Writes seed rows straight to storage: requires --scratch.
    SimulateWeek(SimulateArgs),
    /// Prepare and start a session, then exit without completing it (simulated crash); run `g9 init` afterwards. Writes seed rows straight to storage: requires --scratch.
    CrashDrill(CrashDrillArgs),
    /// Offline restore: replace the live database with a verified backup snapshot; the old database is moved to <data dir>/quarantine, never deleted. Stop the application first. Prints the plan and exits 1 unless --confirm-replace-live-database is given.
    Restore(RestoreArgs),
    /// Copy a RETAINED backup to another directory (a USB drive, a NAS mount): the snapshot plus <id>.manifest.json, synced and re-hashed against the manifest. Reads only; the application may be running.
    CopyBackup(CopyBackupArgs),
}

/// `g9 copy-backup` arguments.
#[derive(Debug, Args)]
pub struct CopyBackupArgs {
    /// Backup id (a RETAINED backup of this installation).
    #[arg(long, value_name = "ID")]
    pub backup_id: String,
    /// Destination directory; it must exist (for example a folder on a mounted USB drive).
    #[arg(long, value_name = "DIR")]
    pub to: PathBuf,
}

/// `g9 call` arguments.
#[derive(Debug, Args)]
pub struct CallArgs {
    /// Operation name, e.g. session.start.
    pub op: String,
    /// Inline JSON payload (default: {}).
    #[arg(long, value_name = "JSON", conflicts_with = "payload_file")]
    pub payload: Option<String>,
    /// Read the JSON payload from a file.
    #[arg(long, value_name = "PATH")]
    pub payload_file: Option<PathBuf>,
    /// Present an approval id (approval-gated retry).
    #[arg(long, value_name = "ID")]
    pub approval: Option<String>,
    /// Optimistic-concurrency revision.
    #[arg(long, value_name = "N")]
    pub expected_revision: Option<u64>,
    /// Interaction mode (default: the operation's UI default).
    #[arg(long, value_enum)]
    pub mode: Option<ModeArg>,
    /// Privacy domain (default: the operation's UI default).
    #[arg(long, value_enum)]
    pub domain: Option<DomainArg>,
    /// Idempotency key (reuse to replay).
    #[arg(long, value_name = "K")]
    pub idempotency_key: Option<String>,
    /// Trace id.
    #[arg(long, value_name = "T")]
    pub trace_id: Option<String>,
}

/// Why `simulate-week` and `crash-drill` need `--scratch`: they bypass the
/// command pipeline.
pub const SCRATCH_REFUSAL: &str = "this command writes seed rows directly to storage, bypassing the command pipeline, and must only be pointed at a throwaway data directory; pass --scratch to confirm that --data-dir is disposable (it does not inspect the directory)";

/// `g9 crash-drill` arguments.
#[derive(Debug, Args)]
pub struct CrashDrillArgs {
    /// Acknowledge that `--data-dir` is a disposable directory: the drill writes
    /// seed rows directly to storage.
    #[arg(long)]
    pub scratch: bool,
}

/// `g9 restore` arguments.
#[derive(Debug, Args)]
#[command(group(ArgGroup::new("source").required(true).multiple(false).args(["backup_id", "snapshot"])))]
pub struct RestoreArgs {
    /// Restore the snapshot recorded for this backup id in the live database (the
    /// manifest must be RETAINED and the snapshot inside <data dir>/backups).
    #[arg(long, value_name = "ID")]
    pub backup_id: Option<String>,
    /// Restore this snapshot file; --expect-sha256 is then mandatory.
    #[arg(long, value_name = "PATH", requires = "expect_sha256")]
    pub snapshot: Option<PathBuf>,
    /// Hex SHA-256 the snapshot must have. Mandatory with --snapshot; with
    /// --backup-id it must equal the manifest's checksum.
    #[arg(long, value_name = "HEX")]
    pub expect_sha256: Option<String>,
    /// Actually replace the live database. Without it the plan is printed and
    /// nothing is changed (exit 1).
    #[arg(long)]
    pub confirm_replace_live_database: bool,
    /// Folder to copy the restored database's attachment files from (default:
    /// the `attachments` folder beside the snapshot).
    #[arg(long, value_name = "DIR")]
    pub attachments: Option<PathBuf>,
}

/// `g9 simulate-week` arguments.
#[derive(Debug, Args)]
pub struct SimulateArgs {
    /// Monday of the simulated week (YYYY-MM-DD).
    #[arg(long, value_name = "YYYY-MM-DD")]
    pub start_date: String,
    /// Program week number of that Monday (program start = start date − 7·(week−1) days).
    #[arg(long, default_value_t = 1)]
    pub week: u32,
    /// Fixed clock origin (epoch ms); makes the run fully deterministic (sequential ids).
    #[arg(long, value_name = "EPOCH_MS")]
    pub fixed_clock: Option<i64>,
    /// Seed manifest imported when no curriculum version exists.
    #[arg(long, value_name = "PATH")]
    pub manifest: Option<PathBuf>,
    /// Acknowledge that `--data-dir` is a disposable directory: the week writes
    /// seed rows directly to storage.
    #[arg(long)]
    pub scratch: bool,
}

/// Interaction mode (wire spelling).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModeArg {
    Study,
    Lab,
    Exam,
    Review,
    Executive,
    Research,
    Maintenance,
}

impl ModeArg {
    /// Wire spelling.
    pub const fn as_wire(self) -> &'static str {
        match self {
            ModeArg::Study => "STUDY",
            ModeArg::Lab => "LAB",
            ModeArg::Exam => "EXAM",
            ModeArg::Review => "REVIEW",
            ModeArg::Executive => "EXECUTIVE",
            ModeArg::Research => "RESEARCH",
            ModeArg::Maintenance => "MAINTENANCE",
        }
    }
}

/// Privacy domain (wire spelling).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DomainArg {
    Learning,
    Business,
    Personal,
    SecuritySensitive,
    System,
}

impl DomainArg {
    /// Wire spelling.
    pub const fn as_wire(self) -> &'static str {
        match self {
            DomainArg::Learning => "LEARNING",
            DomainArg::Business => "BUSINESS",
            DomainArg::Personal => "PERSONAL",
            DomainArg::SecuritySensitive => "SECURITY_SENSITIVE",
            DomainArg::System => "SYSTEM",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap()
    }

    #[test]
    fn global_options_work_before_and_after_the_subcommand() {
        let a = parse(&["g9", "--data-dir", "/tmp/x", "init"]);
        let b = parse(&["g9", "init", "--data-dir", "/tmp/x"]);
        assert_eq!(a.data_dir, b.data_dir);
        assert_eq!(a.data_dir.as_deref(), Some(std::path::Path::new("/tmp/x")));
        assert!(matches!(a.command, Command::Init));
        assert_eq!(a.operator_id, crate::bootstrap::OPERATOR_ID);
        assert!(!a.human);
    }

    #[test]
    fn call_parses_payload_mode_domain_and_approval() {
        let c = parse(&[
            "g9",
            "call",
            "skills.invoke",
            "--payload",
            "{\"skill_code\":\"x\",\"input\":{}}",
            "--mode",
            "EXAM",
            "--domain",
            "SECURITY_SENSITIVE",
            "--approval",
            "apr-1",
            "--expected-revision",
            "4",
            "--idempotency-key",
            "k",
            "--trace-id",
            "t",
            "--human",
        ]);
        assert!(c.human);
        match c.command {
            Command::Call(args) => {
                assert_eq!(args.op, "skills.invoke");
                assert_eq!(args.mode.map(ModeArg::as_wire), Some("EXAM"));
                assert_eq!(
                    args.domain.map(DomainArg::as_wire),
                    Some("SECURITY_SENSITIVE")
                );
                assert_eq!(args.approval.as_deref(), Some("apr-1"));
                assert_eq!(args.expected_revision, Some(4));
                assert_eq!(args.idempotency_key.as_deref(), Some("k"));
                assert_eq!(args.trace_id.as_deref(), Some("t"));
                assert!(args.payload.is_some());
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn payload_and_payload_file_are_exclusive_and_modes_are_validated() {
        assert!(Cli::try_parse_from([
            "g9",
            "call",
            "x.y",
            "--payload",
            "{}",
            "--payload-file",
            "p.json"
        ])
        .is_err());
        assert!(Cli::try_parse_from(["g9", "call", "x.y", "--mode", "exam"]).is_err());
        assert!(Cli::try_parse_from(["g9", "--skills-dir", "s", "--no-skills", "status"]).is_err());
        assert!(Cli::try_parse_from(["g9"]).is_err());
    }

    #[test]
    fn simulate_week_defaults() {
        let c = parse(&["g9", "simulate-week", "--start-date", "2026-11-16"]);
        match c.command {
            Command::SimulateWeek(a) => {
                assert_eq!(a.start_date, "2026-11-16");
                assert_eq!(a.week, 1);
                assert_eq!(a.fixed_clock, None);
                assert!(a.manifest.is_none());
                assert!(!a.scratch);
            }
            other => panic!("unexpected {other:?}"),
        }
        let c = parse(&[
            "g9",
            "simulate-week",
            "--start-date",
            "2026-11-16",
            "--week",
            "3",
            "--fixed-clock",
            "1794816000000",
            "--zone",
            "Europe/Berlin",
        ]);
        assert_eq!(c.zone.as_deref(), Some("Europe/Berlin"));
        match c.command {
            Command::SimulateWeek(a) => {
                assert_eq!(a.week, 3);
                assert_eq!(a.fixed_clock, Some(1_794_816_000_000));
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn other_subcommands_parse() {
        assert!(matches!(parse(&["g9", "status"]).command, Command::Status));
        assert!(matches!(
            parse(&["g9", "ops", "--schemas"]).command,
            Command::Ops { schemas: true }
        ));
        assert!(matches!(
            parse(&["g9", "approve", "apr-1"]).command,
            Command::Approve { .. }
        ));
        match parse(&["g9", "reject", "apr-1", "--reason", "no"]).command {
            Command::Reject {
                approval_id,
                reason,
            } => {
                assert_eq!(approval_id, "apr-1");
                assert_eq!(reason.as_deref(), Some("no"));
            }
            other => panic!("unexpected {other:?}"),
        }
        match parse(&["g9", "approvals", "--state", "PENDING"]).command {
            Command::Approvals { state } => assert_eq!(state.as_deref(), Some("PENDING")),
            other => panic!("unexpected {other:?}"),
        }
        assert!(matches!(
            parse(&["g9", "import-seed"]).command,
            Command::ImportSeed { manifest: None }
        ));
        match parse(&["g9", "crash-drill", "--data-dir", "d"]).command {
            Command::CrashDrill(args) => assert!(!args.scratch),
            other => panic!("unexpected {other:?}"),
        }
        match parse(&["g9", "crash-drill", "--scratch"]).command {
            Command::CrashDrill(args) => assert!(args.scratch),
            other => panic!("unexpected {other:?}"),
        }
        match parse(&[
            "g9",
            "simulate-week",
            "--start-date",
            "2026-11-16",
            "--scratch",
        ])
        .command
        {
            Command::SimulateWeek(args) => assert!(args.scratch),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn restore_takes_exactly_one_source_and_a_checksum_with_a_snapshot() {
        match parse(&["g9", "restore", "--backup-id", "b1"]).command {
            Command::Restore(a) => {
                assert_eq!(a.backup_id.as_deref(), Some("b1"));
                assert!(a.snapshot.is_none() && a.expect_sha256.is_none());
                assert!(!a.confirm_replace_live_database);
            }
            other => panic!("unexpected {other:?}"),
        }
        let hex = "ab".repeat(32);
        match parse(&[
            "g9",
            "restore",
            "--snapshot",
            "s.sqlite",
            "--expect-sha256",
            &hex,
            "--data-dir",
            "d",
            "--confirm-replace-live-database",
        ])
        .command
        {
            Command::Restore(a) => {
                assert_eq!(
                    a.snapshot.as_deref(),
                    Some(std::path::Path::new("s.sqlite"))
                );
                assert_eq!(a.expect_sha256.as_deref(), Some(hex.as_str()));
                assert!(a.confirm_replace_live_database);
            }
            other => panic!("unexpected {other:?}"),
        }
        // No source, two sources, and a snapshot without a checksum are usage errors.
        assert!(Cli::try_parse_from(["g9", "restore"]).is_err());
        assert!(Cli::try_parse_from([
            "g9",
            "restore",
            "--backup-id",
            "b1",
            "--snapshot",
            "s.sqlite",
            "--expect-sha256",
            &hex
        ])
        .is_err());
        assert!(Cli::try_parse_from(["g9", "restore", "--snapshot", "s.sqlite"]).is_err());
        assert!(Cli::try_parse_from(["g9", "restore", "--expect-sha256", &hex]).is_err());
    }
}
