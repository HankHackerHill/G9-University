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
    // Tool caches that are not UTF-8 text and that git ignores (ruff writes its own .gitignore in it).
    ".ruff_cache",
];

/// Repository-relative directories skipped as a whole: agent sessions' temporary git
/// worktrees are copies, not part of the repository. `.claude` itself is scanned: its project
/// skills steer an AI agent.
const SKIPPED_PATHS: &[&str] = &[".claude/worktrees"];

/// Files that are binary by design (the Tauri icons and the SQLite upgrade fixtures). Any other
/// file that is not valid UTF-8 is a failure: a file with one invalid byte used to be skipped
/// whole, so hidden characters elsewhere in it went unseen (audit 2026-11-19, GATE-1).
const BINARY_EXTENSIONS: &[&str] = &["png", "sqlite"];

/// Why a file that is not valid UTF-8 cannot be skipped, or `None` when it may be.
fn not_text_defect(relative: &str, bytes: &[u8]) -> Option<String> {
    if std::str::from_utf8(bytes).is_ok() {
        return None;
    }
    let extension = relative.rsplit('.').next().unwrap_or("").to_lowercase();
    if BINARY_EXTENSIONS.contains(&extension.as_str()) {
        None
    } else {
        Some(format!(
            "{relative}: not valid UTF-8 and not one of the binary types {BINARY_EXTENSIONS:?}"
        ))
    }
}

fn repository_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Every regular file under `dir`, except the skipped directories; symbolic links are not
/// followed and are collected in `links`: a link outside the skipped directories is a failure.
fn text_files(root: &Path, dir: &Path, out: &mut Vec<PathBuf>, links: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_symlink() {
            links.push(path);
            continue;
        }
        if kind.is_dir() {
            let name = entry.file_name().to_string_lossy().to_string();
            let relative = path
                .strip_prefix(root)
                .map(|r| r.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            if !SKIPPED_DIRS.contains(&name.as_str()) && !SKIPPED_PATHS.contains(&relative.as_str())
            {
                text_files(root, &path, out, links);
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
fn a_file_that_is_not_utf8_or_a_link_can_no_longer_slip_past_the_scan() {
    // One invalid byte next to a right-to-left override: the old scan skipped the whole file.
    let source = b"let a = 1; // \xE2\x80\xAE\xFF\n";
    assert!(not_text_defect("src/a.ts", source).is_some());
    assert!(not_text_defect("scripts/run.sh", source).is_some());
    // The known binary types and valid UTF-8 pass.
    assert!(not_text_defect("icons/icon.png", b"\x89PNG\xFF").is_none());
    assert!(not_text_defect("fixtures/schema-v01.sqlite", b"SQLite\xFF").is_none());
    assert!(not_text_defect("docs/a.md", "plain \u{fc}ber".as_bytes()).is_none());

    // A symbolic link is reported, not followed.
    #[cfg(unix)]
    {
        let dir = std::env::temp_dir().join(format!("g9-scan-links-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("real.txt"), "x").unwrap();
        let _ = std::fs::remove_file(dir.join("link.txt"));
        std::os::unix::fs::symlink(dir.join("real.txt"), dir.join("link.txt")).unwrap();
        let (mut files, mut links) = (Vec::new(), Vec::new());
        text_files(&dir, &dir, &mut files, &mut links);
        assert_eq!(files, vec![dir.join("real.txt")]);
        assert_eq!(links, vec![dir.join("link.txt")]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

#[test]
fn no_text_file_of_the_repository_holds_a_hidden_character() {
    let root = repository_root();
    let mut files = Vec::new();
    let mut links = Vec::new();
    text_files(&root, &root, &mut files, &mut links);
    assert!(
        links.is_empty(),
        "symbolic links outside the skipped directories (the scan does not follow them): {links:?}"
    );
    assert!(
        files.len() > 500,
        "the scan found only {} files under {}",
        files.len(),
        root.display()
    );
    // Files that steer an AI agent (project skills, `.claude/`) are the most valuable place to
    // hide an instruction in invisible characters: the scan must reach them (audit 2026-11-19;
    // `.claude` used to be skipped by name).
    for required in [".claude/skills", ".agents/skills"] {
        assert!(
            files
                .iter()
                .any(|f| f.strip_prefix(&root).is_ok_and(|r| r.starts_with(required))),
            "the scan does not reach any file under {required}"
        );
    }
    let mut offenders = Vec::new();
    let mut allowed_seen = vec![0_usize; ALLOWED.len()];
    for file in &files {
        let Ok(bytes) = std::fs::read(file) else {
            continue;
        };
        let relative = file
            .strip_prefix(&root)
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/");
        // Binary files are listed by type; a text-looking file that is not UTF-8 is a failure.
        if let Some(defect) = not_text_defect(&relative, &bytes) {
            offenders.push(defect);
            continue;
        }
        let Ok(text) = String::from_utf8(bytes) else {
            continue;
        };
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
