# Friday practical W01-FRIDAY: a Linux service host after an unreviewed change (blind)

- Status: PROPOSED. Nothing in this file was run. This is the practical of the scored Friday, day D05, block C (13:45 to 16:15, 150 minutes) with its report in block D. The exam's Part 1 is `ASSESSMENT.md`; this file holds what Parts 2 and 3 ask and how Part 4 is built.
- Code: V2-W01-EXAM. Plan source: section 9.3 row 5 ("Linux service or permission defect"; what cannot be assumed: root access, or that one specific command will succeed; evidence-based pass criterion: reproducible misconfiguration and least-privilege retest) and section 5 week 1 day 5.
- No hint, no key, no published solution, no AI assistant. The fixture map and the faults of any instance are instructor material and are not in this file. A catalogue of the classes of fault is: it is below.
- Fixtures for this practical: `docs/curriculum/v2/WEEKS/W01/LABS/fixtures/friday/`. Do not open that folder, nor read `friday/provision_baseline.sh`, before the attempt: the SHA-256 values in `MANIFEST.md` are the commitment, and an instance you have already read is not blind.

## Why it matters

A practical scored on what you can do without the scaffold has to look like the work, and the work on a Linux host is usually this: someone changed something, two symptoms were reported, a review asks whether anything exceeds its role, and you have an unprivileged account, a policy and a clock. You need to scope first, collect facts you can source, say what the access you have does not let you see, form hypotheses that can fail, test them, find the cause at the control and not the symptom, correct with the least privilege that still works, and prove both halves with retests. Then you leave the host in a state somebody else can verify from your note alone.

The practical is blind in the plan's sense: the instance is a modified scenario that differs from the guided lab and from the independent lab. What you may not assume is part of the task: you may not have root, a tool you expect may be absent, a command you expect to work may be refused, and an unusual setting may be compliant.

## Scope and authorization

PROPOSED and unsigned. A line without a signed rules-of-engagement record is void.

```text
SCOPE W01-FRIDAY | TARGET G9-TGT-01 | CLASS E | ADDR 10.77.20.20 (the spool service and the SSH daemon are exercised only through 127.0.0.1 inside the guest) | FROM G9-TGT-01 console | WINDOW 2026-11-20T13:45/2026-11-20T16:15 America/Chicago | ALLOW read-only enumeration as the analyst account; access tests as the synthetic accounts through sudo from the administrator account, only after the written reason; changes under /srv/g9toy-spool, /etc/g9toy-spool, /etc/systemd/system/g9toy-spool.service and its drop-in directory, /etc/sudoers.d and /etc/ssh/sshd_config.d with a configuration check before every reload; one SSH probe that sends no credential | DENY every other address, any password guess or authentication attempt with a password, any real key, any change outside the listed paths, package installation, the router and the other guests, scanning of any kind | STOP STOP-01..STOP-16 | ROE <record id> | TERMS none
SCOPE W01-FRIDAY-P | TARGET DATA:w01-friday-bundle | CLASS A | ADDR none (offline text files) | FROM host text editor and the shell for hashes only | WINDOW 2026-11-20T13:45/2026-11-20T16:15 America/Chicago | ALLOW reading the files of the evidence bundle, writing your own notes | DENY running any command from the bundle on any system, any network access | STOP STOP-01..STOP-16 | ROE <record id> | TERMS none
```

The first line is the local mode (L), the second the paper mode (P). The hosted mode (H) uses a class F line for the one exercise drawn from the pool (see "How the scenario is made unseen").

Accounts of the local scenario: `labanalyst` (an unprivileged assessment account, no sudo), `labadmin` (administrator), `labuser6`, `labuser7`, `svcspool`. Evidence that the analyst account cannot read is requested from the administrator account only after you wrote down why it is needed; every such use goes in the command log. No password is ever set or used.

## Environment

| Item | Value |
|---|---|
| Guest | `G9-TGT-01`, DMZ `10.77.20.20/24`, one NIC, no route to the LAN or the Internet; the tests T8, T6, T7 and T5 current from the guided lab; nothing else runs |
| Stack | the target alone: 2 vCPU, 3 GiB, 40 GiB thin |
| Principals and groups | `labadmin`; `labanalyst`; `labuser6` (group `shippers`); `labuser7` (group `loaders`); `svcspool` (service account); the policy below |
| Service | `g9toy-spool.service`: the toy service of the guided lab in a third instance, TCP port 8100 |
| Baseline | the clean state built by `friday/provision_baseline.sh` from the read-only medium (it plants no fault) and then changed by the preparer; the preparer's record of what was changed is sealed until the report is submitted |
| Setup time, ESTIMATE, not measured | 15 to 20 minutes for the baseline, 10 minutes for the preparer's changes; neither is counted in the 150 minutes |
| Tests current | T8, T6, T7, T5 for this guest, from the guided lab, taken after the last baseline restore; if stale, the local mode is BLOCKED and the paper or hosted mode applies |
| Time | 150 minutes for Parts 2 and 3 in one block; the hash of the evidence bundle at the end of the block; Part 4 in block D |

The policy of the scenario (every mode uses it):

```text
Principals
  labadmin    administrator of the host; member of group sudo.
  labanalyst  assessment account; read-only; no sudo.
  labuser6    member of group shippers.
  labuser7    member of group loaders.
  svcspool    service account of g9toy-spool; no login shell.

Required access
  P1  The service runs as svcspool and listens on 127.0.0.1 port 8100 and on no other address.
  P2  Members of shippers may read the batches in /srv/g9toy-spool/outbox. Only svcspool creates, changes or deletes them.
  P3  Members of loaders may deliver files into /srv/g9toy-spool/inbox. They may not list, read or delete anything there, and they have no access to outbox.
  P4  SSH logins are by key only. Root may not log in. Only labanalyst, labuser6, labuser7 and labadmin may log in.
  P5  Only labadmin has sudo rights.
```

### How the scenario is made unseen

You are the learner and the instructor, so the practical is made unseen by a protocol of `docs/curriculum/v2/INSTRUCTOR_GUIDE.md` section 8; write on the scorecard which one you used, and its limit.

| Mode | When it applies | What makes it unseen | Limit to state on the scorecard |
|---|---|---|---|
| L, local guest, protocol 1 | the local lab is operational, isolation tests current, and another person can prepare it | another person applies faults from the instructor catalogue to the clean baseline, gives you the SHA-256 of their map before the clock and the scenario only at the start of the clock | depends on a second person; their mistake costs your time |
| L, local guest, protocol 3 | the lab is operational and no second person is available | a sealed fault-injection by a seed from a physical die, written outside your reach; at least 12 classes in the catalogue below with parameter spaces held in the instructor key | self-prepared: a modified scenario, never unseen; the key's author knows the catalogue |
| H, hosted, protocol 2 | the local lab is blocked | one exercise drawn by a physical die from a pool of at least five written down, dated and hashed before the week started (candidates: unplayed levels of `w01-otw-leviathan` and of `w01-otw-bandit` beyond those the days used; none a day guide uses; none whose solution you read) | partial match to the plan's row; Part 3 is a paper analysis plus tests against a small local fixture built before the attempt, labelled as such |
| P, paper bundle | no guest and no hosted exercise is possible | the evidence bundle of the folder `friday/bundle_01/`, a self-prepared fixture variant that differs from the guided and independent fixtures | one bundle only: it is a modified scenario, seen by the person who keeps the repository; scored on what is demonstrated; every claim is INFERRED or NOT RUN |

A hosted mode never runs from the local lab's guests (no guest has a route to the Internet). Platform terms must be read and dated before use.

### Catalogue of fault classes

The classes below are the classes from which the instance is drawn, one to three of them, with parameters you do not know. An instance may also hold compliant items that look unusual, and symptoms that are explained by no fault at all.

| Class | In words |
|---|---|
| C01 | the group of a file or directory differs from the role |
| C02 | a mode is wider than the role needs |
| C03 | a parent directory blocks an operation that the target's own mode would allow |
| C04 | an access control list grants more than the role, or its mask hides a grant |
| C05 | an account's group membership is missing, or present but not yet in effect for a running session |
| C06 | a service runs as an account with more reach than its role |
| C07 | a sandbox setting of the service is absent, wrong or points at a path that does not exist |
| C08 | the service listens on an address or port the policy does not allow |
| C09 | the SSH daemon's effective setting differs from the written one |
| C10 | an account's own files have a mode or owner that makes the SSH daemon refuse them |
| C11 | a delegation rule matches more than its author meant, or does not apply at all |
| C12 | a secret sits in a place an unprivileged account can read |
| C13 | the directory that should give new files the role's group does not |
| C14 | the service's state directory has a mode wider than the role needs |
| C15 | the owner of a file differs from the account the service runs as |

## Task

You hold the access of the analyst account. Two symptoms were reported (they are given to you at the start of the clock, with the clock). A review asks whether any account or process has more access than its role needs.

Part 2, practical (40 points), then Part 3, defensive validation (25 points, type F: it validates a fix or a control).

1. Scope first. Before any action write the scope block (targets and addresses, accounts, allowed actions, exclusions, time limit, stop rule), and a test plan that orders your hypotheses and says what each test would show. In the local mode confirm the identity of the target from an independent source (the guest's own inventory) before the first action.
2. Enumerate with the access you have. Every fact has a source (command, file, line) and a time with its zone. State what the analyst account cannot see and which administrator evidence you asked for and why. Do not infer a missing event, a missing process or an empty list as an absent thing.
3. Form falsifiable hypotheses before testing, with the predicted observation. Test them. Keep a negative control. Reproduce the symptom or the deviation from a clean state, or document why you cannot.
4. State the cause at the control or trust boundary, the preconditions and privileges, what the deviation allows and what it does not, the strongest alternative explanation and the test that separates them. Do not describe a symptom as a cause, a crash or a listener as an exploit, or a score as an impact.
5. Correct with the least privilege that works: for each confirmed deviation, a correction that acts on the cause, with its side effects and rollback. In the local mode apply it; in the paper and hosted modes write the exact commands as proposed text and label them NOT RUN.
6. Validate: a positive retest (the permitted behaviour still works) and a negative retest in a near-miss form (one variable changed from the test that showed the problem), each with its artifact; or, where nothing could be run, the predicted outputs written beforehand and labelled NOT RUN. Assess the hardening you chose: effect on the cause, side effects, regression risk, rollback and residual risk.
7. Report in block D: the findings in the format of `docs/curriculum/v2/TEMPLATES/PENTEST_FINDING.md` (status Confirmed, Unconfirmed, False positive or Not retested), and the baseline document `W01-Linux-Baseline-and-Hardening.md`: what the secure state of this host is, how each element is verified in one command, what would break it, and what you could not determine.

What cannot be assumed (the plan's row, and the practical's own): that you have root, that `sudo` works for the analyst, that `getfacl`, `ss -p` or `journalctl` shows what you expect to an unprivileged account, that a configured value is the running value, that every unusual setting is a fault, that a group in the database is in a running session, and that the faults of your instance match any earlier lab.

## What is scored

Marking bands and the observable items of each sub-criterion are in `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md` sections 5 to 7. In summary:

| Part | Points | Sub-criteria |
|---|---:|---|
| 2 Practical | 40 | P1 scope and design correctness 5; P2 enumeration and evidence fidelity 10; P3 independent hypothesis and reproduction 15; P4 impact and root-cause reasoning 10 |
| 3 Defensive validation (type F) | 25 | D1 correct data source and hypothesis 8; D2 positive validation 7; D3 negative testing 5; D4 hardening and mitigation assessment 5 |
| 4 Evidence and reporting | 15 | E1 reproducibility 5; E2 precision and provenance 5; E3 remediation and retest 5 |

The weekly gate is at least 80 of 100 with the safety gate and the evidence-integrity gate passed. A blocked lab is scored as demonstrated: what you did earns points, what you could not do earns none, and the block itself is not punished beyond that. Any use of a hint or a published solution puts P3 in the none band; an undisclosed use is also an integrity failure.

## Preconditions

| Id | Condition | If not met |
|---|---|---|
| F1 | a signed RoE and an approved command list for the line you use | no command is run; use the paper mode |
| F2 | the mode and its protocol are written on the scorecard before the clock starts, with the SHA-256 commitment of the preparer's map (modes L and H) | no scored attempt in that mode |
| F3 | the guest has `sudo`, `systemd`, `sshd`, `namei`, `ss`, `stat` and Python 3; a missing tool is a missing dependency | install nothing; record it as a limit; it is not a task failure |
| F4 | you did not open the instructor key, the sealed record, `friday/provision_baseline.sh` or the bundle folder | declare it; the attempt is then not blind |
| F5 | the knowledge exam's answers are locked and hashed (end of block A) | block C does not start for scoring purposes until they are |
| F6 | the attempt is begun in the window before the knowledge exam, as the Friday guide says | see the instructor guide, section 2 |

## Positive validation

Shown in your retests, not claimed: for each corrected deviation the behaviour the policy permits still works. For this scenario the permitted behaviour includes: the service runs as its account and serves the batches to the readers the policy names; loaders can deliver into the inbox; the SSH daemon accepts a configuration check and offers the methods the policy allows; the administrator keeps full administration. Each is tied to a time and an artifact.

## Negative and safety validation

Shown in your retests: the forbidden behaviour fails in a near-miss form of the test that showed it, one variable changed. An over-tight correction that fails the positive items is rejected, however many forbidden operations it stops. Safety items that hold throughout: only the listed paths were changed; no password was set or tried; no real key was created; no route, NIC or listener outside the policy appeared; every temporary change is shown reversed; the sealed record was not opened before submission. A breach of any of them is a safety or integrity failure and is handled by `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` sections 10 and 11.

## Evidence

Kept in `<evidence-folder>/w01/friday/`, hashed at the end of block C before any key is opened, with the time written down. `docs/curriculum/v2/WEEKS/W01/EVIDENCE_CHECKLIST.md` gives the pass or fail test of each.

| Artifact | Content |
|---|---|
| scope block and test plan | timestamped before the first action |
| command log | every command with target, time, privilege and status word; the administrator requests with their written reason |
| hint log | empty (there is no ladder in a scored attempt) |
| enumeration table | facts with source, time, zone; the list of what the analyst access could not show |
| hypothesis log | predictions before tests; revisions as dated added lines |
| finding records | one per finding, with status |
| correction and retest records | commands, expected and observed (or NOT RUN), side effects, rollback, artifacts |
| `W01-Linux-Baseline-and-Hardening.md` | the baseline note of step 7 |
| independence declaration | mode, protocol, hint level (none), published solution read (no), AI assistant used (no), what you did not open |
| hash list | SHA-256 of each file and of the bundle, with the time, before the key is opened |

## Rollback and cleanup

Local mode: after the evidence is copied out as text and hashed, restore the guest to its clean baseline (row R-06 of `docs/curriculum/v2/LAB_ARCHITECTURE.md` section 12.4, or discard the overlay) and run T8 again. If the baseline is not used, reverse in this order and prove each step; the preparer's sealed record is opened only after the report is submitted.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| FC1 | `sudo systemctl disable --now g9toy-spool.service` | G9-TGT-01 | stops and disables the scenario's service | the enablement link removed; the unit stops | the service is gone | sudo in the guest | this scenario's unit | `sudo systemctl enable --now g9toy-spool.service` |
| FC2 | `sudo rm -r /etc/systemd/system/g9toy-spool.service.d /etc/g9toy-spool /srv/g9toy-spool /var/lib/g9toy-spool` then `sudo rm /etc/systemd/system/g9toy-spool.service` then `sudo systemctl daemon-reload` | G9-TGT-01 | removes the unit, its drop-ins, configuration, data and state | no output | deletes only this scenario's paths | sudo in the guest | the listed paths | reprovision from the medium |
| FC3 | `sudo rm -f /etc/sudoers.d/40-spool /etc/ssh/sshd_config.d/10-g9-lab.conf` then `sudo sshd -t` then `sudo systemctl reload ssh.service` | G9-TGT-01 | removes any delegation file and the scenario's SSH file the scenario added, with the check before the reload (a name the scenario did not use is reported missing, not an error, because of `-f`) | no output from the check; the reload succeeds | the daemon runs without the scenario's file | sudo in the guest | the two paths | reprovision from the medium |
| FC4 | `sudo userdel --remove labanalyst` then `sudo userdel --remove labuser6` then `sudo userdel --remove labuser7` then `sudo userdel svcspool` then `sudo groupdel shippers` then `sudo groupdel loaders` | G9-TGT-01 | removes the synthetic accounts and groups | no output | removes three home directories | sudo in the guest | the six names | reprovision from the medium |
| FC5 | `sudo rm /etc/g9-lab-state` then `ss -ltn` then `getent passwd labanalyst labuser6 labuser7 svcspool` | G9-TGT-01 | removes the marker; proves no listener on 8100 and no scenario account | no `8100` line; the last command prints nothing and exits with status 2 | none beyond the marker | sudo in the guest, then user | the guest | none needed |

Paper mode: nothing was changed; the evidence is your notes and their hash. Hosted mode: end the platform's instance as its own rules say, and write nothing that is a flag or a solution.
