#!/usr/bin/env bash
# Local replication of .github/workflows/ci.yml on a CLEAN EXPORT of a git revision.
#
# Why an export: a CI checkout contains only what git tracks. A working tree also holds
# untracked files, empty directories (git cannot store them) and build outputs, any of
# which can make a test pass locally and fail on CI. Every step below runs inside
# `git archive <rev>`, never in the working tree.
#
# A step that cannot run here is reported SKIPPED with the reason. SKIPPED is not PASS.
# Exit status: 0 when no step failed, 1 when any step failed, 2 on a usage error.
#
# Usage:
#   scripts/ci_preflight.sh [--rev <rev>] [--work <dir>] [--target <dir>]
#                           [--only <job>[,<job>...]] [--skip <job>[,<job>...]]
#                           [--offline] [--keep]
# Jobs (the names of the CI jobs): rust, desktop-shell, frontend, content-gates, sbom
#   --rev      revision to export (default HEAD); uncommitted changes are never tested
#   --work     work directory (default: a new directory under ${TMPDIR:-/tmp})
#   --target   CARGO_TARGET_DIR (default: <work>/target; a fresh one needs several GB)
#   --offline  skip the steps that need the network (npm ci, npm audit, npx, cargo install)
#   --keep     keep the work directory (logs are always kept when a step fails)
set -euo pipefail

REV="HEAD"
WORK=""
TARGET=""
ONLY=""
SKIP=""
OFFLINE=0
KEEP=0

usage() { sed -n '2,22p' "$0" | sed 's/^# \{0,1\}//'; }

while [ $# -gt 0 ]; do
  case "$1" in
    --rev) REV="${2:?--rev needs a value}"; shift 2 ;;
    --work) WORK="${2:?--work needs a value}"; shift 2 ;;
    --target) TARGET="${2:?--target needs a value}"; shift 2 ;;
    --only) ONLY="${2:?--only needs a value}"; shift 2 ;;
    --skip) SKIP="${2:?--skip needs a value}"; shift 2 ;;
    --offline) OFFLINE=1; shift ;;
    --keep) KEEP=1; shift ;;
    -h|--help) usage; exit 0 ;;
    *) echo "unknown argument: $1" >&2; usage >&2; exit 2 ;;
  esac
done

REPO="$(git -C "$(dirname "$0")/.." rev-parse --show-toplevel)"
SHA="$(git -C "$REPO" rev-parse --verify "${REV}^{commit}")" || { echo "not a commit: $REV" >&2; exit 2; }

for job in $(echo "$ONLY,$SKIP" | tr ',' ' '); do
  case "$job" in
    rust|desktop-shell|frontend|content-gates|sbom) ;;
    *) echo "unknown job: $job" >&2; exit 2 ;;
  esac
done

wanted() {
  local job="$1"
  if [ -n "$ONLY" ] && ! echo ",$ONLY," | grep -q ",$job,"; then return 1; fi
  if echo ",$SKIP," | grep -q ",$job,"; then return 1; fi
  return 0
}

if [ -z "$WORK" ]; then WORK="$(mktemp -d "${TMPDIR:-/tmp}/g9-preflight.XXXXXX")"; fi
# The run deletes the export and, when it created it, the build directory. A path with '..' can
# name a directory outside --work, so such paths are refused and the others are resolved first
# (audit 2026-11-19, P2D-01).
case "/$WORK/" in */../*) echo "refusing a --work with '..' components: $WORK" >&2; exit 2 ;; esac
WORK="$(realpath -m -- "$WORK")"
SRC="$WORK/src"
LOGS="$WORK/logs"
if [ -e "$SRC" ]; then echo "refusing to reuse an existing export: $SRC" >&2; exit 2; fi
mkdir -p "$SRC" "$LOGS"
if [ -z "$TARGET" ]; then
  TARGET="$WORK/target"
else
  case "/$TARGET/" in */../*) echo "refusing a --target with '..' components: $TARGET" >&2; exit 2 ;; esac
  TARGET="$(realpath -m -- "$TARGET")"
fi
# A build directory that exists before this run is never this run's to delete.
TARGET_EXISTED=0
if [ -e "$TARGET" ]; then TARGET_EXISTED=1; fi
if [ "$TARGET" = "$WORK/target" ] && [ "$TARGET_EXISTED" -eq 1 ]; then
  echo "refusing to reuse an existing build directory: $TARGET" >&2; exit 2
fi

git -C "$REPO" archive "$SHA" | tar -x -C "$SRC"
echo "revision : $SHA ($REV)"
echo "export   : $SRC ($(find "$SRC" -type f | wc -l) files)"
echo "target   : $TARGET"
if [ -n "$(git -C "$REPO" status --porcelain)" ]; then
  echo "NOTE     : the working tree has uncommitted changes; they are NOT part of this run"
fi

# CI sets these for every job.
export CARGO_TERM_COLOR=never
export RUSTFLAGS="-D warnings"
export CARGO_TARGET_DIR="$TARGET"

RESULTS=()
FAILED=0
SKIPPED=0
N=0

# step <job> <name> <working dir relative to the export> <command...>
step() {
  local job="$1" name="$2" dir="$3"; shift 3
  N=$((N + 1))
  local log; log="$LOGS/$(printf '%02d' "$N")-$job-$(echo "$name" | tr -c 'a-zA-Z0-9' '_').log"
  printf '%-14s %-44s ' "$job" "$name"
  local rc=0
  (cd "$SRC/$dir" && "$@") >"$log" 2>&1 || rc=$?
  if [ "$rc" -eq 0 ]; then
    echo "PASS"; RESULTS+=("PASS|$job|$name|")
  else
    echo "FAIL (exit $rc; $log)"; RESULTS+=("FAIL|$job|$name|exit $rc, log $log"); FAILED=$((FAILED + 1))
  fi
}

skip() {
  local job="$1" name="$2" why="$3"
  printf '%-14s %-44s SKIPPED (%s)\n' "$job" "$name" "$why"
  RESULTS+=("SKIPPED|$job|$name|$why"); SKIPPED=$((SKIPPED + 1))
}

have() { command -v "$1" >/dev/null 2>&1; }

if wanted rust; then
  step rust "fmt" . cargo fmt --all --check
  step rust "clippy" . cargo clippy --locked --all-targets -- -D warnings
  step rust "test" . cargo test --locked --all-targets
  step rust "doc tests" . cargo test --locked --doc
  # The deny.toml target graph, shell included (needs the Tauri crates in the
  # registry, not WebKit): the ignored test of the layering gate.
  step rust "layering: deny-target graph" . cargo test --locked -p g9-evaluation --test layering_gate -- --ignored
  step rust "dependency tree" . sh -c 'cargo tree --locked -e normal --prefix none | sort -u | wc -l'
  if have cargo-deny; then
    # CI installs cargo-deny (pinned) with cargo install and runs it live, which fetches the advisory database.
    # Offline, the local advisory database is used as it is; its age is printed in the log.
    if [ "$OFFLINE" -eq 1 ]; then
      step rust "cargo-deny (offline database)" . cargo deny --all-features --locked --offline check
    else
      step rust "cargo-deny" . cargo deny --all-features --locked check
    fi
  else
    skip rust "cargo-deny" "cargo-deny is not installed"
  fi
else
  skip rust "all steps" "not selected"
fi

if wanted desktop-shell; then
  if have pkg-config && pkg-config --exists webkit2gtk-4.1 gdk-3.0 2>/dev/null; then
    step desktop-shell "cargo check" . cargo check --locked -p g9-mentor-desktop
    step desktop-shell "shell tests" . cargo test --locked -p g9-mentor-desktop --lib
  else
    skip desktop-shell "cargo check, shell tests" "webkit2gtk-4.1 / gdk-3.0 development packages are not installed"
  fi
else
  skip desktop-shell "all steps" "not selected"
fi

if wanted frontend; then
  if ! have npm; then
    skip frontend "all steps" "npm is not installed"
  elif [ "$OFFLINE" -eq 1 ]; then
    skip frontend "all steps" "--offline: npm ci needs the registry"
  else
    step frontend "npm ci" apps/desktop npm ci --no-fund
    step frontend "typecheck" apps/desktop npm run typecheck
    step frontend "lint" apps/desktop npm run lint
    step frontend "test" apps/desktop npm test -- --run
    step frontend "build" apps/desktop npm run build
    step frontend "npm audit (high+)" apps/desktop npm audit --audit-level=high
  fi
else
  skip frontend "all steps" "not selected"
fi

if wanted content-gates; then
  if python3 -I -c 'import jsonschema' 2>/dev/null; then
    step content-gates "curriculum seed" . python3 -I scripts/check_curriculum_seed.py data/curriculum/g9-ct9-480.seed.v0.json
    step content-gates "skill package contract" . python3 -I scripts/check_skill_packages.py .agents/skills
    step content-gates "skill package hashes" . python3 -I scripts/skill_hashes.py --check
    step content-gates "reference source register" . python3 -I scripts/check_source_register.py
    step content-gates "source verifier self-test" . python3 -I scripts/verify_source_register.py --selftest
    step content-gates "json schemas" . python3 -I scripts/check_schemas.py schemas .agents/skills
    step content-gates "curriculum build (generated files current)" . python3 -I scripts/build_curriculum.py
    # v2.0 courseware (ADR-0023): --available checks every week that has all five day guides (a partly written week is reported, not judged); once all 12 exist it also requires the generated files to be current
    step content-gates "curriculum v2.0 build (weeks written so far)" . python3 -I scripts/build_curriculum_v2.py --available
    step content-gates "competency packs" . python3 -I scripts/check_competency_packs.py data/competency-packs
    step content-gates "data inventory" . python3 -I scripts/check_data_inventory.py
    step content-gates "error-code catalogue" . python3 -I scripts/gen_error_codes.py
    step content-gates "document references" . python3 -I scripts/check_doc_refs.py
    step content-gates "metrics" . python3 -I scripts/gen_metrics.py
    step content-gates "backlog map" . python3 -I scripts/check_backlog.py
    # The export has no .git: the gate reads the original working tree (audit 2026-11-19, GIT-1).
    step content-gates "no ignored project file" . python3 -I scripts/check_ignored_files.py --root "$REPO"
    # Reads the tracked files of the original repository (the export has no .git); owner rule 2026-11-20.
    step content-gates "no container engine" . python3 -I scripts/check_no_docker.py --root "$REPO"
    step content-gates "gate tests" . env G9_REQUIRE_JSONSCHEMA=1 python3 -I -m unittest discover -s scripts/tests -p 'test_*.py'
  else
    skip content-gates "all steps" "python3 module jsonschema is not installed (CI pins jsonschema==4.26.0)"
  fi
else
  skip content-gates "all steps" "not selected"
fi

if wanted sbom; then
  if have cargo-cyclonedx; then
    step sbom "rust sbom" . cargo cyclonedx --format json --target x86_64-unknown-linux-gnu --all-features
  else
    skip sbom "rust sbom" "cargo-cyclonedx is not installed (CI installs 0.5.10 --locked)"
  fi
  if [ "$OFFLINE" -eq 1 ] || ! have npx; then
    skip sbom "npm sbom" "needs npx and the registry"
  elif [ ! -d "$SRC/apps/desktop/node_modules" ]; then
    skip sbom "npm sbom" "needs the frontend job's npm ci in the same run"
  else
    mkdir -p "$SRC/sbom"
    step sbom "npm sbom" apps/desktop npx --yes @cyclonedx/cyclonedx-npm@6.0.1 --omit dev --output-file ../../sbom/desktop-npm.cdx.json
  fi
else
  skip sbom "all steps" "not selected"
fi

PASSED=$((N - FAILED))
echo
echo "revision $SHA: $PASSED passed, $FAILED failed, $SKIPPED skipped (SKIPPED is not PASS)"
echo "logs: $LOGS"
if [ "$FAILED" -gt 0 ]; then
  for r in "${RESULTS[@]}"; do case "$r" in FAIL*) echo "  $r" ;; esac; done
  exit 1
fi
if [ "$KEEP" -eq 0 ]; then
  # Keep the logs; remove the export and, when it was created here, the build output.
  rm -rf "$SRC"
  case "$TARGET" in "$WORK"/*) if [ "$TARGET_EXISTED" -eq 0 ]; then rm -rf -- "$TARGET"; fi ;; esac
fi
exit 0
