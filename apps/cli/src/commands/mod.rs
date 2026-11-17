//! One function per subcommand. [`run`] turns the parsed [`Cli`] into a
//! [`Completion`] or a [`CliError`]; `main` only renders and exits.

pub mod approvals;
pub mod call;
pub mod copy_backup;
pub mod crash_drill;
pub mod import_seed;
pub mod init;
pub mod ops;
pub mod reconcile;
pub mod restore;
pub mod simulate;
pub mod status;

use crate::bootstrap::{self, BootOptions};
use crate::cli::{Cli, Command, SCRATCH_REFUSAL};
use crate::output::{CliError, Completion};
use crate::seed;
use g9_domain::IanaZone;

/// Boot options from the global flags (host clock, host zone unless `--zone`).
pub fn boot_options(cli: &Cli) -> Result<BootOptions, CliError> {
    let data_dir = match &cli.data_dir {
        Some(dir) => dir.clone(),
        None => bootstrap::default_data_dir()?,
    };
    let mut opts = BootOptions::new(data_dir);
    opts.operator_id = cli.operator_id.clone();
    opts.event_log = true;
    if cli.no_skills {
        opts.skills_dir = None;
    } else if let Some(dir) = &cli.skills_dir {
        opts.skills_dir = Some(dir.clone());
    }
    if let Some(zone) = &cli.zone {
        opts.zone = Some(parse_zone(zone)?);
    }
    if let Some(date) = &cli.program_start {
        let parsed = seed::parse_date(date).map_err(|_| {
            CliError::Usage(format!("--program-start {date:?}: dates are YYYY-MM-DD"))
        })?;
        if parsed.iso_weekday() != 1 {
            return Err(CliError::Usage(format!(
                "--program-start {date} is not a Monday (program week 1 starts on a Monday)"
            )));
        }
        opts.program_start = Some(parsed);
    }
    Ok(opts)
}

/// Parse an IANA zone name as a usage error.
pub fn parse_zone(zone: &str) -> Result<IanaZone, CliError> {
    IanaZone::parse(zone)
        .map_err(|_| CliError::Usage(format!("--zone {zone:?} is not an IANA time zone")))
}

/// The scratch guard (ADR-0010 addendum): `simulate-week`, `crash-drill` and `perf-fixture` write
/// seed rows straight to storage and refuse to run without `--scratch`. The flag
/// is an explicit acknowledgement that the data directory is disposable; the
/// directory is not inspected (PROPOSED: refuse a directory that holds operator
/// data), so the directory must be named: `--scratch` without `--data-dir` would
/// declare the default data directory disposable (security audit 2026-11-13).
/// Checked before anything is opened or created.
pub fn require_scratch(scratch: bool, data_dir_named: bool) -> Result<(), CliError> {
    if !scratch {
        return Err(CliError::Usage(SCRATCH_REFUSAL.to_string()));
    }
    if !data_dir_named {
        return Err(CliError::Usage(SCRATCH_NEEDS_DATA_DIR.to_string()));
    }
    Ok(())
}

/// Why `--scratch` alone is refused.
pub const SCRATCH_NEEDS_DATA_DIR: &str = "--scratch confirms that --data-dir is disposable, so name that directory with --data-dir; the default data directory is never treated as scratch";

/// Run the parsed command line.
pub fn run(cli: &Cli) -> Result<Completion, CliError> {
    match &cli.command {
        Command::Init => Ok(Completion::of(init::run(&boot_options(cli)?)?)),
        Command::Status => Ok(Completion::of(status::run(&boot_options(cli)?)?)),
        Command::Ops { schemas } => Ok(Completion::of(ops::run(*schemas))),
        Command::Call(args) => Ok(Completion::of(call::run(&boot_options(cli)?, args)?)),
        Command::Approve { approval_id } => Ok(Completion::of(approvals::approve(
            &boot_options(cli)?,
            approval_id,
        )?)),
        Command::Reject {
            approval_id,
            reason,
        } => Ok(Completion::of(approvals::reject(
            &boot_options(cli)?,
            approval_id,
            reason.as_deref(),
        )?)),
        Command::Approvals { state } => Ok(Completion::of(approvals::list(
            &boot_options(cli)?,
            state.as_deref(),
        )?)),
        Command::ImportSeed { manifest } => Ok(Completion::of(import_seed::run(
            &boot_options(cli)?,
            manifest.as_deref(),
        )?)),
        Command::Reconcile(args) => Ok(Completion::of(reconcile::run(
            args,
            cli.data_dir.as_deref(),
        )?)),
        Command::SimulateWeek(args) => {
            require_scratch(args.scratch, cli.data_dir.is_some())?;
            let options = simulate::SimulateOptions::from_cli(cli, args)?;
            Ok(Completion::of(simulate::run(options)?))
        }
        Command::CrashDrill(args) => {
            require_scratch(args.scratch, cli.data_dir.is_some())?;
            crash_drill::run(&boot_options(cli)?)
        }
        Command::PerfFixture(args) => {
            require_scratch(args.scratch, cli.data_dir.is_some())?;
            let data_dir = cli
                .data_dir
                .clone()
                .ok_or_else(|| CliError::Usage(SCRATCH_NEEDS_DATA_DIR.to_string()))?;
            let mut options = crate::perf::fixture::FixtureOptions::new(data_dir, args.divisor)?;
            if let Some(manifest) = &args.manifest {
                options.manifest = manifest.clone();
            }
            Ok(Completion::of(crate::perf::fixture::run(&options)?))
        }
        Command::Restore(args) => Ok(Completion::of(restore::run(&boot_options(cli)?, args)?)),
        Command::CopyBackup(args) => {
            Ok(Completion::of(copy_backup::run(&boot_options(cli)?, args)?))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn refused(args: &[&str]) -> String {
        let cli = Cli::try_parse_from(args).unwrap();
        match run(&cli) {
            Err(CliError::Usage(message)) => message,
            Err(other) => panic!("expected a usage error, got {other:?}"),
            Ok(_) => panic!("expected a refusal"),
        }
    }

    #[test]
    fn the_seed_writing_commands_refuse_without_scratch_and_create_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let data_str = data.to_str().unwrap();
        for args in [
            vec!["g9", "crash-drill", "--data-dir", data_str],
            vec![
                "g9",
                "simulate-week",
                "--start-date",
                "2026-11-16",
                "--data-dir",
                data_str,
            ],
            vec!["g9", "perf-fixture", "--data-dir", data_str],
        ] {
            let message = refused(&args);
            assert!(message.contains("--scratch"), "{message}");
            assert!(
                message.contains("seed rows directly to storage"),
                "{message}"
            );
            assert!(message.contains("throwaway data directory"), "{message}");
        }
        assert!(
            !data.exists(),
            "a refused command must not create the data directory"
        );
        assert!(require_scratch(true, true).is_ok());
        // `--scratch` alone would declare the default data directory disposable.
        for args in [
            vec!["g9", "crash-drill", "--scratch"],
            vec![
                "g9",
                "simulate-week",
                "--scratch",
                "--start-date",
                "2026-11-16",
            ],
            vec!["g9", "perf-fixture", "--scratch"],
        ] {
            let message = refused(&args);
            assert!(
                message.contains("name that directory with --data-dir"),
                "{message}"
            );
        }
    }
}
