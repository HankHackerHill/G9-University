//! `g9 ops`: the operation registry the backend enforces
//! (`g9_app::ops::OPERATIONS`) with the client defaults the UI uses, plus the
//! later-stage operation names that answer `PRECONDITION_FAILED
//! <area>.not_implemented`. Needs no data directory.

use crate::meta::op_defaults;
use serde_json::{json, Value};

/// Run `ops`.
pub fn run(schemas: bool) -> Value {
    let operations: Vec<Value> = g9_app::ops::OPERATIONS
        .iter()
        .map(|spec| {
            let defaults = op_defaults(spec.name);
            let mut v = json!({
                "name": spec.name,
                "action": spec.action,
                "resource_kind": spec.resource_kind,
                "mutating": spec.mutating,
                "approval_expected": defaults.approval_expected,
                "default_scope": { "domain": defaults.domain, "mode": defaults.mode },
                "schema_version": spec.schema_version,
            });
            if schemas {
                v["request_schema"] =
                    json!(format!("schemas/ipc/{}.request.schema.json", spec.name));
                v["response_schema"] =
                    json!(format!("schemas/ipc/{}.response.schema.json", spec.name));
            }
            v
        })
        .collect();
    let proposed: Vec<Value> = [
        g9_app::teaching::PROPOSED_OPERATIONS,
        g9_app::analytics::PROPOSED_OPERATIONS,
        g9_app::executive::PROPOSED_OPERATIONS,
        g9_app::research::PROPOSED_OPERATIONS,
    ]
    .iter()
    .flat_map(|area| area.iter())
    .map(|name| {
        let error = g9_app::not_implemented(name);
        json!({
            "name": name,
            "status": "PROPOSED",
            "error": { "category": error.category, "code": error.code },
        })
    })
    .collect();
    json!({
        "count": operations.len(),
        "operations": operations,
        "proposed_count": proposed.len(),
        "proposed": proposed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_every_routed_operation_with_flags_and_schema_paths() {
        let v = run(true);
        assert_eq!(v["count"], 35);
        let ops = v["operations"].as_array().unwrap();
        let apply = ops.iter().find(|o| o["name"] == "calendar.apply").unwrap();
        assert_eq!(apply["action"], "APPLY");
        assert_eq!(apply["resource_kind"], "SCHEDULE_PROPOSAL");
        assert_eq!(apply["mutating"], true);
        assert_eq!(apply["approval_expected"], true);
        assert_eq!(
            apply["request_schema"],
            "schemas/ipc/calendar.apply.request.schema.json"
        );
        let status = ops.iter().find(|o| o["name"] == "system.status").unwrap();
        assert_eq!(status["mutating"], false);
        assert_eq!(status["default_scope"]["domain"], "SYSTEM");
        for o in ops {
            let name = o["name"].as_str().unwrap();
            assert!(
                crate::bootstrap::repo_root()
                    .join(o["request_schema"].as_str().unwrap())
                    .is_file(),
                "{name}"
            );
        }
        let proposed = v["proposed"].as_array().unwrap();
        assert!(proposed.iter().any(|p| p["name"] == "teaching.propose_hint"
            && p["error"]["code"] == "teaching.not_implemented"
            && p["error"]["category"] == "PRECONDITION_FAILED"));
        assert!(run(false)["operations"][0].get("request_schema").is_none());
    }
}
