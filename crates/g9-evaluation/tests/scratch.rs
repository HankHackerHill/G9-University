#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use g9_evaluation::*;
use serde_json::json;

#[test]
fn explore() {
    let mut h = Harness::new().unwrap();
    println!("registry: {:?}", h.registry_status());
    let (_fx, seed) = fixtures::seeded_week03(&mut h).unwrap();
    println!("seed: {}", serde_json::to_string_pretty(&seed).unwrap());
    let p = h.call("competency.projection", json!({})).unwrap();
    println!("proj: {}", serde_json::to_string_pretty(&p["result"]).unwrap());
    let s = h.call("session.list", json!({})).unwrap();
    for x in s["result"]["sessions"].as_array().unwrap() { println!("session {} {} credited {}", x["session_id"], x["state"], x["credited_ms"]); }
    let st = h.step("export.request", json!({"domains":["LEARNING"],"format":"JSON","include_private":true}), &CallOpts::default());
    println!("export: {:?} decision={} cat={}", st.error().map(|e| (&e.code, &e.message)), st.decision, st.outcome_category);
    let st = h.step("notes.create", json!({"title":"t","body_markdown":"b","classification":"PRIVATE","domain":"PERSONAL"}), &CallOpts::default());
    println!("personal note: {:?} {} {}", st.error(), st.decision, st.outcome_category);
    let nid = st.value()["note_id"].as_str().unwrap_or("x").to_string();
    let st = h.step("notes.revise", json!({"note_id": nid, "body_markdown":"changed"}), &CallOpts::default());
    println!("cross revise: {:?} {} {}", st.error(), st.decision, st.outcome_category);
    let st = h.step("deletion.request", json!({"resource":{"kind":"ATTEMPT","id": seed.attempts["att-net"]},"reason":"x"}), &CallOpts::default());
    println!("del attempt: {:?} {} {}", st.error(), st.decision, st.outcome_category);
    // skills
    let valid = json!({
        "status": "COMPLETED", "result_schema_version": "1.0", "observations": [], "inferences": [], "evidence_refs": [],
        "confidence_limits": ["fixture"], "recommended_actions": [], "approval_requirements": [],
        "resource_usage": { "tokens": 10, "seconds": 1, "tool_calls": 0 }, "validation_results": [], "errors": [],
        "result": { "objectives": [], "prerequisites_status": [], "time_budget": {}, "next_step": "Review the routing table note" }
    });
    h.script_reply(&valid).unwrap();
    let st = h.step("skills.invoke", json!({"skill_code":"T01","input":{"day_id":"D15","mode":"STUDY"}}), &CallOpts::default());
    println!("invoke valid: {:?} {} {} -> {}", st.error(), st.decision, st.outcome_category, st.value());
    let mut bad = valid.clone(); bad["result"].as_object_mut().unwrap().remove("next_step");
    h.script_reply(&bad).unwrap();
    let st = h.step("skills.invoke", json!({"skill_code":"T01","input":{"day_id":"D15","mode":"STUDY"}}), &CallOpts::default());
    println!("invoke schema-invalid: {:?} {} {}", st.error(), st.decision, st.outcome_category);
    h.script_text_reply("not json at all").unwrap();
    let st = h.step("skills.invoke", json!({"skill_code":"T01","input":{"day_id":"D15","mode":"STUDY"}}), &CallOpts::default());
    println!("invoke text: {:?} {} {}", st.error(), st.decision, st.outcome_category);
    let st = h.step("skills.invoke", json!({"skill_code":"T01","input":{"day_id":"D15","mode":"STUDY","extra":1}}), &CallOpts::default());
    println!("invoke bad input: {:?} {} {}", st.error(), st.decision, st.outcome_category);
    let mut exe = valid.clone();
    exe["recommended_actions"] = json!([{ "action": "EXECUTE", "resource": { "kind": "PROCESS", "id": "bash" }, "rationale": "run", "evidence_refs": [], "parameters": {} }]);
    h.script_reply(&exe).unwrap();
    let st = h.step("skills.invoke", json!({"skill_code":"T01","input":{"day_id":"D15","mode":"STUDY"}}), &CallOpts::default());
    println!("invoke execute: {:?} {} {}", st.error(), st.decision, st.outcome_category);
    let st = h.step("skills.invoke", json!({"skill_code":"T01","input":{"day_id":"D15","mode":"STUDY"}}), &CallOpts::default());
    println!("invoke exhausted: {:?} {} {}", st.error(), st.decision, st.outcome_category);
    let st = h.step("system.status", json!({}), &CallOpts::default());
    println!("status: {}", st.value());
}
