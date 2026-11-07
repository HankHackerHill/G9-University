//! `g9` binary: parse, run, render, exit (0 ok / 2 refused or FAIL / 1 usage).

use clap::error::ErrorKind;
use clap::Parser;
use g9_cli::output::{render_error, render_value, EXIT_OK, EXIT_USAGE};
use g9_cli::{Cli, Completion};
use std::io::Write;

fn main() {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let code = match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => EXIT_OK,
                _ => EXIT_USAGE,
            };
            // clap already formats help/usage; the stream choice is its own.
            let _ = e.print();
            std::process::exit(code);
        }
    };
    let human = cli.human;
    match g9_cli::commands::run(&cli) {
        Ok(Completion { value, abandon }) => {
            let mut stdout = std::io::stdout().lock();
            let _ = writeln!(stdout, "{}", render_value(&value, human));
            let _ = stdout.flush();
            if let Some(client) = abandon {
                // Simulated crash: keep the database connection open and skip
                // every destructor on the way out.
                std::mem::forget(client);
                std::process::exit(EXIT_OK);
            }
        }
        Err(error) => {
            let rendered = render_error(&error, human);
            if let Some(text) = rendered.stdout {
                let mut stdout = std::io::stdout().lock();
                let _ = writeln!(stdout, "{text}");
                let _ = stdout.flush();
            }
            if let Some(text) = rendered.stderr {
                let mut stderr = std::io::stderr().lock();
                let _ = writeln!(stderr, "{text}");
                let _ = stderr.flush();
            }
            std::process::exit(rendered.code);
        }
    }
}
