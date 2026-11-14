#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! No source file holds a character that cannot be seen or that reorders the text around it.
//!
//! "Trojan Source" (CVE-2021-42574): a bidirectional override or an invisible character in a
//! source file makes the code a reviewer reads differ from the code the compiler reads. The
//! same characters are refused in names by `g9_domain::text` (security audit 2026-11-13);
//! this test applies the same definition, [`g9_domain::text::is_hidden`], to every text file of
//! the repository. It found two such files during that audit: both were written by a tool that
//! decoded a `\uXXXX` escape into the character it names.
//!
//! Write such a character as an escape (`\u{202E}` in Rust, `\u202E` in TypeScript), never raw.
//! A file that needs one raw (a test fixture) is listed in [`ALLOWED`] with the exact count.

use g9_domain::text::is_hidden;
use std::path::{Path, PathBuf};

/// `(repository-relative path, character, how many times it may occur)`.
const ALLOWED: &[(&str, char, usize)] = &[
    // A UTF-8 byte order mark in front of a JSON attachment: the mock must refuse it as a
    // content mismatch, exactly as the backend does.
    (
        "apps/desktop/src/ipc/transports/mock.test.ts",
        '\u{FEFF}',
        1,
    ),
];

const SKIPPED_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    "__pycache__",
    ".venv",
];

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn text_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !SKIPPED_DIRS.contains(&name.as_str()) {
                text_files(&path, out);
            }
        } else if kind.is_file() {
            out.push(path);
        }
    }
}

/// Every hidden character of `text` as `(line, column, character)`.
fn hidden_in(text: &str) -> Vec<(usize, usize, char)> {
    let mut found = Vec::new();
    for (n, line) in text.lines().enumerate() {
        for (i, c) in line.chars().enumerate() {
            // Tabs and carriage returns are whitespace of the file format, not hidden text.
            if is_hidden(c) && c != '\t' && c != '\r' {
                found.push((n + 1, i + 1, c));
            }
        }
    }
    found
}

#[test]
fn the_scan_finds_what_it_is_for() {
    let sample = "let a = 1;\nlet b = \u{202E}2;\nok\t\n\u{200B}\u{E0041}x\n";
    assert_eq!(
        hidden_in(sample),
        vec![(2, 9, '\u{202E}'), (4, 1, '\u{200B}'), (4, 2, '\u{E0041}')]
    );
    assert!(hidden_in("plain text, \u{00fc}ber, \u{5831}\u{544a}, tabs\tand CR\r\n").is_empty());
}

#[test]
fn no_text_file_of_the_repository_holds_a_hidden_character() {
    let root = repository_root();
    let mut files = Vec::new();
    text_files(&root, &mut files);
    assert!(
        files.len() > 500,
        "the scan found only {} files under {}",
        files.len(),
        root.display()
    );
    let mut offenders = Vec::new();
    let mut allowed_seen = vec![0_usize; ALLOWED.len()];
    for file in &files {
        // Binary files (images, fonts, archives) are not text; skip what is not UTF-8.
        let Ok(bytes) = std::fs::read(file) else {
            continue;
        };
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
        let relative = file
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        for (line, column, c) in hidden_in(&text) {
            match ALLOWED
                .iter()
                .position(|(path, allowed, _)| *path == relative && *allowed == c)
            {
                Some(i) => allowed_seen[i] += 1,
                None => offenders.push(format!(
                    "{relative}:{line}:{column}: U+{:04X}",
                    u32::from(c)
                )),
            }
        }
    }
    assert!(
        offenders.is_empty(),
        "hidden characters in source (write them as escapes):\n{}",
        offenders.join("\n")
    );
    for (i, (path, c, count)) in ALLOWED.iter().enumerate() {
        assert_eq!(
            allowed_seen[i],
            *count,
            "{path}: expected exactly {count} U+{:04X}; update ALLOWED",
            u32::from(*c)
        );
    }
}
