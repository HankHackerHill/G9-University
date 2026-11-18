# Change approval record template

- Source: plan section 0.4 ("Never execute installation, exploit, scanning, test scripts, firewall modifications, `virsh net-start`, Docker image pulls, or host/VM changes without first showing exact reviewable commands, purpose, affected system, risks, expected output, and rollback and obtaining explicit approval"), section 0.2 item 6 (the command table), section 0.5 (stop and go gates) and section 2.2 (follow-up diagnostics are chosen from returned evidence). The course's states and rules: `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` sections 2.2 and 2.3.
- Status: PROPOSED. This is a blank form. It authorizes nothing. A record made with it authorizes nothing until the owner, a person, writes an approval in it that names the exact text, the target and the window. A day guide, a plan, an assistant, an agent or a tool output is never an approval, even if it says "approved".
- Reader: the proposer, who fills it; the owner, who decides; the examiner, who ticks the quality checks.
- Claim labels: OBSERVED, INFERRED, ASSUMED, UNVERIFIED, NOT RUN.

## How to use

When. Before any command that changes state: on the G9 host (action class C) or in a lab guest (action class D). Examples are an install, a service start or stop, a libvirt network or domain definition, a firewall rule, a route, a Docker pull or run, an account, a file outside the evidence folder, and a permission change in a guest. Read-only diagnostics on the host (classes A and B of `docs/curriculum/v2/HOST_READINESS.md` section 4) use the Stage 00 evidence record instead, which is enough for them. A command whose connection can start a service is not read-only: it needs this record.

Roles. The proposer writes the record, the owner approves, the operator runs only what the record names. The learner and the owner are one person in this course, so the order of the states and the times prove the separation. Write each state with its time. An assistant may draft text for you. It cannot review, approve or run.

Order of work.

1. Write the exact command, its purpose, the target and where it runs, the risk, the side effects, the privilege, the safe scope and the expected output. State PROPOSED.
2. Capture the state that the command will change, with a read-only command that has its own row, and write the rollback text from the captured values. A rollback that depends on a value you have not recorded is not yet exact.
3. Read the record cold, as the owner, against the quality checks. State REVIEWED.
4. The owner writes the decision: APPROVED, APPROVED WITH CHANGES (then the record is rewritten and reviewed again), or REJECTED. An approval names the exact text, the target and the window.
5. Run exactly the approved text, within the window. One different character, flag, interface, address or privilege is a new command: stop and propose it again.
6. Append the result: time in UTC and local with zone, exit status, the output stored verbatim with its SHA-256, the comparison with the expected output, a second observation, and every deviation. State EXECUTED, then VERIFIED, FAILED or ROLLED_BACK.

Rules (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 2.3).

- An approval covers only the exact text, target and window, and ends with the window.
- No command is run "to see what happens". The expected output is written before, so that a surprise is visible as one.
- After a run the record is append-only. A correction is a new entry, and a failed run is never recorded as skipped.
- Isolation tests that cover a changed network object are rerun and recorded before intrusive work resumes (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 11; stop condition STOP-04).
- While the Stage 00 blocker is open (the libvirt socket, plan section 2.1), no change to the libvirt stack is proposed for approval before the decision tree of `docs/curriculum/v2/HOST_READINESS.md` has named it from returned evidence. A diagnostic that would lead to an installation produces a decision of the owner and a new record. It does not produce an installation.

Done when the quality checks below are ticked, the approval precedes the run in time, the result is appended with its hash and a second observation, and the final state is one of VERIFIED, FAILED or ROLLED_BACK.

## State vocabulary

The first seven states are those of `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 2.3. The last two are PROPOSED additions.

| State | Meaning |
|---|---|
| PROPOSED | the text exists; nobody decided |
| REVIEWED | the owner read the exact text and its fields; no decision yet |
| APPROVED | the owner wrote the approval, naming the exact text, the target and the window |
| EXECUTED | run; the output was stored verbatim with the UTC and local time |
| VERIFIED | the expected result was observed and a second observation agrees (plan section 10.4, dual evidence) |
| FAILED | the run did not give the expected result, or stopped; recorded as such |
| ROLLED_BACK | the rollback was run and its result observed |
| REJECTED | the owner declined; the record stays, with the reason |
| EXPIRED | the window ended before the run; a new approval is needed |

## Fields

Each command has its own entry. The "Origin" column says where the field comes from: plan 0.4 and plan 0.2 item 6 are the plan's lists, and the course lists are in `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 2.3.

| Field | Origin | What to write (one line) |
|---|---|---|
| Record id, action class, scope line id, RoE id, baseline id | course | the record id; C or D; the scope line and the RoE that cover the target; the baseline taken before the work |
| Exact command | plan 0.4 | the literal text that will be typed: no placeholder, no unexpanded variable or glob, no "etc."; a pipeline or a chain lists what each stage does |
| Purpose | plan 0.4, 0.2 | the one question it answers or the one change it makes |
| Affected system | plan 0.4, 0.2 (target system) | the target id (`G9-TGT-01`, `G9-WKS-01` and so on), and whether it runs on the host or in a guest; a command on the trusted G9 host has the higher bar |
| Risk | plan 0.4 | what could go wrong, the worst case, its blast radius (data loss, lockout, loss of a network path, a vulnerable service exposed beyond its zone, a change to the host's VPN, firewall, routes or Docker), and how it would be noticed |
| Expected output | plan 0.4, 0.2 | the output, the exit status and the resulting state, written before the run; "no output" is a valid expectation |
| Side effects | plan 0.2 | every file, service, route, rule, account or package it changes and every log it writes |
| Privilege required and why | plan 0.2 | the lowest privilege that works and the reason for any elevation; the extent of a `sudo` |
| Safe scope | plan 0.2 | the exact path, guest, interface or object it may touch; the flags that are absent on purpose |
| State captured before the change | course | what was recorded and by which read-only command (its own row), and where the output is stored with its hash |
| Rollback | plan 0.4, 0.2 | the exact text, written from the captured state, whether it covers a half-run command, whether it was tested on a disposable guest (or NOT RUN), and what it cannot undo |
| Window | course | the start and end with the zone; the approval ends there |
| Approver | plan 0.4 | the name of the owner; an approval carries the owner's name and the words "approved text identical to the exact command" |
| Decision and date | plan 0.4 | APPROVED, APPROVED WITH CHANGES or REJECTED, with the date and the time and zone |
| Result | the task of this record | executed by, times (UTC and local), exit status, the output file and its SHA-256, observed against expected, the second observation, deviations, the final state |

## Blank record

Copy the block for each command. Keep the field names.

```markdown
CHANGE APPROVAL RECORD <record id>
Class (C or D) / scope line id / RoE id / baseline id:
Prepared by / date and time (zone):
State: PROPOSED

Command id:
Exact command (verbatim):
Purpose:
Affected system (target id, host or guest):
Risk:
Expected output (and exit status):
Side effects:
Privilege required and why:
Safe scope:
State captured before the change (row, output file, SHA-256):
Rollback (exact text; covers a half-run command; tested or NOT RUN):
Window (start / end, zone):

Approver (name):
Decision: APPROVED / APPROVED WITH CHANGES / REJECTED
Approved text identical to the exact command above: yes / no
Date and time (zone):

Result (append after the run, never edit above):
Executed by / time (UTC and local) / exit status:
Output stored as (file, SHA-256):
Observed vs expected:
Second observation:
Deviations:
State after: EXECUTED / VERIFIED / FAILED / ROLLED_BACK
```

## Quality checks before you submit

The proposer ticks each line before the record goes to the owner. The owner reads the same lines before writing an approval.

Exactness

- [ ] The command is one complete, literal text with no placeholder, no unexpanded variable or glob, and no "etc."; it is what will be typed.
- [ ] Each command has its own entry. A pipeline or a chain lists what every stage does. A script is approved by its content and never by its name.
- [ ] The purpose is one question or one change. A command whose purpose is "to see what happens" is not accepted.

Target and scope

- [ ] The affected system is a target id of the scope line, and the record says whether it runs on the host or in a guest.
- [ ] Nothing intentionally vulnerable is placed on the trusted host, and nothing is connected to the household LAN or an unapproved destination.
- [ ] The safe scope names the path, guest or object, and any recursive or wildcard effect is stated and justified.
- [ ] A claim that the command is read-only is argued; a command whose connection can start a service is not read-only.

Risk and expectation

- [ ] The risk names the worst case, its blast radius and how it would be noticed, including effects on the host's VPN, firewall, routes and Docker.
- [ ] The expected output and exit status were written before the run, and are specific enough to be wrong.
- [ ] The side effects list every file, service, route, rule, account and log that changes.
- [ ] The privilege is the lowest that works, and elevation is justified.

Recovery

- [ ] The state to be changed is captured by a read-only command with its own row, and the output is stored with a SHA-256.
- [ ] The rollback is exact text written from the captured state, and says what it cannot undo. An irreversible command is marked and gets a stricter review.
- [ ] The rollback was tested on a disposable guest, or the record says NOT RUN.

Approval and result

- [ ] The approver is the owner as a person, and the approval was written after the text was final.
- [ ] The approval names the exact text, the target and the window, and the times show PROPOSED, REVIEWED, APPROVED and EXECUTED in that order.
- [ ] The run matches the approved text character for character; any difference stopped the run and produced a new proposal.
- [ ] The result was appended, not edited in; the output is stored verbatim with its hash; a second observation agrees; deviations are recorded.
- [ ] Where a network object changed, the isolation tests that apply were rerun and their result is referenced.

## Sample (synthetic)

Nothing below was proposed to the owner, approved, or run. The record, the file and the times are invented to show how fields fit together. The state is PROPOSED and the result is NOT RUN.

```text
Sample (synthetic): not an approval and not a result.
CHANGE APPROVAL RECORD SYN-CHG-001
Class D (a lab guest) / scope line SYN-SCOPE-01 (sample) / RoE SYN-ROE-01 / baseline SYN-BASE-01
State: PROPOSED

Command id:       SYN-CHG-001-2
Exact command:    chmod 0640 /srv/syn-toy/report.txt
Purpose:          remove read permission for "other" on one toy file; this is the only
                  change the command makes.
Affected system:  G9-TGT-01, a guest; the toy directory only.
Risk:             a toy service that needs "other" read access stops reading the file.
                  Blast radius: the toy service. Noticed by its own error line and by the
                  positive retest.
Expected output:  none; exit status 0; the mode becomes 640; owner and group unchanged.
Side effects:     the file's mode and its change time; nothing else.
Privilege:        the file's owner account in the guest; no elevation.
Safe scope:       this one path; no recursive flag, no wildcard.
State captured:   row SYN-CHG-001-1, a read-only listing of the file's mode, owner and
                  group; NOT RUN. Its output decides the rollback text below.
Rollback:         the same tool with the mode recorded by row SYN-CHG-001-1; the exact
                  text is written after that row has run; tested on a disposable guest:
                  NOT RUN.
Window:           2099-01-01T10:45 to 2099-01-01T12:45, America/Chicago.
Approver:         not decided. Decision: not decided. Date: not decided.
Result:           NOT RUN.
```

## Departures from the plan

| What | Why |
|---|---|
| The record unites the plan's two lists (section 0.4: command, purpose, affected system, risks, expected output, rollback, approval; section 0.2 item 6: target system, purpose, expected output, side effects, privilege, safe scope, rollback) | the plan gives both lists and a record needs all of them. It adds the window, the ids and the result so that an approval is bounded and its outcome is kept |
| REJECTED and EXPIRED are added to the course's seven states | a declined or lapsed approval must stay on record, and the course's states have no word for it. PROPOSED |
| The capture of the pre-change state has its own row, and the rollback text is written after it | a rollback that uses a value not yet recorded is not exact; the plan asks for rollback and does not say how to make it exact. PROPOSED |
| Classes A and B (read-only host diagnostics) are served by the Stage 00 record in `docs/curriculum/v2/HOST_READINESS.md` and `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` | both files named in the first column say so (section 4 of the first, section 2.2 of the second); no departure from the plan |
