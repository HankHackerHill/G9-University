//! Write the upgrade fixtures that do not exist yet (REL-5): one database per
//! schema version `1..=CURRENT_SCHEMA_VERSION` in the directory given as the
//! only argument. An existing fixture is kept as it is: it stands for what an
//! older build wrote (see `tests/upgrade_fixtures/generate.rs`).
//!
//! ```text
//! cargo run -p g9-app --example write_upgrade_fixtures -- crates/g9-app/tests/upgrade_fixtures
//! ```

#[path = "../tests/upgrade_fixtures/generate.rs"]
mod generate;

use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let (Some(dir), None) = (args.next(), args.next()) else {
        eprintln!("usage: write_upgrade_fixtures <directory>");
        return ExitCode::from(2);
    };
    let dir = PathBuf::from(dir);
    if !dir.is_dir() {
        eprintln!("{} is not a directory", dir.display());
        return ExitCode::from(2);
    }
    for version in 1..=g9_storage::CURRENT_SCHEMA_VERSION {
        let path = dir.join(generate::file_name(version));
        if path.exists() {
            println!("kept    {}", path.display());
            continue;
        }
        // Written beside its final name and renamed only when complete.
        let partial = dir.join(format!(".{}.partial", generate::file_name(version)));
        let _ = std::fs::remove_file(&partial);
        if let Err(e) = generate::write_fixture(&partial, version)
            .and_then(|()| std::fs::rename(&partial, &path).map_err(|e| e.to_string()))
        {
            let _ = std::fs::remove_file(&partial);
            eprintln!("schema {version}: {e}");
            return ExitCode::FAILURE;
        }
        println!("written {}", path.display());
    }
    ExitCode::SUCCESS
}
