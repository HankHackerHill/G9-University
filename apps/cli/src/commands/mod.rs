//! One function per subcommand. [`run`] turns the parsed [`Cli`] into a
//! [`Completion`] or a [`CliError`]; `main` only renders and exits.

pub mod approvals;
pub mod call;
pub mod crash_drill;
pub mod import_seed;
pub mod init;
pub mod ops;
pub mod simulate;
pub mod status;

use crate::bootstrap::{self, BootOptions};
use crate::cli::{Cli, Command};
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
        Command::SimulateWeek(args) => {
            let options = simulate::SimulateOptions::from_cli(cli, args)?;
            Ok(Completion::of(simulate::run(options)?))
        }
        Command::CrashDrill => crash_drill::run(&boot_options(cli)?),
    }
}
