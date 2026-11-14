#!/usr/bin/env bash
# Smoke test of the G9 MENTOR backend through the command-line driver (apps/cli).
#
# Builds `g9`, creates a disposable data directory and runs:
#   init -> import-seed -> simulate-week (fixed clock; PASS, complete, reproducible across
#   two fresh directories: identical step tables, the full reports differ only in data_dir
#   and the backend-generated ids under artifacts; a re-run on the same directory is FAIL at
#   the first replayed idempotency key and executes nothing; the Saturday grade reaches
#   competency.projection: its evidence_refs include the 3 observations derived from the
#   finalized attempt, and the self-adjudicated grade is provisional, never VALIDATED)
#   -> the scratch guard: simulate-week and crash-drill refuse to run without --scratch (they
#      write seed rows directly to storage), exit 1, and create nothing
#   -> crash-drill + init (the drill leaves exactly one open interval in storage, checked
#      read-only in the database before recovery; recovery closes that interval UNCERTAIN
#      and the session is INTERRUPTED with nothing credited)
#   -> the refusal that must happen (calendar.apply without approval -> APPROVAL_REQUIRED,
#      exit 2), the operator decision both ways across processes (reject; approve + retry
#      with --approval -> applied and CONSUMED; a consumed or rejected approval is refused
#      when presented again)
#   -> skills.invoke in EXAM mode with the registry loaded (exact observed refusal), without a
#      registry, and in an allowed mode with no model runtime
#   -> a PROPOSED operation -> exit-code contract
#   -> backups across processes and the offline restore, each step its own process: a note is created,
#      then backup.create, backup.verify and backup.restore_test reach RETAINED in three processes and
#      `g9 status` names the backup; a second note is created after the backup; `g9 restore` without confirmation exits 1 and
#      leaves the note (read-only SQLite check); wrong --expect-sha256 fails restore.checksum_mismatch
#      and changes nothing; `g9 restore --confirm-replace-live-database` succeeds, the note written after
#      the backup is gone and the note written before it is present (the live database is the snapshot,
#      not a fresh one), the quarantine directory holds the old database with the note,
#      restore-log.jsonl has one metadata-only line
#   -> no key material (hex, base64, byte list of every binding.key) in any stdout/stderr.
# Prints a summary table and exits non-zero on any failed assertion.
#
# Usage: bash scripts/smoke_test.sh            (from anywhere; paths are resolved from the script)
# Env:   CARGO_TARGET_DIR (default <repo>/target/cli), G9_SMOKE_KEEP=1 keeps the work directory.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$REPO/target/cli}"
WORK="$(mktemp -d "${TMPDIR:-/tmp}/g9-smoke.XXXXXX")"
DATA="$WORK/data"
DRILL="$WORK/drill"
FIXED_CLOCK=1794812400000   # 2026-11-16T08:00:00+01:00 (Monday)
START_DATE=2026-11-16
WEEK_MS=604800000

PASS=0
FAIL=0
ROWS=()

cleanup() {
  if [[ "${G9_SMOKE_KEEP:-0}" != "1" ]]; then
    rm -rf "$WORK"
  else
    echo "kept: $WORK"
  fi
}
trap cleanup EXIT

row() { # name result detail
  ROWS+=("$(printf '%-56s %-5s %s' "$1" "$2" "$3")")
  if [[ "$2" == "PASS" ]]; then PASS=$((PASS + 1)); else FAIL=$((FAIL + 1)); fi
}

assert_eq() { # name expected actual
  if [[ "$2" == "$3" ]]; then row "$1" PASS "$3"; else row "$1" FAIL "expected '$2', got '$3'"; fi
}

# jq-free JSON field access (python3 -I: isolated interpreter, no cwd imports).
jget() { # file path.to.field  (arrays by index)
  python3 -I - "$1" "$2" <<'PY'
import json, sys
with open(sys.argv[1]) as f:
    node = json.load(f)
for key in sys.argv[2].split('.'):
    if key == '':
        continue
    if isinstance(node, list):
        node = node[int(key)] if int(key) < len(node) else None
    else:
        node = node.get(key) if isinstance(node, dict) else None
    if node is None:
        break
if isinstance(node, bool):
    print('true' if node else 'false')
elif node is None:
    print('null')
elif isinstance(node, (dict, list)):
    print(json.dumps(node, separators=(',', ':')))
else:
    print(node)
PY
}

jlen() { # file path.to.array -> length (0 when absent)
  python3 -I - "$1" "$2" <<'PY'
import json, sys
with open(sys.argv[1]) as f:
    node = json.load(f)
for key in sys.argv[2].split('.'):
    if key == '':
        continue
    node = node[int(key)] if isinstance(node, list) else (node.get(key) if isinstance(node, dict) else None)
    if node is None:
        break
print(len(node) if isinstance(node, (list, dict)) else 0)
PY
}

count_steps() { # file op -> number of steps with that op ('' = all)
  python3 -I - "$1" "$2" <<'PY'
import json, sys
steps = json.load(open(sys.argv[1]))["steps"]
print(len(steps) if sys.argv[2] == '' else sum(1 for s in steps if s["op"] == sys.argv[2]))
PY
}

last_state() { # file op -> state_after.state of the last step with that op
  python3 -I - "$1" "$2" <<'PY'
import json, sys
steps = [s for s in json.load(open(sys.argv[1]))["steps"] if s["op"] == sys.argv[2]]
print(steps[-1]["state_after"].get("state") if steps else "absent")
PY
}

check_field() { # file check-name field -> field of the named simulate-week check's detail ('absent' when no such check)
  python3 -I - "$1" "$2" "$3" <<'PY'
import json, sys
checks = [c for c in json.load(open(sys.argv[1]))["checks"] if c["name"] == sys.argv[2]]
if not checks:
    print("absent")
else:
    v = checks[0]["detail"].get(sys.argv[3]) if sys.argv[3] != "passed" else checks[0]["passed"]
    print(json.dumps(v) if not isinstance(v, str) else v)
PY
}

step_field() { # file op field -> state_after.<field> of the last step with that op ('absent' when none)
  python3 -I - "$1" "$2" "$3" <<'PY'
import json, sys
steps = [s for s in json.load(open(sys.argv[1]))["steps"] if s["op"] == sys.argv[2]]
if not steps:
    print("absent")
else:
    v = steps[-1]["state_after"].get(sys.argv[3])
    print(json.dumps(v) if not isinstance(v, str) else v)
PY
}

first_fail() { # file -> "<first failing op>/<its decision>|<every failing op>" or "none"
  python3 -I - "$1" <<'PY'
import json, sys
fails = [s for s in json.load(open(sys.argv[1]))["steps"] if s["decision"].startswith("FAIL")]
print(f"{fails[0]['op']}/{fails[0]['decision']}|" + ",".join(f["op"] for f in fails) if fails else "none")
PY
}

open_intervals_in_db() { # sqlite file -> number of intervals without an end (read-only)
  python3 -I - "$1" <<'PY'
import sqlite3, sys
conn = sqlite3.connect("file:" + sys.argv[1] + "?mode=ro", uri=True)
print(conn.execute("SELECT COUNT(*) FROM time_intervals WHERE end_utc IS NULL").fetchone()[0])
conn.close()
PY
}

notes_with_id() { # sqlite file, note id, mode (ro | immutable) -> notes rows with that id (read-only)
  python3 -I - "$1" "$2" "${3:-ro}" <<'PY'
import sqlite3, sys
path, note, mode = sys.argv[1], sys.argv[2], sys.argv[3]
uri = "file:" + path + ("?mode=ro" if mode == "ro" else "?immutable=1")
conn = sqlite3.connect(uri, uri=True)
print(conn.execute("SELECT COUNT(*) FROM notes WHERE id = ?", (note,)).fetchone()[0])
conn.close()
PY
}

echo "== build (CARGO_TARGET_DIR=$CARGO_TARGET_DIR)"
(cd "$REPO/apps/cli" && cargo build --quiet)
G9="$CARGO_TARGET_DIR/debug/g9"
[[ -x "$G9" ]] || { echo "g9 binary missing at $G9"; exit 1; }

run() { # outfile args...  -> echoes exit code, never aborts the script
  local out="$1"; shift
  set +e
  "$G9" "$@" >"$out" 2>"$out.err"
  local code=$?
  set -e
  echo "$code"
}

echo "== init"
code=$(run "$WORK/init.json" init --data-dir "$DATA" --no-skills)
assert_eq "init exit code" 0 "$code"
assert_eq "init storage READY" READY "$(jget "$WORK/init.json" status.storage.state)"
assert_eq "init model unavailable (NullRuntime)" false "$(jget "$WORK/init.json" status.model.available)"
assert_eq "init network egress disabled" false "$(jget "$WORK/init.json" status.network.egress_allowed)"
assert_eq "init audit chain ok" true "$(jget "$WORK/init.json" recovery.audit_chain_ok)"
assert_eq "binding key file mode 0600" 600 "$(stat -c %a "$DATA/binding.key")"
assert_eq "binding key length 32 bytes" 32 "$(stat -c %s "$DATA/binding.key")"
if grep -q "WARNING binding_key.file_fallback" "$WORK/init.json"; then row "init reports key-file fallback" PASS "warning present"; else row "init reports key-file fallback" FAIL "warning missing"; fi

echo "== import-seed"
code=$(run "$WORK/import.json" import-seed --data-dir "$DATA" --no-skills)
assert_eq "import-seed exit code" 0 "$code"
assert_eq "import-seed version id" "g9-ct9-480:0.1.0-seed" "$(jget "$WORK/import.json" curriculum_version_id)"
assert_eq "import-seed counts weeks/days/objectives" "12/72/216" "$(jget "$WORK/import.json" weeks)/$(jget "$WORK/import.json" days)/$(jget "$WORK/import.json" objectives)"
assert_eq "import-seed consumed an approval" true "$([[ "$(jget "$WORK/import.json" approval_id)" != "null" ]] && echo true || echo false)"
code=$(run "$WORK/import2.json" import-seed --data-dir "$DATA" --no-skills)
assert_eq "import-seed re-run reports already imported" "0/true" "$code/$(jget "$WORK/import2.json" already_imported)"

echo "== simulate-week (fixed clock)"
code=$(run "$WORK/week.json" simulate-week --scratch --data-dir "$DATA" --no-skills --start-date "$START_DATE" --zone Europe/Berlin --fixed-clock "$FIXED_CLOCK")
assert_eq "simulate-week exit code" 0 "$code"
assert_eq "simulate-week result PASS and complete" "PASS/true/true" "$(jget "$WORK/week.json" result)/$(jget "$WORK/week.json" passed)/$(jget "$WORK/week.json" complete)"
assert_eq "simulate-week failures" "[]" "$(jget "$WORK/week.json" failures)"
assert_eq "simulate-week not_run" "[]" "$(jget "$WORK/week.json" not_run)"
assert_eq "simulate-week ids deterministic" sequential "$(jget "$WORK/week.json" clock.ids)"
steps=$(count_steps "$WORK/week.json" '')
assert_eq "simulate-week completed 34 block sessions" 34 "$(count_steps "$WORK/week.json" session.complete)"
assert_eq "simulate-week six daily plans, six pauses, six journal notes" "6/6/6" "$(count_steps "$WORK/week.json" curriculum.daily_plan)/$(count_steps "$WORK/week.json" session.pause)/$(count_steps "$WORK/week.json" notes.create)"
assert_eq "simulate-week attempt APPROVED_FINAL" APPROVED_FINAL "$(last_state "$WORK/week.json" assessment.approve_final)"
assert_eq "simulate-week backup RETAINED" RETAINED "$(last_state "$WORK/week.json" backup.restore_test)"
# The Saturday grade reaches the competency model: the projection is non-empty and its evidence_refs
# include the 3 observations derived from the finalized attempt (understanding, application, transfer).
# The operator grades their own attempt (SELF_ADJUDICATED), so the projection is provisional, never VALIDATED.
assert_eq "simulate-week projection lists the finalized attempt's observations" "true/3/3/3" "$(check_field "$WORK/week.json" "competency.projection includes the observations of the finalized attempt" passed)/$(check_field "$WORK/week.json" "competency.projection includes the observations of the finalized attempt" attempt_observations)/$(check_field "$WORK/week.json" "competency.projection includes the observations of the finalized attempt" evidence_refs)/$(step_field "$WORK/week.json" competency.projection evidence_refs)"
assert_eq "simulate-week self-adjudicated grade is provisional, not VALIDATED" "true/false/true" "$(check_field "$WORK/week.json" "self-adjudicated grade does not validate (provisional, no VALIDATED)" passed)/$(check_field "$WORK/week.json" "self-adjudicated grade does not validate (provisional, no VALIDATED)" validated)/$(check_field "$WORK/week.json" "self-adjudicated grade does not validate (provisional, no VALIDATED)" provisional)"
# Determinism: two runs on fresh directories (both import the seed) must produce identical step
# tables, and their full reports may differ only in the data directory and the backend ids.
code2=$(run "$WORK/week2.json" simulate-week --scratch --data-dir "$WORK/data2" --no-skills --start-date "$START_DATE" --zone Europe/Berlin --fixed-clock "$FIXED_CLOCK")
code3=$(run "$WORK/week3.json" simulate-week --scratch --data-dir "$WORK/data3" --no-skills --start-date "$START_DATE" --zone Europe/Berlin --fixed-clock "$FIXED_CLOCK")
same=$(python3 -I -c 'import json,sys; a=json.load(open(sys.argv[1]))["steps"]; b=json.load(open(sys.argv[2]))["steps"]; print("true" if a==b else "false")' "$WORK/week2.json" "$WORK/week3.json")
differing=$(python3 -I -c 'import json,sys; a=json.load(open(sys.argv[1])); b=json.load(open(sys.argv[2])); print(",".join(sorted(k for k in set(a)|set(b) if a.get(k)!=b.get(k))))' "$WORK/week2.json" "$WORK/week3.json")
fresh_steps=$(count_steps "$WORK/week2.json" '')
assert_eq "simulate-week step table reproducible (fresh dirs)" "0/0/true" "$code2/$code3/$same"
assert_eq "simulate-week full reports differ only in data_dir/artifacts" "artifacts,data_dir" "$differing"
assert_eq "simulate-week fresh run = pre-imported run + 4 import steps" "$((steps + 4))" "$fresh_steps"
# Re-running the same week with the same fixed clock issues the same sequential idempotency keys;
# the backend replays stored outcomes. The scenario must fail at the first replayed mutating step,
# never pass its checks on replayed results.
code=$(run "$WORK/week-again.json" simulate-week --scratch --data-dir "$DATA" --no-skills --start-date "$START_DATE" --zone Europe/Berlin --fixed-clock "$FIXED_CLOCK")
assert_eq "simulate-week re-run on the same data dir is FAIL (exit 2)" "2/FAIL" "$code/$(jget "$WORK/week-again.json" result)"
again_fail=$(first_fail "$WORK/week-again.json")
assert_eq "simulate-week re-run fails at the first replayed step; only the audit-growth check fails after it" "session.prepare/FAIL: idempotency.replayed|session.prepare,check:audit chain verifies and grew" "$again_fail"
assert_eq "simulate-week re-run executed no session (replay is not execution)" 0 "$(count_steps "$WORK/week-again.json" session.complete)"
if grep -q "fresh --data-dir" "$WORK/week-again.json"; then row "simulate-week re-run carries the hint" PASS "hint present"; else row "simulate-week re-run carries the hint" FAIL "hint missing"; fi

echo "== crash drill (two steps)"
# The scratch guard: both seed-writing commands refuse without --scratch (exit 1, message, nothing created).
code=$(run "$WORK/guard-week.json" simulate-week --data-dir "$WORK/guard" --no-skills --start-date "$START_DATE" --zone Europe/Berlin --fixed-clock "$FIXED_CLOCK")
assert_eq "simulate-week without --scratch is a usage error (exit 1)" 1 "$code"
code=$(run "$WORK/guard-drill.json" crash-drill --data-dir "$WORK/guard" --no-skills)
assert_eq "crash-drill without --scratch is a usage error (exit 1)" 1 "$code"
if grep -q -- "--scratch" "$WORK/guard-week.json.err" && grep -q "seed rows directly to storage" "$WORK/guard-week.json.err" && grep -q "throwaway data directory" "$WORK/guard-drill.json.err"; then row "scratch refusal explains seed rows and a throwaway directory" PASS "message present"; else row "scratch refusal explains seed rows and a throwaway directory" FAIL "message missing"; fi
assert_eq "refused seed-writing commands created no data directory" absent "$([[ -e "$WORK/guard" ]] && echo present || echo absent)"
code=$(run "$WORK/drill.json" crash-drill --scratch --data-dir "$DRILL" --no-skills)
assert_eq "crash-drill exit code" 0 "$code"
assert_eq "crash-drill session RUNNING" RUNNING "$(jget "$WORK/drill.json" state)"
session=$(jget "$WORK/drill.json" session_id)
interval=$(jget "$WORK/drill.json" intervals.0.interval_id)
assert_eq "crash-drill reports one open interval" "1/true/null" "$(jlen "$WORK/drill.json" intervals)/$([[ "$(jget "$WORK/drill.json" open_interval_started_at)" != "null" ]] && echo true || echo false)/$(jget "$WORK/drill.json" intervals.0.end_utc)"
assert_eq "crash-drill left one open interval in storage (read-only check)" 1 "$(open_intervals_in_db "$DRILL/g9.sqlite")"
code=$(run "$WORK/drill-init.json" init --data-dir "$DRILL" --no-skills)
assert_eq "init after crash exit code" 0 "$code"
assert_eq "init after crash: recovery closed that interval" "[\"$interval\"]" "$(jget "$WORK/drill-init.json" recovery.closed_intervals)"
assert_eq "init after crash: recovery interrupted that session" "[\"$session\"]" "$(jget "$WORK/drill-init.json" recovery.interrupted_sessions)"
assert_eq "init after crash: session INTERRUPTED" INTERRUPTED "$(jget "$WORK/drill-init.json" interrupted_sessions.0.state)"
assert_eq "init after crash: same session id" "$session" "$(jget "$WORK/drill-init.json" interrupted_sessions.0.session_id)"
assert_eq "init after crash: interval UNCERTAIN and closed" "$interval/UNCERTAIN/true" "$(jget "$WORK/drill-init.json" interrupted_sessions.0.intervals.0.interval_id)/$(jget "$WORK/drill-init.json" interrupted_sessions.0.intervals.0.continuity)/$(jget "$WORK/drill-init.json" interrupted_sessions.0.intervals.0.closed)"
assert_eq "init after crash: nothing credited, correction required" "0/true" "$(jget "$WORK/drill-init.json" interrupted_sessions.0.credited_ms)/$(jget "$WORK/drill-init.json" interrupted_sessions.0.requires_correction)"
assert_eq "init after crash: audit chain ok" true "$(jget "$WORK/drill-init.json" recovery.audit_chain_ok)"
assert_eq "init after crash: no open interval left in storage" 0 "$(open_intervals_in_db "$DRILL/g9.sqlite")"
code=$(run "$WORK/drill-init2.json" init --data-dir "$DRILL" --no-skills)
assert_eq "second init: nothing left to recover" "0/[]" "$code/$(jget "$WORK/drill-init2.json" recovery.interrupted_sessions)"

echo "== refusal that must happen: calendar.apply without approval; the decision both ways"
code=$(run "$WORK/propose.json" call calendar.propose --data-dir "$DATA" --no-skills --payload "{\"from\":$FIXED_CLOCK,\"to\":$((FIXED_CLOCK + WEEK_MS)),\"intent\":\"ADD_REVIEWS\"}")
assert_eq "calendar.propose (ADD_REVIEWS) exit code" 0 "$code"
proposal=$(jget "$WORK/propose.json" result.proposal_id)
code=$(run "$WORK/apply.json" call calendar.apply --data-dir "$DATA" --no-skills --payload "{\"proposal_id\":\"$proposal\"}")
assert_eq "calendar.apply without approval exit code" 2 "$code"
assert_eq "calendar.apply without approval category" APPROVAL_REQUIRED "$(jget "$WORK/apply.json" category)"
approval=$(jget "$WORK/apply.json" approval_id)
assert_eq "calendar.apply created an approval request" true "$([[ "$approval" != "null" ]] && echo true || echo false)"
code=$(run "$WORK/pending.json" approvals --data-dir "$DATA" --no-skills --state PENDING)
pending_ops=$(python3 -I -c 'import json,sys; print(",".join(sorted(a["operation_name"] for a in json.load(open(sys.argv[1]))["result"]["approvals"])))' "$WORK/pending.json")
assert_eq "approvals --state PENDING lists calendar.apply" calendar.apply "$pending_ops"
code=$(run "$WORK/reject.json" reject "$approval" --data-dir "$DATA" --no-skills --reason "smoke test")
assert_eq "reject pending approval" "0/REJECTED" "$code/$(jget "$WORK/reject.json" result.state)"
if grep -q "approvals.reason_not_persisted" "$WORK/reject.json"; then row "reject --reason is reported as not persisted (PROPOSED)" PASS "warning present"; else row "reject --reason is reported as not persisted (PROPOSED)" FAIL "warning missing"; fi
code=$(run "$WORK/apply-rejected.json" call calendar.apply --data-dir "$DATA" --no-skills --approval "$approval" --payload "{\"proposal_id\":\"$proposal\"}")
assert_eq "calendar.apply with a REJECTED approval is refused" "2/APPROVAL_REQUIRED/approval.not_approved" "$code/$(jget "$WORK/apply-rejected.json" category)/$(jget "$WORK/apply-rejected.json" code)"
# The approve path, each step in its own process (approval persisted and MAC-bound through the key file).
code=$(run "$WORK/propose2.json" call calendar.propose --data-dir "$DATA" --no-skills --payload "{\"from\":$((FIXED_CLOCK + WEEK_MS)),\"to\":$((FIXED_CLOCK + 2 * WEEK_MS)),\"intent\":\"ADD_REVIEWS\"}")
proposal2=$(jget "$WORK/propose2.json" result.proposal_id)
code=$(run "$WORK/apply2.json" call calendar.apply --data-dir "$DATA" --no-skills --payload "{\"proposal_id\":\"$proposal2\"}")
approval2=$(jget "$WORK/apply2.json" approval_id)
assert_eq "second calendar.apply without approval" "2/APPROVAL_REQUIRED" "$code/$(jget "$WORK/apply2.json" category)"
code=$(run "$WORK/approve.json" approve "$approval2" --data-dir "$DATA" --no-skills)
assert_eq "approve pending approval" "0/APPROVED/calendar.apply" "$code/$(jget "$WORK/approve.json" result.state)/$(jget "$WORK/approve.json" result.operation_name)"
code=$(run "$WORK/apply-approved.json" call calendar.apply --data-dir "$DATA" --no-skills --approval "$approval2" --payload "{\"proposal_id\":\"$proposal2\"}")
assert_eq "calendar.apply with the approval succeeds" "0/$proposal2" "$code/$(jget "$WORK/apply-approved.json" result.proposal_id)"
code=$(run "$WORK/consumed.json" approvals --data-dir "$DATA" --no-skills --state CONSUMED)
consumed=$(python3 -I -c 'import json,sys; print(",".join(sorted(a["approval_id"] for a in json.load(open(sys.argv[1]))["result"]["approvals"] if a["operation_name"]=="calendar.apply")))' "$WORK/consumed.json")
assert_eq "the approval is CONSUMED (single use)" "$approval2" "$consumed"
code=$(run "$WORK/apply-replay.json" call calendar.apply --data-dir "$DATA" --no-skills --approval "$approval2" --payload "{\"proposal_id\":\"$proposal2\"}")
assert_eq "presenting the consumed approval again is refused" 2 "$code"

echo "== skills.invoke: EXAM mode with the registry, without a registry, allowed mode without a model"
code=$(run "$WORK/init-skills.json" init --data-dir "$DATA")
assert_eq "skill registry loads (packages > 0, none refused)" "0/true/null" "$code/$([[ "$(jget "$WORK/init-skills.json" skills.packages)" =~ ^[1-9][0-9]*$ ]] && echo true || echo false)/$(jget "$WORK/init-skills.json" skills.refused)"
# With the registry loaded: the orchestrator preflight refuses the mode before any model call
# (observed: PRECONDITION_FAILED skills.preflight_failed; the policy table allows the invoke itself).
code=$(run "$WORK/skill-exam.json" call skills.invoke --data-dir "$DATA" --mode EXAM --payload '{"skill_code":"g9-socratic-coaching","input":{"attempt_context_ref":"ev-smoke","stuck_point":"smoke"}}')
assert_eq "skills.invoke EXAM refused by preflight" "2/PRECONDITION_FAILED/skills.preflight_failed" "$code/$(jget "$WORK/skill-exam.json" category)/$(jget "$WORK/skill-exam.json" code)"
code=$(run "$WORK/skill-none.json" call skills.invoke --data-dir "$DATA" --no-skills --mode EXAM --payload '{"skill_code":"g9-socratic-coaching","input":{"attempt_context_ref":"ev-smoke"}}')
assert_eq "skills.invoke without registry" "2/RESOURCE_UNAVAILABLE/skills.registry_unavailable" "$code/$(jget "$WORK/skill-none.json" category)/$(jget "$WORK/skill-none.json" code)"
code=$(run "$WORK/skill-study.json" call skills.invoke --data-dir "$DATA" --payload '{"skill_code":"g9-socratic-coaching","input":{"attempt_context_ref":"ev-smoke"}}')
assert_eq "skills.invoke STUDY with NullRuntime" "2/RESOURCE_UNAVAILABLE/skills.backend.unavailable" "$code/$(jget "$WORK/skill-study.json" category)/$(jget "$WORK/skill-study.json" code)"

echo "== PROPOSED operation and exit-code contract"
code=$(run "$WORK/proposed.json" call teaching.propose_hint --data-dir "$DATA" --no-skills)
assert_eq "PROPOSED op answers PRECONDITION_FAILED" "2/PRECONDITION_FAILED/teaching.not_implemented" "$code/$(jget "$WORK/proposed.json" category)/$(jget "$WORK/proposed.json" code)"
code=$(run "$WORK/unknown.json" call nope.op --data-dir "$DATA" --no-skills)
assert_eq "unknown op is INVALID_INPUT" "2/INVALID_INPUT" "$code/$(jget "$WORK/unknown.json" category)"
code=$(run "$WORK/badjson.json" call system.status --data-dir "$DATA" --no-skills --payload '{bad')
assert_eq "bad payload JSON is a usage error" 1 "$code"
code=$(run "$WORK/tuesday.json" simulate-week --scratch --data-dir "$WORK/unused" --no-skills --start-date 2026-11-17)
assert_eq "simulate-week on a Tuesday is a usage error" 1 "$code"
code=$(run "$WORK/ops.json" ops --schemas)
assert_eq "ops lists 53 operations" "0/53" "$code/$(jget "$WORK/ops.json" count)"
code=$(run "$WORK/status.json" status --data-dir "$DATA" --no-skills)
assert_eq "status after the week: storage READY" "0/READY" "$code/$(jget "$WORK/status.json" result.storage.state)"

echo "== backups across processes and the offline restore (each step its own process)"
RDATA="$WORK/restore"
code=$(run "$WORK/r-init.json" init --data-dir "$RDATA" --no-skills)
assert_eq "restore dir: init exit code" 0 "$code"
# A note written BEFORE the backup is part of the snapshot: after the restore it must still be there,
# which a restore that merely quarantined the old database and booted a fresh one would not satisfy.
code=$(run "$WORK/r-marker.json" call notes.create --data-dir "$RDATA" --no-skills --payload '{"title":"before the backup","body_markdown":"written before the backup","classification":"PRIVATE","domain":"LEARNING"}')
marker_id=$(jget "$WORK/r-marker.json" result.note_id)
assert_eq "notes.create before the backup" "0/1" "$code/$(notes_with_id "$RDATA/g9.sqlite" "$marker_id")"
code=$(run "$WORK/r-create.json" call backup.create --data-dir "$RDATA" --no-skills --payload '{"label":"smoke"}')
backup_id=$(jget "$WORK/r-create.json" result.backup_id)
assert_eq "backup.create (process 1)" "0/SNAPSHOT_CREATED" "$code/$(jget "$WORK/r-create.json" result.state)"
code=$(run "$WORK/r-verify.json" call backup.verify --data-dir "$RDATA" --no-skills --payload "{\"backup_id\":\"$backup_id\"}")
assert_eq "backup.verify of that backup in a separate process" "0/CHECKED" "$code/$(jget "$WORK/r-verify.json" result.state)"
code=$(run "$WORK/r-test.json" call backup.restore_test --data-dir "$RDATA" --no-skills --payload "{\"backup_id\":\"$backup_id\"}")
assert_eq "backup.restore_test in a third process reaches RETAINED" "0/RETAINED" "$code/$(jget "$WORK/r-test.json" result.state)"
code=$(run "$WORK/r-status.json" status --data-dir "$RDATA" --no-skills)
assert_eq "status in a fourth process names the retained backup" "0/$backup_id/RETAINED" "$code/$(jget "$WORK/r-status.json" result.last_backup.backup_id)/$(jget "$WORK/r-status.json" result.last_backup.state)"
code=$(run "$WORK/r-unknown.json" call backup.verify --data-dir "$RDATA" --no-skills --payload '{"backup_id":"01a1ba37-2097-7435-97d0-f1394210d000"}')
assert_eq "backup.verify of an unknown id is NOT_FOUND" "2/NOT_FOUND/backup.not_found" "$code/$(jget "$WORK/r-unknown.json" category)/$(jget "$WORK/r-unknown.json" code)"
code=$(run "$WORK/r-note.json" call notes.create --data-dir "$RDATA" --no-skills --payload '{"title":"after the backup","body_markdown":"written after the backup","classification":"PRIVATE","domain":"LEARNING"}')
note_id=$(jget "$WORK/r-note.json" result.note_id)
assert_eq "notes.create after the backup" "0/1" "$code/$(notes_with_id "$RDATA/g9.sqlite" "$note_id")"
live_before=$(sha256sum "$RDATA/g9.sqlite" | cut -d' ' -f1)
listing_before=$(ls -A "$RDATA" | tr '\n' ',')
code=$(run "$WORK/r-plan.json" restore --backup-id "$backup_id" --data-dir "$RDATA" --no-skills)
assert_eq "restore without confirmation prints the plan and exits 1" "1/PLAN/false" "$code/$(jget "$WORK/r-plan.json" restore)/$(jget "$WORK/r-plan.json" changed)"
assert_eq "restore plan names the snapshot, live path and quarantine directory" "true/true/true" "$([[ "$(jget "$WORK/r-plan.json" snapshot)" == "$RDATA/backups/$backup_id.sqlite" ]] && echo true || echo false)/$([[ "$(jget "$WORK/r-plan.json" live)" == "$RDATA/g9.sqlite" ]] && echo true || echo false)/$([[ "$(jget "$WORK/r-plan.json" quarantine_dir)" == "$RDATA/quarantine" ]] && echo true || echo false)"
assert_eq "unconfirmed restore left the note in the live database" "1" "$(notes_with_id "$RDATA/g9.sqlite" "$note_id")"
assert_eq "unconfirmed restore changed no byte of the live database and made no quarantine" "$live_before/absent" "$(sha256sum "$RDATA/g9.sqlite" | cut -d' ' -f1)/$([[ -e "$RDATA/quarantine" ]] && echo present || echo absent)"
assert_eq "unconfirmed restore created no file in the data directory (no -wal/-shm either)" "$listing_before" "$(ls -A "$RDATA" | tr '\n' ',')"
wrong_sha=$(printf '0%.0s' $(seq 1 64))
code=$(run "$WORK/r-wrong.json" restore --backup-id "$backup_id" --expect-sha256 "$wrong_sha" --data-dir "$RDATA" --no-skills --confirm-replace-live-database)
assert_eq "restore with a wrong --expect-sha256 fails restore.checksum_mismatch" "2/restore.checksum_mismatch" "$code/$(jget "$WORK/r-wrong.json" code)"
code=$(run "$WORK/r-wrong-snap.json" restore --snapshot "$RDATA/backups/$backup_id.sqlite" --expect-sha256 "$wrong_sha" --data-dir "$RDATA" --no-skills --confirm-replace-live-database)
assert_eq "restore --snapshot with a wrong checksum fails restore.checksum_mismatch" "2/restore.checksum_mismatch" "$code/$(jget "$WORK/r-wrong-snap.json" code)"
assert_eq "failed restores changed nothing (live bytes, note, no quarantine, no log, no temp file)" "$live_before/1/absent/absent/0" "$(sha256sum "$RDATA/g9.sqlite" | cut -d' ' -f1)/$(notes_with_id "$RDATA/g9.sqlite" "$note_id")/$([[ -e "$RDATA/quarantine" ]] && echo present || echo absent)/$([[ -e "$RDATA/restore-log.jsonl" ]] && echo present || echo absent)/$(ls "$RDATA" | grep -c 'restore-tmp' || true)"
code=$(run "$WORK/r-nosha.json" restore --snapshot "$RDATA/backups/$backup_id.sqlite" --data-dir "$RDATA" --no-skills --confirm-replace-live-database)
assert_eq "restore --snapshot without --expect-sha256 is a usage error" 1 "$code"
code=$(run "$WORK/r-restore.json" restore --backup-id "$backup_id" --data-dir "$RDATA" --no-skills --confirm-replace-live-database)
assert_eq "restore with confirmation succeeds" "0/true" "$code/$(jget "$WORK/r-restore.json" restored)"
assert_eq "restore verified the snapshot and booted READY with the audit chain ok" "READY/true" "$(jget "$WORK/r-restore.json" boot.status.storage.state)/$(jget "$WORK/r-restore.json" boot.recovery.audit_chain_ok)"
assert_eq "the note written after the backup is gone from the live database" 0 "$(notes_with_id "$RDATA/g9.sqlite" "$note_id")"
assert_eq "the note written before the backup is in the live database (the snapshot, not a fresh database)" 1 "$(notes_with_id "$RDATA/g9.sqlite" "$marker_id")"
assert_eq "restore reports the SHA-256 of the snapshot file, which is unchanged" "$(sha256sum "$RDATA/backups/$backup_id.sqlite" | cut -d' ' -f1)" "$(jget "$WORK/r-restore.json" snapshot_sha256)"
assert_eq "restore verified the snapshot's audit chain (rows > 0)" true "$([[ "$(jget "$WORK/r-restore.json" audit_rows_verified)" =~ ^[1-9][0-9]*$ ]] && echo true || echo false)"
quarantined_db=$(ls "$RDATA"/quarantine/g9.sqlite.*.quarantine | head -n 1)
assert_eq "quarantine directory (0700) holds the old database with the note" "700/1" "$(stat -c %a "$RDATA/quarantine")/$(notes_with_id "$quarantined_db" "$note_id" immutable)"
assert_eq "restore-log.jsonl has exactly one line" 1 "$(wc -l < "$RDATA/restore-log.jsonl")"
assert_eq "restore-log.jsonl holds metadata only (timestamp, snapshot, sha256, quarantined)" "quarantined,sha256,snapshot,timestamp" "$(python3 -I -c 'import json,sys; print(",".join(sorted(json.loads(open(sys.argv[1]).readline()).keys())))' "$RDATA/restore-log.jsonl")"
cp "$RDATA/restore-log.jsonl" "$WORK/r-restore-log.json"
code=$(run "$WORK/r-status-after.json" status --data-dir "$RDATA" --no-skills)
assert_eq "status after the restore: READY (the snapshot predates its own manifest row, so no last_backup)" "0/READY/null" "$code/$(jget "$WORK/r-status-after.json" result.storage.state)/$(jget "$WORK/r-status-after.json" result.last_backup)"

echo "== key material"
# Every binding.key created above, in every text encoding (hex both cases, base64 standard and
# URL-safe, Rust byte-list), must appear in no stdout or stderr capture.
leaks=$(python3 -I - "$WORK" <<'PY'
import base64, glob, os, sys
work = sys.argv[1]
keys = glob.glob(os.path.join(work, "*", "binding.key"))
if len(keys) < 4:
    print(f"only {len(keys)} key files found")
    sys.exit(0)
encodings = []
for path in keys:
    raw = open(path, "rb").read()
    if len(raw) != 32:
        print(f"{path} is {len(raw)} bytes")
    hx = raw.hex()
    std = base64.b64encode(raw).decode().rstrip("=")
    url = base64.urlsafe_b64encode(raw).decode().rstrip("=")
    encodings += [hx, hx.upper(), std, url, str(list(raw))]
hits = []
for out in sorted(glob.glob(os.path.join(work, "*.json")) + glob.glob(os.path.join(work, "*.err"))):
    text = open(out, "rb").read().decode("utf-8", "replace")
    for enc in encodings:
        if enc in text:
            hits.append(os.path.basename(out))
            break
print(",".join(hits) if hits else "none")
PY
)
assert_eq "no key material in any stdout/stderr (hex, base64, byte list)" none "$leaks"

echo
echo "== summary ($steps simulated steps with the seed pre-imported, $fresh_steps on a fresh directory; $PASS passed, $FAIL failed)"
printf '%-56s %-5s %s\n' "assertion" "result" "detail"
printf '%-56s %-5s %s\n' "--------------------------------------------------------" "-----" "------"
for r in "${ROWS[@]}"; do echo "$r"; done
echo
if [[ "$FAIL" -ne 0 ]]; then
  echo "SMOKE TEST FAIL ($FAIL assertion(s) failed)"
  exit 1
fi
echo "SMOKE TEST PASS"
