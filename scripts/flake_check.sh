#!/usr/bin/env bash
# Flake check (plan step TEST-4, docs/release/path-to-10.md): run the test suites
# repeatedly on a frozen tree and report every test whose outcome differs between
# runs, and every failure. A run that differs from another is a flake until its cause
# is named; nothing here retries or hides a result.
#
# Usage:
#   scripts/flake_check.sh --out <dir> [--rust N] [--rust-single N] [--vitest N] [--e2e N]
#                          [--target <dir>]
#   --rust N         full Rust suite N times, default test threads   (default 10)
#   --rust-single N  full Rust suite N times, one test thread        (default 3)
#   --vitest N       the desktop vitest suite N times                 (default 10)
#   --e2e N          scripts/e2e/personal.cjs N times (real backend)  (default 3)
#   --target         CARGO_TARGET_DIR (default target/flake)
# The counts are a choice (the plan proposes 20); the report states the counts used.
# Offline: cargo runs with --offline --locked; npm needs apps/desktop/node_modules.
# Runs in the working tree as it is: commit or stash first so the tree is frozen.
# Exit status: 0 when every run of every suite had the same outcome per test and no
# failure; 1 otherwise; 2 on a usage error.
set -euo pipefail

OUT=""
RUST=10
RUST_SINGLE=3
VITEST=10
E2E=3
TARGET=""
while [ $# -gt 0 ]; do
  case "$1" in
    --out) OUT="${2:?--out needs a value}"; shift 2 ;;
    --rust) RUST="${2:?}"; shift 2 ;;
    --rust-single) RUST_SINGLE="${2:?}"; shift 2 ;;
    --vitest) VITEST="${2:?}"; shift 2 ;;
    --e2e) E2E="${2:?}"; shift 2 ;;
    --target) TARGET="${2:?}"; shift 2 ;;
    -h|--help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done
[ -n "$OUT" ] || { echo "--out is required" >&2; exit 2; }
for n in "$RUST" "$RUST_SINGLE" "$VITEST" "$E2E"; do
  case "$n" in ''|*[!0-9]*) echo "counts are whole numbers" >&2; exit 2 ;; esac
done

REPO="$(cd "$(dirname "$0")/.." && pwd)"
[ -n "$TARGET" ] || TARGET="$REPO/target/flake"
mkdir -p "$OUT"
export CARGO_TARGET_DIR="$TARGET" CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_TERM_COLOR=never

echo "revision : $(git -C "$REPO" rev-parse HEAD)"
if [ -n "$(git -C "$REPO" status --porcelain --untracked-files=no)" ]; then
  echo "NOTE     : tracked files have uncommitted changes; the runs test the working tree"
fi
echo "counts   : rust $RUST, rust single-thread $RUST_SINGLE, vitest $VITEST, e2e $E2E"

# One file per run: "<suite>\t<test>\t<outcome>" lines.
rust_run() {
  local label="$1"; shift
  local log="$OUT/$label.log"
  (cd "$REPO" && cargo test --offline --locked --all-targets --no-fail-fast "$@") >"$log" 2>&1 || true
  # "Running <target>" names the binary; "test <name> ... <outcome>" the result.
  awk '
    /^     Running / { bin = $2 }
    /^   Doc-tests / { bin = "doc:" $2 }
    /^test .* \.\.\. (ok|FAILED|ignored)/ {
      outcome = $NF; name = $2
      if ($0 ~ /\.\.\. ignored/) outcome = "ignored"
      print bin "\t" name "\t" outcome
    }' "$log" | sort >"$OUT/$label.tsv"
}

if [ "$RUST" -gt 0 ] || [ "$RUST_SINGLE" -gt 0 ]; then
  echo "building the Rust tests once"
  (cd "$REPO" && cargo test --offline --locked --all-targets --no-run) >"$OUT/rust-build.log" 2>&1
  for i in $(seq 1 "$RUST"); do echo "rust run $i/$RUST"; rust_run "rust-$i"; done
  for i in $(seq 1 "$RUST_SINGLE"); do
    echo "rust single-thread run $i/$RUST_SINGLE"; rust_run "rust-single-$i" -- --test-threads=1
  done
fi

if [ "$VITEST" -gt 0 ]; then
  for i in $(seq 1 "$VITEST"); do
    echo "vitest run $i/$VITEST"
    (cd "$REPO/apps/desktop" && npx vitest run --reporter=json --outputFile="$OUT/vitest-$i.json") \
      >"$OUT/vitest-$i.log" 2>&1 || true
    python3 -I - "$OUT/vitest-$i.json" "$OUT/vitest-$i.tsv" <<'PY'
import json, sys
src, dst = sys.argv[1], sys.argv[2]
rows = []
try:
    data = json.load(open(src, encoding="utf-8"))
    for f in data.get("testResults", []):
        for t in f.get("assertionResults", []):
            rows.append(f"{f.get('name','?').rsplit('/apps/desktop/',1)[-1]}\t{t.get('fullName','?')}\t{t.get('status','?')}")
except (OSError, ValueError):
    rows.append("vitest\t<no report>\tFAILED")
open(dst, "w", encoding="utf-8").write("\n".join(sorted(rows)) + "\n")
PY
  done
fi

if [ "$E2E" -gt 0 ]; then
  (cd "$REPO" && cargo build --offline --locked -p g9-cli --example bridge_stdio) >"$OUT/bridge-build.log" 2>&1
  (cd "$REPO/apps/desktop" && npm run build) >"$OUT/vite-build.log" 2>&1
  for i in $(seq 1 "$E2E"); do
    echo "e2e run $i/$E2E"
    G9_BRIDGE="$TARGET/debug/examples/bridge_stdio" \
      node "$REPO/scripts/e2e/personal.cjs" --out "$OUT/e2e-$i" >"$OUT/e2e-$i.log" 2>&1 || true
    grep -E '^(PASS|FAIL) ' "$OUT/e2e-$i.log" | sed -E 's/^(PASS|FAIL)  ?(.*)$/e2e\t\2\t\1/' \
      | sed -E 's/  \(.*\)\t/\t/' | sort >"$OUT/e2e-$i.tsv" || true
  done
fi

python3 -I - "$OUT" <<'PY'
import collections, glob, os, sys
out = sys.argv[1]
groups = collections.defaultdict(list)
for path in sorted(glob.glob(os.path.join(out, "*.tsv"))):
    name = os.path.basename(path)[:-4]
    suite = name.rsplit("-", 1)[0]
    groups[suite].append(path)
bad = False
lines = ["| Suite | Runs | Tests per run | Failures (any run) | Tests whose outcome differs |", "|---|---|---|---|---|"]
details = []
for suite, paths in sorted(groups.items()):
    runs = []
    for p in paths:
        rows = {}
        for line in open(p, encoding="utf-8"):
            parts = line.rstrip("\n").split("\t")
            if len(parts) == 3:
                rows[(parts[0], parts[1])] = parts[2]
        runs.append(rows)
    keys = set().union(*[set(r) for r in runs]) if runs else set()
    differs = sorted(k for k in keys if len({r.get(k, "missing") for r in runs}) > 1)
    failed = sorted(k for k in keys if any(r.get(k) in ("FAILED", "failed", "FAIL") for r in runs))
    counts = sorted({len(r) for r in runs})
    lines.append(f"| {suite} | {len(runs)} | {', '.join(map(str, counts))} | {len(failed)} | {len(differs)} |")
    for k in differs:
        details.append(f"- {suite}: `{k[0]}` `{k[1]}`: " + ", ".join(r.get(k, "missing") for r in runs))
    for k in failed:
        details.append(f"- {suite}: FAILED `{k[0]}` `{k[1]}`")
    if differs or failed or len(counts) > 1 or not keys:
        bad = True
report = "\n".join(lines) + ("\n\n" + "\n".join(details) if details else "\n\nNo test differed between runs and none failed.") + "\n"
open(os.path.join(out, "summary.md"), "w", encoding="utf-8").write(report)
print(report)
sys.exit(1 if bad else 0)
PY
