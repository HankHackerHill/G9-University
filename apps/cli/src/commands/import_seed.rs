//! `g9 import-seed`: `curriculum.validate`, then `curriculum.import` as
//! `PROPOSED_SEED` through the real approval round trip (the first import
//! answers `APPROVAL_REQUIRED`, the operator approves, the retry presents the
//! approval). A manifest that fails validation is refused with the validation
//! report (exit 2). An already imported version is reported, not an error.

use crate::bootstrap::{self, BootOptions};
use crate::client::Client;
use crate::meta::MetaOptions;
use crate::output::CliError;
use g9_domain::{ErrorCategory, G9Error};
use serde_json::{json, Value};
use std::path::Path;

/// Load and parse the manifest file.
pub fn load_manifest(path: &Path) -> Result<Value, CliError> {
    let text = std::fs::read_to_string(path).map_err(|e| CliError::io("manifest", &e))?;
    serde_json::from_str(&text).map_err(|e| {
        CliError::Usage(format!(
            "manifest is not valid JSON (line {}, column {})",
            e.line(),
            e.column()
        ))
    })
}

/// Observer of every dispatch made by [`import`] (`(operation, outcome)`).
pub type Observer<'a> = &'a mut dyn FnMut(&str, &Result<Value, G9Error>);

fn count<'a>(manifest: &'a Value, path: &[&str]) -> impl Iterator<Item = &'a Value> + 'a {
    let mut node = manifest;
    for key in path {
        node = node.get(key).unwrap_or(&Value::Null);
    }
    node.as_array().into_iter().flatten()
}

/// Weeks / days / objectives counted from the manifest itself (used only when
/// the version already exists and the backend reports no counts).
fn manifest_counts(manifest: &Value) -> (usize, usize, usize) {
    let weeks: Vec<&Value> = count(manifest, &["weeks"]).collect();
    let days: Vec<&Value> = weeks.iter().flat_map(|w| count(w, &["days"])).collect();
    let objectives = days.iter().flat_map(|d| count(d, &["objectives"])).count();
    (weeks.len(), days.len(), objectives)
}

/// Validate then import `manifest` as `PROPOSED_SEED`.
pub fn import(
    client: &mut Client,
    manifest: &Value,
    observe: Observer<'_>,
) -> Result<Value, CliError> {
    let validated = client.call("curriculum.validate", json!({ "manifest": manifest }));
    observe("curriculum.validate", &validated);
    let validated = validated?;
    let report = &validated["result"];
    if report["valid"] != json!(true) {
        return Err(CliError::Backend(
            G9Error::invalid_input(
                "curriculum.invariant_violated",
                "the manifest violates the curriculum invariants or the manifest contract; import refused",
            )
            .with_details(json!({
                "violations": report["violations"],
                "checksum_sha256": report["checksum_sha256"],
            })),
        ));
    }
    let checksum = report["checksum_sha256"].clone();
    let payload = json!({ "manifest": manifest, "provenance": "PROPOSED_SEED" });
    let first = client.call_with(
        "curriculum.import",
        payload.clone(),
        &MetaOptions::default(),
    );
    observe("curriculum.import", &first);
    let (outcome, approval_id) = match first {
        Ok(out) => (out, None),
        Err(e)
            if e.category == ErrorCategory::Conflict && e.code == "curriculum.version_exists" =>
        {
            let (weeks, days, objectives) = manifest_counts(manifest);
            let version_id = manifest
                .get("curriculum_id")
                .and_then(Value::as_str)
                .zip(manifest.get("version").and_then(Value::as_str))
                .map(|(c, v)| format!("{c}:{v}"));
            return Ok(json!({
                "already_imported": true,
                "curriculum_version_id": version_id,
                "checksum_sha256": checksum,
                "provenance": manifest.get("provenance").cloned().unwrap_or(Value::Null),
                "weeks": weeks,
                "days": days,
                "objectives": objectives,
                "approval_id": Value::Null,
            }));
        }
        Err(e) if e.category == ErrorCategory::ApprovalRequired => {
            let approval = e
                .approval_id
                .as_ref()
                .map(|a| a.as_str().to_string())
                .ok_or_else(|| G9Error::internal_redacted("cli.approval_id_missing"))?;
            let decided = client.approve(&approval);
            observe("approvals.decide", &decided);
            decided?;
            let retry = client.call_with(
                "curriculum.import",
                payload,
                &MetaOptions::approval(approval.clone()),
            );
            observe("curriculum.import", &retry);
            (retry?, Some(approval))
        }
        Err(e) => return Err(e.into()),
    };
    let result = &outcome["result"];
    Ok(json!({
        "already_imported": false,
        "curriculum_version_id": result["curriculum_version_id"],
        "checksum_sha256": result["checksum_sha256"],
        "provenance": result["provenance"],
        "weeks": result["weeks"],
        "days": result["days"],
        "objectives": result["objectives"],
        "approval_id": approval_id,
        "revision": outcome["revision"],
        "audit_ref": outcome["audit_ref"],
    }))
}

/// Run `import-seed`.
pub fn run(boot: &BootOptions, manifest_path: Option<&Path>) -> Result<Value, CliError> {
    let path = manifest_path
        .map(Path::to_path_buf)
        .unwrap_or_else(bootstrap::default_seed_manifest);
    let manifest = load_manifest(&path)?;
    let booted = bootstrap::open(boot)?;
    let mut client = Client::new(booted.ctx);
    let mut noop = |_: &str, _: &Result<Value, G9Error>| {};
    let mut value = import(&mut client, &manifest, &mut noop)?;
    value["manifest"] = json!(path);
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_counts_follow_the_structure() {
        let m = json!({
            "weeks": [
                { "days": [ { "objectives": [1, 2, 3] }, { "objectives": [1] } ] },
                { "days": [ { "objectives": [] } ] }
            ]
        });
        assert_eq!(manifest_counts(&m), (2, 3, 4));
        assert_eq!(manifest_counts(&json!({})), (0, 0, 0));
    }

    #[test]
    fn seed_manifest_in_repository_parses() {
        let m = load_manifest(&bootstrap::default_seed_manifest()).unwrap();
        assert_eq!(manifest_counts(&m), (12, 72, 216));
        assert_eq!(m["provenance"], "PROPOSED_SEED");
    }
}
