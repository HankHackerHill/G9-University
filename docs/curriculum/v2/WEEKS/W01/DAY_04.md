# G9 W01 / DAY_04 — Bash and Python: argument lists, quoting, exceptions and JSON; an inventory utility and a failed-authentication parser

- Status: PROPOSED
- Day id: D04
- Original calendar date: 2026-11-19 (Thursday)
- Duration: 8 instructional hours (R 0.5 h; A 2 h; B 2 h; C 2.5 h; D 1 h)
- Lab authorization: `D04-B`, target `DATA:w01-d04-fixtures`, class A, no address: offline copies of `LABS/fixtures/d04/` in the private evidence folder, read as an ordinary user; window 2026-11-19T08:30/2026-11-19T17:15 America/Chicago; ALLOW parsing them and writing source, tests and outputs under `<evidence-folder>/w01/`; DENY the live host as input, `sudo`, any network, any command built from log text; a live guest run is a separate class E line, the optional hosted exercise a class F line (`HOSTED:cyberdefenders:hammered`); STOP-01 to STOP-16; the RoE id is the owner's
- Dependencies: days 2 and 3; the d04 fixtures checked against `LABS/fixtures/MANIFEST.md`; Python 3 standard library only (version ASSUMED, record it); the tier of `w01-cyberdefenders-hammered` is UNKNOWN, so no gate item depends on it
- Primary provider: Python Software Foundation (documentation) | Secondary: CyberDefenders

## Objectives
1. Bash/Python, JSON, exceptions, subprocess safety, shell quoting — you state, with a harmless one-line test for each, what a shell does to a string before the program starts, why an argument list with a timeout does not, what each child failure raises or returns, and why a JSON encoder is not concatenation [python.secure-parsing]
2. Design process/user/service inventory utility; parse example failed-auth logs — a written output contract (sections, types, `errors`, exit statuses) and a line-rejection policy exist before code, and every clean fixture maps to named fields [python.secure-parsing, python.testing-fixtures]
3. Independently implement/inspect inventory outputs and errors — a utility written without a reference passes IV1 to IV10 of `EVIDENCE_CHECKLIST.md` on the clean fixtures and survives the hold-out inputs unchanged, with at least eight automated cases, five negative [python.testing-fixtures, linux.evidence-completeness]

## Hand-in
Negative tests for missing privileges, corrupted log lines and absent utilities — a report in which each case was run, with observed behaviour and exit status, accepted, rejected and ignored line counts that sum to an independent line count, and the SHA-256 of report, source and inputs; a stranger can repeat each run from it.

## Resources
| Id | Use in this day | Block |
|---|---|---|
| w01-python-subprocess | argument sequences, `shell`, `timeout`, `check`, the exceptions a run raises; a paragraph per section in your words | A |
| w01-gnu-bash-manual | quoting, order of expansions, word splitting; three cases with your prediction | A |
| w01-man7-credentials | the four IDs of a `Uid:` line; capability masks via the guest's `capabilities(7)` (`w01-man7-capabilities`) | B |
| w01-man7-ss | socket listing columns, which owners an unprivileged reader sees; your day 3 observation beside it | B |
| w01-systemd-man-index | the properties `systemctl show` prints; the guest's own manual wins | B |
| w01-thm-python-basics | hosted warm-up if Python is rusty (class F, free account, ESTIMATE 45 minutes); record sections done, no answers | B |
| w01-cyberdefenders-hammered | optional hosted attempt at real Linux logs if the tier opens (class F); record question kinds and your method; the walkthrough stays closed until you submit | C |
| w01-thm-linux-logs-investigations | optional reading on authentication and service log structure (tier UNKNOWN, never a gate item) | C |

## R — Active recall (30 min)
Answer the generated queue closed book: the two one-day questions of day 3 and the three-day question of day 1 (INFERRED from the builder's rule; the window shows the real queue); mark each against its origin key. If the queue is short, write from memory: the four numbers of a `Uid:` line and which one a permission check uses; enabled against active; what an unprivileged `ss -ltnp` omits. Then explain one failed experiment of days 1 to 3: what you did, predicted and saw, what you now think explains the gap, and which check would test that.

## A — Argument lists, quoting, exceptions and untrusted text (120 min)
Read `w01-python-subprocess` and `w01-gnu-bash-manual`, then test each claim with a probe.

Mechanism. With `shell=True` Python starts `/bin/sh -c <string>`: the shell expands the string (command substitution, parameter expansion, word splitting, pathname expansion) and honours redirections and separators before your program starts, so sender-chosen text inside it becomes syntax. A list with `shell=False` reaches `execve` as the argument vector and nothing parses it. Residual hazards of the list form: an element starting with `-` is read as an option, a bare name is searched through `PATH`, and a child such as `sh -c` or `find` interprets its own arguments. Hence absolute paths, `--` where supported, validated values, a timeout. `shlex.quote` is documented for Unix shells only: a last resort, not a design.

Failed children. A list with a missing executable raises `FileNotFoundError`; the same name through `shell=True` returns status 127 and stderr text, no exception. A non-zero exit is data in `returncode` unless `check=True` raises `CalledProcessError`. At the timeout the child is killed and `TimeoutExpired` is raised. `text=True` can raise `UnicodeDecodeError`: capture bytes. Each case becomes an `errors` entry, never an empty list that looks like a result.

Untrusted text. Read bytes, split on `b"\n"`, bound each line, decode and match each line in its own `try`, and give every physical line one outcome: accepted, rejected with a reason, or ignored with a reason (a valid line of another program is ignored, not damaged). Text-mode `open()` turns a lone carriage return into a line break and `str.splitlines()` splits on more separators than `\n`, so counts drift from `awk`. Anchor the pattern at both ends and from the right: the true address and port are the last fields whatever the name holds. A name is data: encode it into JSON, never into a shell string, path, format string or raw terminal. Syslog prefixes carry no year and no zone: keep the raw text, mark conversions ASSUMED, never subtract wall-clock times for a duration.

Guided probe (written by you, NOT RUN; predict its output first):

```python
import subprocess
v = "$(printf X)"
a = subprocess.run(["printf", "[%s]\\n", v], capture_output=True, text=True, timeout=5)
b = subprocess.run('printf "[%s]\\n" ' + v, shell=True, capture_output=True, text=True, timeout=5)
print(a.stdout.strip(), b.stdout.strip())
print(subprocess.run("g9-no-such-tool", shell=True, capture_output=True, timeout=5).returncode)
```

Then take `d04/auth_sample_01.log` only: classify lines by event kind, count failures by sender and account by hand, and note the IPv6 sender, the success after failures for one account and the names that look like syntax. Wrong models, each with its test: quoting makes `shell=True` safe (a value with a quote and a semicolon); an absent tool always raises (the probe); a non-zero status raises (`["false"]` against `check=True`); a skipped file leaves a complete inventory (compare counts with an unreadable input); `wc -l` counts records (a last line without a newline).

## B — Design the inventory contract and the log parser (120 min)
Host, ordinary user, copies of the clean fixtures only; nothing runs as root. If Python is rusty, spend the first 45 minutes (ESTIMATE) on `w01-thm-python-basics`. Compare your copies' hashes with `MANIFEST.md` first.

Write the contract before code. IV1 fixes the keys: `schema_version`, `generated_at` (UTC), `host_label`, `accounts`, `processes`, `services`, `listeners`, `findings`, `errors`; add your own `log_summary`. Decide each field's source: accounts from the passwd and group samples; processes from the status files (pid, name, four user and four group IDs, supplementary groups, effective capability mask, `NoNewPrivs`); services from the `systemctl show` sample; listeners from the socket samples (address, port, queue columns, process only if shown, whether an owner was visible). Decide statuses: 0 only if every source was read and every line accounted; one for incomplete; one for misuse; a crash is never 0. Decide the rejection policy: maximum length, pattern, reasons, what is ignored.

Validate against independent ground truth, not your code: per listener, the privileged socket sample's pid, the service's main pid and the status file's identity must agree, and a disagreement is a finding you derive, not invent; every process owner must resolve in the passwd sample; the unprivileged socket sample must give the same listeners minus owners. Count log lines by hand with `wc -l` and `awk`, then against the tool. Only `generated_at` may differ between runs.

## C — Hold-out inputs and the unchanged tool (150 min)
Scope `D04-B`: your work folder, no network, no `sudo`, nothing built from log text. A second site sends inputs you have not seen: copy `d04/auth_sample_02.log` and `d04/passwd_sample_02`, opened now for the first time; build the binary-damage log (recipe W01-R2) and an unreadable copy of the process samples (recipe W01-R4) from `LABS/fixtures/MANIFEST.md`. Before the first run write six predictions: kinds of damage, how each is counted, the expected exit status.

Success: the tool finishes on every input with valid JSON; each physical line ends in one counter and the counters sum to the independent line count; each rejection has a stable reason; hostile text appears escaped and nothing runs (a folder listing before and after shows no new file); the unreadable status file yields one `errors` entry, other sections present, an incomplete status; a missing helper yields an `errors` entry and no invented result; clean fixtures give the output they gave before. Negative control: a deliberately mutated rule must make a test fail. Rules: after the first run fix classes of defect, never one line; hash the source before and after; alone for 45 minutes, then the ladder, each hint logged with its minute. Plan (ESTIMATE): 15 predict, 45 diagnose, 60 fix and rerun, 30 hash and write up. Hosted option: `w01-cyberdefenders-hammered` if the tier opens, class F, no walkthrough; otherwise this task is the block.

## D — Negative tests, telemetry and the record (60 min)
A failed-login log records the attempted name as sent, so each line is a defender's evidence and an attacker's input. Where `auditd` runs (not installed here: PROPOSED) a tool that executed or opened log text would leave `execve` and `openat` records; here the only trace is the tool's own counters, so they must reconcile. The control is the contract: argument lists, bounded strict per-line decoding, counted outcomes, no `sudo`, an `errors` section that cannot stay empty after a failure. Positive retest: clean fixtures still give every expected field. Negative retests, each changing one variable of the positive run: an input with its mode removed, a damaged log, a helper that does not exist. Run IV1 to IV10, then write the report: per negative case the command, expected and observed behaviour, status, counts, hand count. Status VERIFIED only if your counts match an independent count, else TESTED. Hash, write the lab record, answer the eight questions closed book, add flashcards.

## Commands proposed for review
| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| 1 | `mkdir -p <evidence-folder>/w01/d04-work/in` then `cp <fixtures-folder>/d04/auth_sample_01.log <fixtures-folder>/d04/passwd_sample <fixtures-folder>/d04/group_sample <fixtures-folder>/d04/ss_sample_admin.txt <fixtures-folder>/d04/ss_sample_unprivileged.txt <fixtures-folder>/d04/systemd_show_sample.txt <evidence-folder>/w01/d04-work/in/` then `cp -r <fixtures-folder>/d04/proc_sample <evidence-folder>/w01/d04-work/in/` then `sha256sum <evidence-folder>/w01/d04-work/in/auth_sample_01.log` | host, evidence folder | copies the clean inputs, checks one hash | one hash equal to row FX-W01-13 of the manifest | writes copies | user | `<evidence-folder>/w01/d04-work` | `rm -r <evidence-folder>/w01/d04-work` after the hash list |
| 2 | `python3 --version` then `python3 -I <evidence-folder>/w01/d04-work/quote_probe.py` | host interpreter, your probe | records the interpreter; runs the quoting and absent-tool probe | `Python 3.x.y`, then your predicted two lines | starts printf and one shell | user | the work folder | none needed |
| 3 | `python3 -I <evidence-folder>/w01/d04-work/g9_inventory.py --root <evidence-folder>/w01/d04-work/in --out <evidence-folder>/w01/d04-work/run1.json` then `python3 -I -m json.tool <evidence-folder>/w01/d04-work/run1.json` | your utility, clean copies | produces the inventory, then checks it parses (IV1) | status 0, one JSON file, pretty-printed JSON | writes one file | user | the work folder | delete the file |
| 4 | `python3 -I -m unittest discover -s <evidence-folder>/w01/d04-work/tests -v` | your tests | runs the automated cases (IV10) | a line per case, a final `OK` | temporary test files | user | the work folder | none needed |
| 5 | `mkdir -p <evidence-folder>/w01/d04-work/in2` then `cp <fixtures-folder>/d04/auth_sample_02.log <fixtures-folder>/d04/passwd_sample_02 <evidence-folder>/w01/d04-work/in2/` then `wc -l <evidence-folder>/w01/d04-work/in2/auth_sample_02.log` then `awk 'END { print NR }' <evidence-folder>/w01/d04-work/in2/auth_sample_02.log` | hold-out copies | copies them, counts lines two ways | two numbers; write why they may differ | writes copies | user | the work folder | `rm -r <evidence-folder>/w01/d04-work/in2` |
| 6 | `cp -r <fixtures-folder>/d04/proc_sample <scratch-folder>/proc_copy` then `chmod 000 <scratch-folder>/proc_copy/101/status` (recipe W01-R4) | scratch folder outside the course source | builds the missing-privilege case | no output | one file unreadable to its owner | user | `<scratch-folder>` only | `chmod 600 <scratch-folder>/proc_copy/101/status` then `rm -r <scratch-folder>/proc_copy` |
| 7 | `python3 -I -c "import sys; p=sys.argv[1]; data=b'Nov 19 10:00:01 tgt sshd[3001]: Failed password for invalid user admin from 203.0.113.7 port 40022 ssh2\n' + b'Nov 19 10:00:02 tgt sshd[3002]: Failed password for invalid user \xff\xfe from 203.0.113.7 port 40023 ssh2\n' + b'Nov 19 10:00:03 tgt sshd[3003]: Failed password for invalid user ad\x00min from 203.0.113.7 port 40024 ssh2\n' + b'Nov 19 10:00:04 tgt sshd[3004]: Failed password for invalid user admin from 203.0.113.7 port 40025 ssh2\r\n'; open(p, 'wb').write(data)" <evidence-folder>/w01/d04-work/in2/auth_sample_03_binary.log` (recipe W01-R2) | evidence folder | writes a four-line log with invalid UTF-8, a NUL and a carriage return | a file of about 400 bytes | writes one file | user | the work folder | delete the file |
| 8 | `sha256sum <evidence-folder>/w01/d04-work/g9_inventory.py <evidence-folder>/w01/d04-work/tests/*.py <evidence-folder>/w01/d04-work/run*.json` | your files | the hash list with its time, before any key is opened | one line per file | none | user | the work folder | none needed (read-only) |

## Offense, detection, remediation, retest
| Offensive mechanism | Defensive observation | Remediation | Retest |
|---|---|---|---|
| A sender picks the attempted name (syntax characters, a path, a tab, thousands of letters) so a collector executes, opens, mis-splits or exhausts on it | the log keeps the name verbatim as data; a collector that acted on it would leave process and file-open records where `auditd` runs (PROPOSED); the counters show the line | argument lists, no shell; the name is escaped JSON data; bounded lines; strict per-line decode with a counted reason | both logs through the tool in an empty scratch folder: listing unchanged, names escaped, counters add up |
| Damaged, truncated or binary input to crash a collector or hide records | the gap between the independent line count and the counters | one outcome per line; the sum of outcomes equals the total, asserted in a test | hold-out and binary logs: counters add up, reasons stable, no exception escapes |
| An absent or failing helper becomes a clean-looking empty inventory | an `errors` entry, an incomplete status, units with main pids but no listeners | handle `FileNotFoundError`, `PermissionError`, `TimeoutExpired`; no default empty section | a name that does not exist: `errors` filled, other sections present, incomplete status |
| The tool run as root, so a parsing flaw acts as root and the output hides what an unprivileged observer cannot see | the tool prints its effective identity and `owner_visible`; the source has no `sudo` | run unprivileged, state what was not seen | run as an ordinary user; search the source; compare the two socket samples |

## Quiz
### Recall (closed book)
1. [W01D04-R1] Why do malformed input and least privilege both matter to a small inventory script that parses a failed-authentication log and reads process, account and socket data? Give one concrete failure of the output for each if it is ignored.
2. [W01D04-R2] A value taken from a log is placed in a command that Python starts. State what changes between building one string for `shell=True` and passing an argument list, and name one hazard that remains with the list form.
3. [W01D04-R3] For a child started through an argument list with a timeout, say what Python does when the executable does not exist, when the program runs and exits with status 1, and when it outlives the timeout, and what the inventory should record for each.
4. [W01D04-R4] An inventory writes its output by concatenating strings around a user name taken from a log. Give a name that changes the meaning of the output, and say what a JSON encoder does with it.
5. [W01D04-R5] A classic syslog line begins with `Nov 19 08:02:13`. List what that prefix does not tell you and say how the inventory should store and use it.

### Applied
1. [W01D04-A1] A parser splits each failed-password line on whitespace and takes the account as token 8 and the sender as token 10 (counting from 0; date, time, host and `sshd[pid]:` are tokens 0 to 4). From these three lines, prefix omitted, it prints `root 198.51.100.77 1`, `invalid admin 1`, `invalid web 1`: `Failed password for root from 198.51.100.77 port 41001 ssh2`; `Failed password for invalid user admin from 198.51.100.77 port 41002 ssh2`; `Failed password for invalid user web admin from 198.51.100.78 port 41004 ssh2`. Explain each row, give the correct table, describe a parse that a sender-chosen name cannot shift, and name the test that would have caught the defect.
2. [W01D04-A2] A collector runs `subprocess.run('ss -H -ltnp', shell=True, capture_output=True, text=True)` and parses stdout. On a minimal guest it saved `{"listeners": [], "errors": []}` with exit status 0, while the saved stderr reads `sh: 1: ss: not found` and the returncode is 127. The same inventory lists a unit with `ActiveState=active` and `MainPID=412`. A reviewer concludes that the guest listens on nothing. Name the two defects in the collector, say what the output and exit status should have been, and describe the test that would have caught it.
3. [W01D04-A3] A parser reports `lines_read=412 accepted=391 rejected=14` for a log, and its author calls 14 rejected lines acceptable. You count the same file independently: `wc -l` prints 412 and `awk 'END { print NR }'` prints 413. Say how many lines each discrepancy concerns, what kind of line each is likely to be, why the quoted percentage means nothing yet, and which invariant a test must assert.

## Hint ladder
1. No hint: for 45 minutes alone, write the contract from the clean fixtures, implement the reader so every physical line ends in one counter, run it unchanged on the hold-out inputs, and keep your predictions and the minute you stopped.
2. Conceptual hint: look at every place where your code decides that a line or file is nothing; a branch that moves on without a record or a count is where evidence disappears, and total lines must equal the sum of outcomes.
3. Scoped technical hint: read bytes and split on the newline byte; give each line's decode and match its own `try` and count the reason; bound length before matching; open each status file in its own `try` so one `PermissionError` is one `errors` entry; run helpers with an argument list, a timeout and handlers for `FileNotFoundError` and `TimeoutExpired`.
4. Instructor explanation: in the answer key, after the attempt is submitted.

## Troubleshooting and rollback
- Expected: valid JSON on standard output, ten acceptance tests green, counters equal to the independent counts, statuses as your contract says, no stray files.
- Missing dependency: if `python3` is absent install nothing; record `command -v python3`, do design and tests as text, label the run NOT RUN and the day BLOCKED for it; a missing `awk` or `sha256sum` is a recorded limit.
- No connectivity: the day needs no network; a hosted exercise is a browser session under a class F line, skipped without a workaround or VPN profile if unreachable.
- Unexpected result: compare with the independent counts before suspecting the fixtures; a hash that differs from the manifest means the copy changed (STOP-08); a one-line difference between counters usually has a stated cause, such as a last line without a newline.
- Safety breach: a file created outside the work folder, a command built from log text or the tool run with `sudo` is a stop (STOP-01): preserve the listing and command log first, record the incident; the inputs are synthetic.
- Restoration: after the hash list is saved, remove the work copies, restore the mode of the unreadable copy and remove it (rollback of command 6), keep source, tests, runs and report.
- Time exceeded: at 150 minutes submit what exists, mark each missing test NOT RUN and each unmet IV item failed; do not extend into Friday.

## Evidence and acceptance
- Evidence: source, at least eight automated cases, saved runs on clean and hold-out inputs, the negative-test report, independent counts, hash list with times, hint log, independence declaration.
- Grading: IV1 to IV10 each pass or fail from the files alone; the hand-in is accepted when the three negative cases ran with observed behaviour and statuses and the counters equal the independent counts; a hinted result is guided evidence.
- Negative test: mutate one rule (remove the end anchor, or let a branch skip without counting) and show a test fails, then restore it; a suite that stays green under the mutation does not separate apparent from verified success.

## Expert extension (optional)
Write a seeded mutation generator with the standard library only (`random.Random`, recorded seed) that flips bytes, truncates lines and inserts control characters into copies of the clean log, and assert on every mutant that no exception escapes and the outcomes sum to the total. Or write the same reader in Bash with `read -r` and compare counters, noting how a final line without a newline behaves. Drop it if time is short.

## Spaced recall
- 1 day: W01D04-R1, W01D04-R2
- 3 days: W01D04-R3
- 7 days: W01D04-R4
- 14 days: W01D04-R5

## Reflection
- What was observed and what was inferred?
- What was not proven?
- Which test would falsify the leading hypothesis?
