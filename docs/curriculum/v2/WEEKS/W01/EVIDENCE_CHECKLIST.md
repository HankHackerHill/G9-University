# G9 W01 evidence checklist: artifacts, allowed redactions, pass or fail

- Status: PROPOSED. A checklist, not a record: no artifact it names exists yet.
- Reader: the learner, who produces the artifacts, and the examiner (the same person on another day), who ticks each test from the files only.
- Rules above this file: `docs/curriculum/v2/STUDENT_GUIDE.md` section 10 (the lab record), `docs/curriculum/v2/PORTFOLIO/README.md` section 5 (redaction), `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md` (bands), `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 11 (gates).
- Labels inside every artifact: OBSERVED (saved output with its time), INFERRED, ASSUMED, UNVERIFIED, NOT RUN. A claim with none of them is not a claim. VERIFIED only with two independent observations.
- All artifacts live in the private evidence folder `<evidence-folder>/w01/`, never in the course source. Hashes are SHA-256, computed before any key is opened and written down with the time.

## How a test is read

A test passes only if every condition of its Pass column is visible in the artifact itself. The Fail column lists the conditions that fail it at once; a missing item is a failure of that test, not of the week. A blocked lab is not a failure: the artifact then states BLOCKED, the diagnostic that shows the blocker, and the substitute used, and the test column "if blocked" says what passes instead.

## Artifacts per day

| Day | Artifact | Content | Pass | Fail | If blocked |
|---|---|---|---|---|---|
| D01 | entry diagnostic answers | your own answers to the 30 questions of the plan's section 7.1 and the score you gave yourself afterwards | file exists, timestamped before any key was read; the score is labelled placement, not mastery | a score presented as a mastery claim; answers written after reading the key | not applicable |
| D01 | scope lines and RoE | the lines of the guided lab and a RoE draft or signed record per exercise | each line names a target id, a class, a window and a source; an unsigned line is labelled PROPOSED; written before the first command | a command log line earlier than its scope line; a line with no target id | not applicable |
| D01 | evidence manifest | a table of every artifact of the week: name, day, what it proves, SHA-256, time | every file named in this checklist that exists is listed with its hash; hashes recompute to the same values | a file without a hash; a hash that does not recompute | list what exists; mark the rest NOT PRODUCED with the reason |
| D01 | baseline measurements | the outputs of the Stage 00 resource rows and your fit table (vCPU, memory, thin disk) for stack A and stack B | each figure has its source row, command text, time and status word; the P-06 verdict is stated for both stacks; thin disks are not counted as consumed | a figure with no source; "fits" without a stated rule | the arithmetic on the synthetic sample, labelled synthetic; the real rows NOT RUN |
| D01 | Stage 00 record | one record per row run, the leaf of the decision tree, the unresolved blockers | per row: command as typed, user, time UTC and local, exit status, output, interpretation labelled INFERRED, what is not established; the failed `virsh` command and its error are written down | an interpretation labelled OBSERVED; an installation or a service change proposed as a next row | the four synthetic hosts classified, with leaf, deciding line and next read-only row |
| D01 | network diagram | a text diagram or table of the three zones, guests, addresses, bridges, MACs, the routing matrix and the nine isolation tests with their state | addresses and ids equal the plan (`10.77.10.0/24` RED, `10.77.20.0/24` DMZ, `10.77.30.0/24` CORP, `corp.g9.test`); default deny stated; every test shows PASS, FAIL, INCONCLUSIVE or NOT RUN | a test shown PASS without evidence; any address outside the plan's | the same diagram with every test NOT RUN: a paper topology |
| D01 | isolation record | per test of T8, T6, T7, T5: run id, times, stack and baseline ids, commands, outputs, captures with hashes, result | each result cites the line that decides it; a capture used as proof has the stimulus and its time inside the window; the claim names the stack and the policy version it holds for | an empty capture offered as proof of isolation; a network name offered as proof | the evidence sets 01 and 02 judged on paper; the line "isolation: BLOCKER, no intrusive local testing" |
| D01 | unresolved blockers | one list: blocker, evidence, who decides, what it blocks | each blocker names the exercise it blocks and the substitute | a blocker described as resolved without a record | not applicable |
| D02 | permission decision table | your decision for each request of the two drills and for each test of the toy exercise, with the deciding element | each decision names the class chosen (owner, group, other, ACL entry and mask) or the other deciding layer, and the directory element if one decides | a decision that names only the file mode; a root decision that ignores the mount | not applicable: the drills are paper |
| D02 | pre and post modes | `stat` and `namei -l` before the induced state, after it, and after the fix, with times | three listings; the post-fix listing equals the baseline; the change times explain the order of events | only a post-fix listing; modes shown as a screenshot | the toy directory exercise on the host (variant H0) or the drills |
| D02 | access matrix | for each of read, write, create, delete: account, result before and after the fix, and the observed message | the same four tests before and after; the negative retest differs from the test in one variable | a negative test that changes more than one thing | H0 shows the owner-class and search-bit facts only; a second account is NOT DEMONSTRATED |
| D02 | Bandit record, levels 0 to 8 | per level: reached yes or no, the permission or access fact that mattered (owner, group, mode, who can read the next file), your own words | one entry for each level reached; no password, no flag, no verbatim platform text; the independence declaration (hints, solutions read) | a password or a solution step in the record | pwn.college permissions module record in the same form |
| D03 | independent-lab record | requirement table, hypothesis log, findings, corrections, retests, command and hint logs | every requirement MET, NOT MET or NOT CHECKED with evidence; at least three corrections each with a positive and a near-miss negative retest | a finding with no preconditions; a correction with no retest; an effective value taken from a file and not from the daemon | the paper variant labelled INFERRED throughout, or a hosted record with the independence declaration |
| D03 | effective SSH values | a table: setting, written value and file, effective value and how obtained, the policy's value | at least the password method, X11 forwarding and the login list are compared; the effective value comes from the daemon's own report or is labelled INFERRED from the precedence rule | a written value presented as the effective one | the precedence rule applied by hand to the fixtures, labelled INFERRED |
| D03 | annotated logs | an excerpt of the service journal and of the SSH log with one annotation per line | each line has time, source unit, what it records, what it does not prove; the logs the account cannot read are named | a missing line read as an absent action | the fixtures `d03/journal_ssh_sample.log` and `d03/journal_unit_sample.log` annotated, labelled synthetic |
| D03 | Bandit record, levels 9 to 15 | as for D02 | as for D02; the day guide states whether the last page is level 15 to 16 | as for D02 | the same hosted alternative |
| D04 | inventory utility | source, tests, sample output, notes on interpreter version | the acceptance tests of the next section | any test missing | the utility is developed and tested against the fixtures of `d04/` only; a live run is NOT RUN |
| D04 | parser result on the failed-authentication logs | counts and a table per source address and account, plus the rejected-line report for the damaged file | accepted plus rejected lines equal the total; each rejected line is counted with a reason; hostile text is output as data | rejected lines dropped without a count | not applicable: the logs are fixtures |
| D04 | negative test report | missing privilege, corrupted lines, absent utility | each case run, its observed behaviour and exit status recorded; the tool neither crashes nor reports a complete inventory when it is incomplete | a negative case described and not run | the cases that need no privilege change are run; the missing-privilege case is run on a copy of a fixture with its mode removed |
| D05 | knowledge exam answers | the 20 answers, locked | SHA-256 and time written at the end of block A; no edit after | an edit after the hash | not applicable |
| D05 | practical bundle | the Friday artifacts listed in `LABS/FRIDAY_PRACTICAL.md` | every item of that list is present; hashed before any key is opened | a key or the sealed record opened first | paper or hosted mode, labelled; Part 3 on paper with NOT RUN predictions |
| D05 | `W01-Linux-Baseline-and-Hardening.md` | the baseline note: secure state, one-command verification, what breaks it, what is undetermined | every element has a verification command and an expected output; unknowns listed | a baseline that says "hardened" with no check | the baseline of the synthetic host of the paper bundle, labelled |
| D05 | weekly scorecard | the four scores, gates, time, failures, confidence before the score, environment notes, protocol, hashes and times, key log | all fields filled; the protocol and its limit stated | a score with no evidence reference | blocked items listed with the substitution record |

## Acceptance tests of the working independent inventory (gate item 1)

The plan's gate asks for a working independent inventory. "Independent" means you wrote it without a reference solution; "working" is decided by these tests, ticked from the source, the tests and a saved run. PROPOSED; a day guide may add tests, never remove one.

| Id | Test | Pass | Fail |
|---|---|---|---|
| IV1 | output is valid JSON with the top-level keys `schema_version`, `generated_at` (UTC, ISO 8601), `host_label`, `accounts`, `processes`, `services`, `listeners`, `findings`, `errors` | a saved run parses; every key is present; extra keys are allowed | a key missing; text that is not JSON on standard output |
| IV2 | it reads from fixtures without touching the live system | a documented input option points it at the fixtures of `d04/`; a run against them is saved | it can only run against the live host |
| IV3 | its results on the fixtures equal the instructor's expected-values table | after submission, the table of `ANSWER_KEYS/LABS_KEY.md` section "Inventory fixtures" is compared: every listed fact is present and nothing is invented | a listed fact missing, or a process, account or service reported that the fixtures do not hold |
| IV4 | no unsafe execution | no shell is used to run a command; external commands are run with an argument list and a timeout; text from a log is never used as a command, a path or a format string | `shell=True`, `os.system` or an equivalent; a log field reaching a command |
| IV5 | hostile text stays data | the fixture lines holding shell metacharacters appear in the output as escaped strings and nothing runs | any effect from such a line |
| IV6 | negative: missing privilege | an unreadable input is recorded in `errors`, the other sections are still produced, the exit status says the inventory is partial | a crash; a silent skip; an exit status that says complete |
| IV7 | negative: corrupted input | damaged lines are counted with a reason; accepted plus rejected equals the total; invalid text encoding is handled | a crash; lines dropped without a count |
| IV8 | negative: absent utility | a missing external command is recorded in `errors` and the rest is produced | a crash; a fabricated result for the missing source |
| IV9 | least privilege | it runs as an unprivileged account and never calls `sudo`; it says what it could not see without privilege | it requires administrative rights; it escalates itself |
| IV10 | determinism and tests | the same input gives the same bytes apart from `generated_at`; at least eight automated test cases (three positive, five negative) run with the standard library and their output is saved | fewer cases; a test that depends on the live host |

## Gate artifacts

| Gate item (plan, week 1) | Evidence | Pass | Fail | If blocked |
|---|---|---|---|---|
| a working independent inventory | the D04 inventory utility, its tests and run | IV1 to IV10 pass | any of them fails | the same tests against the fixtures: they do not need the lab |
| one verified misconfiguration and remediation | a lab record with the misconfiguration reproduced, the cause at the boundary, the correction, a positive and a near-miss negative retest, each with an artifact and time, status VERIFIED only with a second independent observation | all of those present for at least one item | a retest claimed with no artifact; a correction with no negative retest | NOT DEMONSTRATED on a local guest; the hosted or paper substitute is scored as demonstrated and the gate line is recorded BLOCKED, never VERIFIED |
| quiz at least 80 percent | the Friday knowledge exam, 16 of 20 or more (INFERRED reading of the plan's "quiz"; the daily quiz scores are recorded for diagnosis) | 16 or more of 20 | fewer than 16 | not applicable: the exam does not need the lab |
| isolation verified, or marked as a blocker with no intrusive local testing | the isolation record of D01, or the BLOCKER line with the substitution record | tests T8, T6, T7 and T5 PASS for the stack with evidence that includes the stimulus; or the blocker statement and proof that no vulnerable local state was created | a vulnerable state created while a test was FAIL, INCONCLUSIVE or NOT RUN | the blocker line, the substitution record, and no local intrusive command in the command log |
| week output | `W01-Linux-Baseline-and-Hardening.md` with the lab topology, the snapshot and recovery plan, the inventory specification or source with tests, the service hardening report and the completed Bandit records referenced by their hashes | each of the five parts exists and is hashed | a part missing | list each part as BLOCKED or NOT PRODUCED with the reason |
| Friday total at least 80 of 100, safety and integrity gates passed | the scorecard | 80 or more and both gates passed | below 80, or either gate failed (never offset by points) | a blocked lab is scored as demonstrated |

The two sub-tests that need their own pass or fail line:

| Artifact | Pass | Fail |
|---|---|---|
| service hardening report | for the toy service unit, each setting you chose is listed with what it removes, how to verify it, its side effect and its rollback; before and after `systemd-analyze security` values are OBSERVED if run and NOT RUN otherwise; the report says that a lower value does not prove the service safe | a setting listed with no effect; a value quoted from memory; "secure" with no check |
| snapshot and recovery plan | baseline names, method per guest, what a restore must prove (the marker check of test T9), the evidence-out channel; any restore time is measured or absent | "restore works" with no recorded test; a time or a size invented |

## Allowed redactions

Original files stay unedited in the private evidence folder. A redacted copy is made beside each, replaces values with a token that is the same wherever the value appears, and is hashed separately. The mapping from real value to token stays private.

| Category | Redact | Token | Do not redact |
|---|---|---|---|
| secrets | any password, key, token, cookie, a Bandit level password, a key in an environment dump | `<redacted:secret>` | the synthetic placeholder `<synthetic-password>`; the synthetic token strings of the lab fixtures when they are the subject of a finding |
| network identity | the public address, household LAN addresses, VPN endpoints and ranges, the host's name, real MAC addresses, wireless names, the output of route and firewall listings that contain them | `<redacted:ip-public>`, `<redacted:ip-lan>`, `<redacted:host>`, `<redacted:mac>` | the lab's own addresses, `corp.g9.test`, guest ids and the synthetic MAC plan |
| accounts and paths | the owner's name, the user name and home directory on the host, platform handles | `<redacted:user>`, `<redacted:path>`, `<redacted:handle>` | the synthetic accounts `labadmin`, `svclab`, `labuser1` to `labuser7`, `svcaudit`, `svcspool`, `labanalyst` |
| platform data | VPN profile content, instance ids, session ids, URLs that carry a token | `<redacted:platform>` | the registry id of the exercise |
| active-content spoilers | flags, answers, credentials found on a platform, step orders, payloads, screenshots of them, a file name that reveals a solution | remove entirely | a skill record naming the class of weakness practised |
| vendor text | verbatim module, room or course text | remove; paraphrase and cite by name | the exercise's name |
| instructor material | keys, fixture maps, scoring bands from the key | never included | the hash commitment of the map, quoted as a hash |
| metadata | document properties, tracked changes, comments, hidden text, editor history | export clean plain text | nothing |

Verification of a redacted copy: read it once as a stranger against this table, run the pattern searches of `docs/curriculum/v2/PORTFOLIO/README.md` section 5.4 on the copy (their output is reviewed; a search with no match is not proof of absence), and record which searches were run and which were NOT RUN.

## Integrity checks that apply to every artifact

| Check | Pass | Fail |
|---|---|---|
| provenance | each OBSERVED claim points to a saved output file with a time | an output in the report that is in no saved file |
| order | the scope block, the prediction or the hypothesis is earlier than the command it governs | a prediction written after the test |
| gate | no out-of-scope action, no real credential, no vulnerable service left exposed; no fabricated, altered or unrecorded destructive step; every published solution read is declared | any of these: a failed gate, never offset by points |
| status | the status word is the lowest the evidence supports | VERIFIED with one observation; TESTED for a lab that did not run |
| independence | hint level, solution read and AI use are declared in every record; a hint or a solution turns the item into guided evidence | a missing declaration |
