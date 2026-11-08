//! Shared helpers of the CLI integration tests: the `g9` binary runner and the
//! key-material scan used to prove that no output carries the binding key.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::Value;
use std::path::Path;
use std::process::Command;

/// Standard base64 (with padding) of `bytes`; no crate so the test has no new
/// dependency. Used to prove the key bytes appear in no output in any common
/// text encoding.
pub fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                let idx = usize::try_from((n >> (18 - 6 * i)) & 63).unwrap();
                out.push(char::from(TABLE[idx]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Every text encoding in which the 32 key bytes could leak into an output.
pub fn key_encodings(key_file: &Path) -> Vec<String> {
    let bytes = std::fs::read(key_file).unwrap();
    assert_eq!(bytes.len(), 32, "binding key file is 32 bytes");
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let b64 = base64(&bytes);
    vec![
        hex.clone(),
        hex.to_uppercase(),
        b64.trim_end_matches('=').to_string(),
        b64.replace('+', "-")
            .replace('/', "_")
            .trim_end_matches('=')
            .to_string(),
        format!("{bytes:?}"),
    ]
}

pub fn assert_no_key_material(key_file: &Path, outputs: &[&str]) {
    for encoding in key_encodings(key_file) {
        for out in outputs {
            assert!(
                !out.contains(&encoding),
                "binding key material ({} chars) found in an output",
                encoding.len()
            );
        }
    }
}

pub fn g9(args: &[&str]) -> (i32, Value, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_g9"))
        .args(args)
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let value = serde_json::from_str(&stdout).unwrap_or(Value::Null);
    (out.status.code().unwrap_or(-1), value, stderr)
}
