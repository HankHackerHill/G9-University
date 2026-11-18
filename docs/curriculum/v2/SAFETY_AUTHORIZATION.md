# G9 Cybersecurity Mastery v2.0: safety and authorization (rules of engagement)

- Status: PROPOSED. Nothing in this file is approved. Approval is the owner's act (section 2.3).
- Written: 2026-11-18
- Applies to: every day, lab, assessment and portfolio of the course (60 weekdays, 480 hours)
- Source: `docs/curriculum/v2/MASTER_PLAN_V2.0.md`, sections 0.4, 3.2, 9.4, 10.3, 11.6 and the adversarial table of 14.1; `docs/adr/ADR-0023-a-second-curriculum-shape-the-v2-plan-of-60-weekdays.md`; `docs/curriculum/v2/AUTHORING.md` (rules for every file)
- Labels: PROPOSED, ASSUMED, INFERRED, VERIFIED (only with evidence in the repository), NOT RUN. SEARCH_CHECKED means a search result of 2026-11-18 recorded in a week registry (ADR-0023, decision 10); it does not mean the page was read.

> **Non-execution statement.** Nothing was executed, deployed, scanned, installed, configured or tested in order to write this file. No lab, guest, virtual network, router, SIEM, cloud sandbox or agent harness exists because of it. Every command in this file is proposed text for the owner's review. Writing the file authorizes no command in it, and no sentence in it says that running anything is allowed.

## 1. Purpose and use

This file fixes the rules under which any exercise of the course may be attempted: who can authorize what, where the boundaries of host, guest, network, platform and cloud lie, what is never done, and what to do when a rule is broken. It grants nothing. It defines the conditions under which the owner may grant an approval for one exact action.

| Moment | What this file requires | Section |
|---|---|---|
| Before an exercise | a scope line with a target id and a rules-of-engagement (RoE) record for that exercise | 2.4, 12 |
| Before a command that changes state | an approval record for that exact command | 2.3 |
| During | the stop conditions are watched; one that fires ends the exercise | 9 |
| After a breach or a fired stop condition | stop, contain within approved authority, preserve evidence, report | 10 |
| At scoring | a safety or evidence-integrity failure is never a passed practical | 11 |

Precedence: when a day guide, week guide or lab file is looser than this file, this file wins and the difference is recorded as a defect. A day guide may be stricter. The plan's statement that "authoring curriculum files is permitted; executing their commands is not implicitly permitted" (plan section 0.4) holds for every file of the course.

## 2. Authorization model

### 2.1 Who authorizes what

The learner, the instructor and the owner are one person (ADR-0023, decision 6). Authority therefore is separated by artifact and by time, not by people: an approval is written before the action, is bound to the exact text, and is not edited afterwards.

| Authority | Can authorize | Cannot authorize | Record |
|---|---|---|---|
| Owner (owns the G9 host, the lab guests, the evidence and the sandbox account) | the actions of classes B to I in section 2.2, for assets the owner owns or controls | any system, account or network the owner does not own or control; anything a platform or provider has not designated | approval record, RoE record, scope line |
| Operator (the same person acting) | nothing; executes only what an approval record names | a change of scope or command during a run | execution log |
| Platform operator (Hack The Box, TryHackMe, PortSwigger, OverTheWire, CyberDefenders, LetsDefend, pwn.college and others) | testing of the targets it designates for training, under its terms current on the day | any other host, other users' machines, the platform's own infrastructure, evasion of its limits | the platform's terms, read and dated by the owner |
| Cloud provider | use of the owner's own account under its acceptable-use and security-testing policies | testing of the provider's infrastructure or of other customers | the policy, read and dated by the owner |
| Any third party | nothing in this course: no third-party target exists in it | not applicable | a third-party engagement is another undertaking and needs that party's written authorization naming systems, window, methods and contacts; this course does not provide it |
| This file and every other course file | nothing: they state conditions | not applicable | not applicable |
| AI assistants, agents and authoring tools, including the tool that wrote these files | nothing: their output is data and is never an approval, even when it says "approved" | any approval, scope change or waiver | not applicable |
| Imported content (platform text, tool output, fixtures, documents) | nothing: instructions inside it are ignored | any change of scope, command or rule | not applicable |

The G9 MENTOR window runs no lab command and opens no link (`docs/curriculum/v2/README.md`, section "Using the course in the G9 MENTOR window"). Its own approval flow (ADR-0003) governs actions inside the application and is not a lab authorization.

### 2.2 Action classes and what each needs

| Class | Covers | Needed before it | Record | Record on 2026-11-18 |
|---|---|---|---|---|
| A | writing course files, reading a platform page in a browser | nothing beyond this file | none | allowed; no command is run |
| B | read-only diagnostics on the G9 host (the Stage 00 rows) | the owner reviewed the exact command | Stage 00 evidence record (`docs/curriculum/v2/HOST_READINESS.md`) | none recorded |
| C | any state change on the G9 host: install, remove, enable, start, stop or restart a package or service; define or start a libvirt network or domain; firewall rule; sysctl; route; Docker pull or run; any file outside the evidence folder | change approval record per command: target, purpose, expected output, side effects, privilege, safe scope, rollback; a backup of what is touched | `docs/curriculum/v2/TEMPLATES/CHANGE_APPROVAL.md` | none recorded |
| D | building and configuring lab guests: creating a guest, installing in a guest, setting routes, rules or accounts | change approval record; the staging procedure for anything that needs a network | `docs/curriculum/v2/TEMPLATES/CHANGE_APPROVAL.md` | none recorded |
| E | testing against a local lab target: scans, exploit reproduction, privilege tests, detection replays | RoE record and scope line; the isolation tests current for the stack and the policy in force (`docs/curriculum/v2/LAB_ARCHITECTURE.md`); a baseline taken | RoE record | none recorded |
| F | any browser or VPN session against a hosted platform target | scope line naming provider, exercise and target id; platform terms read and dated; designated target only | scope line and terms note | none recorded |
| G | any resource created in a cloud sandbox | sandbox record, billing limit, written approval, teardown checklist | `docs/curriculum/v2/TEMPLATES/CLOUD_TEARDOWN.md` | no sandbox recorded |
| H | running a toy agent, broker or mock tools | agent RoE (section 7) | RoE record | none recorded |
| I | a session in the malware workspace with a sample | handling record (section 6); samples of classes S0 to S2 only | handling record | none recorded |
| J | everything else: third-party systems, household LAN devices, public hosts, production systems, other users | not available in this course | not applicable | forbidden |

### 2.3 Approval record and its states

An approval record holds, for each command: the exact text; the target id; where it runs (host or guest); purpose; expected output; side effects; privilege; safe scope; rollback; time window; approver; date. The record moves through these states:

| State | Meaning |
|---|---|
| PROPOSED | the text exists (these files); nobody decided |
| REVIEWED | the owner read the exact text and its row; no decision yet |
| APPROVED | the owner wrote the approval, naming the exact text, target and window |
| EXECUTED | run; output stored verbatim with the UTC and local time (America/Chicago) |
| VERIFIED | the expected result was observed and a second observation agrees (plan section 10.4, dual evidence) |
| FAILED or ROLLED_BACK | recorded as such; a failed run is never recorded as skipped |

Rules:
1. An approval covers only the exact text, target and window. A different flag, interface, address or privilege is a new command.
2. An approval ends with its window. No approval is implied by "the plan says so", by a day guide, or by an AI assistant.
3. The record is append-only once the command ran; a correction is a new entry (finalized records are immutable, as in the application).
4. No command is run "to see what happens". Each has an expected output written beforehand, so that a surprise is visible as one.
5. Read-only means the command reads local state and writes nothing outside its own output. A command whose connection can start a service is not read-only (`docs/curriculum/v2/HOST_READINESS.md`, class C).
6. If the wording of a command differs from the approved text by one character, the run is stopped and re-proposed.

### 2.4 The scope line and target identifiers

Plan section 0.4 requires that each offensive lab be authorized "by scope and target ID". Every day guide's line `Lab authorization:` is one scope line, written in the form below. The RoE record (section 12) holds the long form.

```text
SCOPE <scope-id> | TARGET <target-id>[,<target-id>...] | CLASS <A-J> | ADDR <addresses or exercise address> | FROM <source id> | WINDOW <start>/<end> <IANA zone> | ALLOW <technique classes> | DENY <exclusions> | STOP <codes> | ROE <record id> | TERMS <platform, date read, or none>
```

| Field | Content | Rule |
|---|---|---|
| scope-id | the day id and block, for example `D17-C`, or a lab id such as `W04-GUIDED` | unique; one scope line per exercise |
| target-id | see the grammar below | a scope line without a target id is void |
| CLASS | the action class of section 2.2 | one class per line |
| ADDR | the lab addresses (`10.77.10.0/24` RED, `10.77.20.0/24` DMZ, `10.77.30.0/24` CORP, domain `corp.g9.test`) or the exercise address the platform shows | no address outside the plan's topology for class E; for class F only the address the platform assigns to the designated exercise |
| FROM | the source: `G9-ATK-01`, the host browser, a named VPN-guest | traffic from any other source is out of scope |
| WINDOW | start and end with zone | an action outside the window is out of scope |
| ALLOW | named technique classes, for example `TCP connect scan of the listed ports`, `manual SQL injection reproduction` | a class not named is not allowed |
| DENY | exclusions, always including the host, the household LAN, the Internet and every address not listed | the standing exclusions of section 3 apply even if the field is shortened. The only exception is an address that the RoE of an isolation test designates (test T5) |
| STOP | the stop-condition codes of section 9 that apply, always `STOP-01` to `STOP-16` unless a code is inapplicable and says why | not removable |
| ROE | the id of the signed RoE record | unsigned RoE means class E to I are not available |
| TERMS | platform and the date its terms were read (class F); cloud provider policy date (class G) | missing date means not read |

Target identifier grammar (PROPOSED):

| Form | Meaning | Examples (synthetic) | May be an offensive target |
|---|---|---|---|
| `G9-<ROLE>-<nn>` | a guest of the local lab | `G9-RTR-01` router, `G9-ATK-01` attacker, `G9-TGT-01` Ubuntu target, `G9-DC-01` domain controller, `G9-WKS-01` workstation, `G9-SIEM-01` Wazuh, `G9-MAL-01` malware workspace | yes, as the RoE says. `G9-ATK-01` is the source, not a target. `G9-RTR-01` is a policy subject in isolation tests only. `G9-MAL-01` has no network and is never a network target |
| `HOSTED:<provider>:<slug>` | one exercise on a platform | `HOSTED:portswigger:sqli-intro-01` | yes, when the platform designates it |
| `CLOUD:<sandbox-label>:<scenario>` | one scenario in the approved sandbox | `CLOUD:sbx-a:iam-minpriv-01` | yes, in the sandbox only |
| `AGENT:<toy-id>` | the toy assistant, its broker and mock tools | `AGENT:toy-01` | yes |
| `DATA:<fixture-set>` | an offline dataset, no live target | `DATA:w08-case-01` | not applicable: analysis only |
| `HOST:G9` | the trusted G9 host | `HOST:G9` | never; the subject of classes B and C only |

Sample (synthetic): a class E scope line, status PROPOSED, not approved.

```text
SCOPE D08-B | TARGET G9-TGT-01 | CLASS E | ADDR 10.77.20.20 | FROM G9-ATK-01 | WINDOW 2026-11-25T10:45/2026-11-25T12:45 America/Chicago | ALLOW TCP connect scan with service version detection of the ports named in the RoE | DENY host, household LAN, Internet, every address except 10.77.20.20, any scan flag not named in the RoE | STOP STOP-01..STOP-16 | ROE <record id> | TERMS none
```

Sample (synthetic): a class F scope line.

```text
SCOPE D17-B | TARGET HOSTED:portswigger:sqli-intro-01 | CLASS F | ADDR the lab address the platform shows for this exercise | FROM host browser | WINDOW 2026-12-08T10:45/2026-12-08T12:45 America/Chicago | ALLOW manual testing of that one lab | DENY every other address on the platform, automated scanners unless the terms allow them, other users | STOP STOP-01..STOP-16 | ROE <record id> | TERMS PortSwigger <date read>
```

The scope register (the learner's private record, not a course file) lists every scope line with its RoE id, window and state (open, closed, stopped). A scope line that is open after its window ended is a finding of the weekly review.

## 3. Boundaries

### 3.1 Host

| Rule | Why |
|---|---|
| The G9 host is trusted and is never an offensive target (`HOST:G9`). Guests do not initiate anything toward it except the probes of isolation tests T5 and T7 that the RoE designates (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 11) | plan section 3.2 topology |
| The host holds no address inside `10.77.0.0/16`; the lab bridges carry no host address (design, `docs/curriculum/v2/LAB_ARCHITECTURE.md`) | least host exposure; plan section 14.1 "libvirt host reachable from guest" |
| Intentionally vulnerable applications never run on the host's Docker. The host's Docker, its networks (`172.17.0.0/16`, `172.18.0.0/16`, `172.19.0.0/16` at the 2026-11-14 observation) and the Proton or WireGuard routing are preserved: no change to them without a class C record and a before and after comparison | plan section 3.2 Docker rule and section 2.2 "preserve Proton/WireGuard and Docker behavior" |
| The host's Docker socket, SSH agent, secrets and home directory are never mounted into or exposed to a vulnerable container or guest | plan section 3.2 |
| The host runs the hypervisor tooling, the G9 MENTOR window, a browser, and read-only analysis of fixtures. Offensive tooling runs in `G9-ATK-01` against lab targets; a hosted browser-based platform exercise uses the host browser (class F) | keeps offensive traffic inside a boundary that can be tested |

### 3.2 Guest

| Rule | Why |
|---|---|
| Each guest has exactly the network interfaces of the inventory (`docs/curriculum/v2/LAB_ARCHITECTURE.md`): one per guest, three on the router, none on the malware workspace | isolation test T8 |
| No shared clipboard, shared folder, drag and drop, USB redirection, GPU passthrough or guest-agent file channel. Data enters a guest as read-only media. Data leaves a guest as text (typed notes or a console log) treated as untrusted | plan section 3.2 malware rule, extended to all guests by this file (PROPOSED) |
| Consoles are reached only through the host's local graphics or serial console (a local socket or `127.0.0.1`); no guest console listens on a LAN address | no new network path |
| No real credential, token, personal datum or organization datum is in any guest or image | plan section 0.4 |
| A guest is treated as hostile to the host once it runs intentionally vulnerable software. Hypervisor escape is a residual risk that no isolation test removes; QEMU and libvirt are kept updated under the owner's policy and the confinement of each guest process is checked (`docs/curriculum/v2/LAB_ARCHITECTURE.md`) | plan section 14.1 |
| A guest may be reverted or discarded at any time; evidence is copied out first | recovery plan |

### 3.3 Network

Zones, policy and tests are in `docs/curriculum/v2/LAB_ARCHITECTURE.md`. The rules that matter here: no route from any guest to the household LAN (`192.168.1.0/24` at the observation) or to the Internet during active training; no reliance on a VPN kill switch or on a libvirt network definition as proof of containment (plan section 0.4); isolation is a measured property of a stack and a policy version, and any change voids it until the relevant tests are re-run.

### 3.4 Hosted platforms

| Rule | Why |
|---|---|
| Only the exercise the scope line names, only the address the platform assigns to it | designated targets only (plan section 0.4) |
| Nothing on a platform's shared network except the assigned target is probed, enumerated or contacted: no scan of a platform VPN subnet, no contact with other users' machines | platform terms; other users are third parties |
| Browser-based environments (a platform's in-browser attack machine, an academy lab opened in the browser) need no VPN and are preferred | no new network path on the host |
| A platform that needs a VPN profile: the profile is used only inside a disposable guest attached to no lab zone and run while no lab guest runs, or on the host only after the owner has recorded how it interacts with the Proton or WireGuard routes and the Docker bridges. The plan does not decide this; see open decision O-02 | the lab has no Internet route by design, and the host has a working VPN |
| Platform terms are read by the owner on a recorded date before first use (section 8) | terms change; the registry is not the terms |

### 3.5 Cloud

Only an approved sandbox account, only the resources of the named scenario, only inside the window, under the guardrails of section 5. A cloud action outside a sandbox is class J.

### 3.6 Evidence and data

Evidence lives in the learner's private evidence folder, outside lab guests and outside the course source. Course files contain no evidence, no result, and no secret. A result appears in a course file only as `Sample (synthetic):` or after the owner has run it and recorded it with provenance (HOST_READINESS, LAB_ARCHITECTURE and FIXTURES define the formats).

## 4. What is never done

| ID | Never | Basis |
|---|---|---|
| N-01 | run a command that has no approval record, or that differs from the approved text | plan section 0.4 |
| N-02 | connect an intentionally vulnerable guest to a physical LAN, the household network, real business infrastructure or an unapproved public destination | plan section 0.4 |
| N-03 | treat a VPN kill switch, a libvirt network definition or any single control as proof of containment | plan sections 0.4 and 3.2 |
| N-04 | act on a target or address that is not in the scope line, or outside its window or technique classes | plan sections 0.4 and 9.4 |
| N-05 | use a real credential, token, key, personal datum, organization datum, or a third-party credential dump; every credential is synthetic | plan sections 0.4 and 5 (week 5 password exercise) |
| N-06 | scan, enumerate or contact the household LAN, public hosts, a platform's shared network, or other users | plan section 9.4 |
| N-07 | run vulnerable containers on the trusted host, mount the host Docker socket, SSH agent, host secrets or sensitive files into a vulnerable container, or run a vulnerable container privileged without an exercise-specific RoE | plan section 3.2 |
| N-08 | execute an unknown sample, run live malware outside the isolated workspace, or give the workspace a network, shared folder, shared clipboard, or real secret; join it to the domain | plan sections 3.2 and 5 (week 9) |
| N-09 | publish, post or commit an active-machine solution, a flag, or a vendor course text; use a published solution without declaring it | plan sections 0.3, 4 and 9.4 |
| N-10 | invent a command, output, result, finding, detection, benchmark or approval, or present an untested step as tested | plan sections 0.4, 9.4 and 16 |
| N-11 | create a cloud resource outside the approved sandbox, without a billing limit, or leave a scenario undestroyed and unverified | plan sections 3.2 and 9.4 |
| N-12 | give an agent a shell, a network connection, a real file path or a real credential; let a model's output decide an authorization | plan sections 5 (weeks 11 and 12) and 14.1 |
| N-13 | disable, "temporarily" alter, or route around the host's VPN, firewall or Docker networking to make a lab work, outside a class C record | plan sections 2.1 and 3.2 |
| N-14 | leave a temporary exception, route, rule, listener or exposed service in place after its exercise; every exception has a removal and a retest | plan section 5 (week 6 day 29) |
| N-15 | use AI assistance during a benchmark or scored attempt (midpoint, final, Friday scored practicals) or let it produce evidence; paste untrusted lab content into an assistant that has tools on the host | plan sections 9.1 and 10.4 |
| N-16 | assess wireless, mobile applications, OT or ICS systems, or any physical or social target; these tracks are deferred (plan section 12) | plan section 12 |
| N-17 | run stress or denial-of-service tests; they are not in the syllabus. If one is proposed it needs its own RoE with resource caps for host and guests (PROPOSED) | plan sections 5 and 9.4 (host and others' data) |
| N-18 | claim a lab was tested, a learner passed, or a certification is within reach on the strength of the documents | plan sections 0.4 and 13; `docs/curriculum/v2/README.md` status line |

## 5. Cloud guardrails

Plan sources: section 3.2 (cloud), 5 (week 11), 11.6 (teardown record), 14.1. Facts about a provider's products and policies are ASSUMED and unchecked here (registry entries for week 11 are `ASSUMED`, `docs/curriculum/v2/WEEKS/W11/RESOURCES.json`); pointer `docs/curriculum/v2/RESOURCE_REGISTRY.md`.

### 5.1 Sandbox record (before any resource exists)

| Item | Requirement |
|---|---|
| Account | a dedicated account used for nothing else; no production workload, no employer or client data, no link to other accounts' data. Its id is redacted before any publication |
| Authorization | written by the owner: account label, region or regions, scenario, window, billing limit, teardown executor and date |
| Principals | the root identity is not used for the exercise and has multi-factor authentication; no long-lived root keys exist. The deployer identity is separate from the exercise identities |
| Credentials | minimal and short-lived where the provider supports it; permissions bounded to the scenario's actions, resources and region (a permissions boundary or an equivalent condition). No credential in the repository, the course files, a guest image or an AI prompt. The deployer's permissions are the largest in the exercise and are reviewed as such |
| Tags | every resource carries `g9-lab=<lab id>` (or the provider's equivalent) so that a tag-based inventory finds all of them |
| Audit | the provider's activity log is on before the exercise so that week 11 day 51 can review audit events |
| Exposure | no scenario makes a resource reachable from the public Internet beyond what the scenario needs; where a framework restricts access to the owner's public address, the narrowest range is used and the address is redacted in every record (ASSUMED behavior of the framework; confirm in its README at the time) |
| Data | none real; every object, record and secret in the scenario is synthetic |

### 5.2 Billing limit

A billing limit is not one control. Budget alerts notify and may lag actual spending, so the limit is layered (ASSUMED; confirm against the provider's current billing documentation before relying on any layer):

| Layer | Control | Evidence |
|---|---|---|
| 1 | the owner sets the limit as an amount the owner accepts to lose in full: `<owner-set-limit>`; this file proposes no figure | sandbox record |
| 2 | the cheapest scenario that teaches the objective; time-boxed to one window | scenario choice in the RoE |
| 3 | budget alerts at stated fractions of the limit, and a budget action where the provider offers one | a screenshot-free export of the budget definition, redacted |
| 4 | manual check of spend at start, at the end of the window, and the next day; delayed charges are possible | the teardown record's "actual spending" line, filled only from an inspected figure |
| 5 | destruction the same day, so that nothing accrues overnight | destroy-and-verify checklist |

### 5.3 Destroy and verify

The record is `docs/curriculum/v2/TEMPLATES/CLOUD_TEARDOWN.md`. The checklist below is the minimum; a step without evidence is not done.

| Step | Action (proposed) | Evidence | Pass criterion |
|---|---|---|---|
| 1 | before creating anything, list the resources of the account and region in the scope of the scenario | inventory before | empty or known |
| 2 | create with the scenario's own tooling, from the sandbox identity only | creation log | resources carry the lab tag |
| 3 | record the created inventory | inventory during | matches the scenario's expectation |
| 4 | after the exercise, destroy with the same tooling that created | destroy log | tool reports completion |
| 5 | query by tag, then by service, in every region used and in the global services (identity, storage, DNS) | post-destroy queries | no resource carries the tag; no resource of the scenario's services remains |
| 6 | check for resources that outlive a stack: volumes, snapshots, addresses, gateways, log groups, keys, roles, policies, users, access keys, buckets | per-type queries | none remain |
| 7 | revoke or delete any credential created for the exercise | identity listing | none remain |
| 8 | review the activity log for calls outside the window or from unexpected principals | log extract | none; otherwise stop condition STOP-12 |
| 9 | check spend and budget status | inspected figures | at or below the limit; otherwise STOP-12 |
| 10 | repeat steps 5 and 9 the next day | second queries | still clean; delayed charges noted |
| 11 | set the final status | record | VERIFIED only if steps 5, 6, 7, 10 have evidence; PARTIAL or UNVERIFIED otherwise |

### 5.4 Substitution

If no sandbox is approved by week 11, the exercise is replaced by the hosted or no-cloud lab the week guide names, the substitution is recorded, and the cloud outcomes of the gate are marked NOT DEMONSTRATED (plan section 5, week 11). No cloud exercise is run in an account that fails section 5.1.

## 6. Malware handling

Plan sources: sections 3.2, 5 (week 9), 14.1. Default: static educational binaries and recorded evidence; no live malware in a generic VM; the workspace is a separate, disposable, network-less guest (`G9-MAL-01`, `docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 8).

| Sample class | Content | Allowed |
|---|---|---|
| S0 | course-authored toy programs and synthetic marker files (a parser with a seeded defect, a harmless process or file simulation) | built and run in `G9-MAL-01` |
| S1 | benign educational binaries with a licence that permits the use | static inspection in `G9-MAL-01`; execution only if the exercise says so |
| S2 | recorded traces: logs, packet captures, process listings, memory-forensic exercise images supplied by a platform | analysis only, no execution; platform terms apply |
| S3 | real malware | not used in this course. A decision to use any needs its own RoE, a non-executing handling plan, and the owner's written approval; absent that, S3 is class J |

Handling rules:
1. Samples enter the workspace as read-only media created on the host; the host never executes, opens in an application, or mounts a sample from the workspace. Hashes are recorded before transfer.
2. Sample files carry a neutral extension and no execute permission until the exercise requires it.
3. The workspace has no network interface, no shared folder, no shared clipboard or file transfer, no USB or guest-agent channel, and holds no real secret. It is never joined to `corp.g9.test`.
4. Each session starts from a clean baseline and ends by discarding the session's changes; a contaminated workspace is discarded, not cleaned.
5. Data leaves the workspace as text only, treated as untrusted (control characters stripped before viewing in a terminal). Indicators are recorded as hashes and strings, never as clickable addresses.
6. A crash, a hang, or an unexpected device in the workspace is a stop condition (STOP-13). A claim that a binary "is malware" or belongs to a family needs evidence beyond a static property (plan section 5, week 9 boundary).
7. Nothing from a sample is submitted to an external service without a recorded decision that the hash or the file may leave the machine.

## 7. AI-agent lab limits

Plan sources: sections 5 (weeks 11 and 12), 6, 9.3, 11.5, 14.1. Every limit is enforced in code or in the environment, not in a prompt (the application's own rule: model output is data, authority is decided outside the model).

| ID | Limit | Check |
|---|---|---|
| AI-01 | The agent runtime has no production privileges: an unprivileged identity, no credential, no host path, no network | process inventory; the RoE lists the identity |
| AI-02 | Tools are mock functions over synthetic data: a note reader, a record lookup, a message outbox that writes to a scratch store. No shell, file-system, network or database tool | tool inventory in the evaluation record |
| AI-03 | A deterministic broker, independent of the model, decides each proposed call: default deny, allowlist per role, argument validation, size and character limits | broker policy file and its tests |
| AI-04 | The model's statement of permission, refusal or compliance is never an input to the broker | test case where the model asserts permission and the broker denies |
| AI-05 | Four things are logged separately: the model's proposal, the broker's decision, the downstream effect, and the state before and after (a digest of the scratch store) | `docs/curriculum/v2/TEMPLATES/AI_AGENT_EVALUATION.md` fields |
| AI-06 | Untrusted content (retrieved documents, tool results) is passed as data in a delimited field; it never extends the instruction or the permission policy | architecture note and a test with an instruction-like document |
| AI-07 | Synthetic content only. No real secret, personal datum or organization datum in any prompt, document or log | fixture manifest (`docs/curriculum/v2/FIXTURES/README.md`) |
| AI-08 | The model comes from outside the lab or is a recorded transcript: lab guests have no network. Only synthetic text crosses, in either direction, by the transfer methods of section 3.2. The path is chosen by the owner (open decision O-05) | RoE names the path |
| AI-09 | A hosted model endpoint, if used, has a spend cap, a key kept outside the repository and the guest, and receives no content that is not synthetic | sandbox-style record |
| AI-10 | An evaluation harness is used only after a manual baseline exists and only through the same path and broker | evaluation record |
| AI-11 | At least two tool-boundary tests are normal-request positives and at least two are unauthorized proposals per exercise; plan minimum across weeks 11 and 12 is three agent boundary test cases | evaluation record |
| AI-12 | A side effect outside the scratch store, a call that reached something real, or a broker bypass is a stop condition (STOP-14) | state digest and broker log |
| AI-13 | The learner's own use of an AI assistant is a critic after an unassisted attempt, outside exam conditions. It does not replace discovery, does not generate evidence, and is not used in the midpoint, the final, or any Friday scored attempt | lab record states use or non-use |
| AI-14 | Untrusted lab content (platform text, fixture strings, tool output) is not pasted into an assistant that holds tools on the host, because that content can carry instructions | N-15 |

## 8. Third-party platform terms

### 8.1 Rules that hold on every platform

| Rule | Detail |
|---|---|
| Designated targets only | the platform's own training machines, rooms, labs or challenges for the exercise named in the scope line (plan sections 0.4, 4) |
| No active-content spoilers | no flag, credential found, step order, payload sequence, answer, or screenshot of an active machine, challenge, Sherlock, room or lab is published, posted, committed to a shared place, or placed in a portfolio. Retired content may be written about only where the platform's current guidelines allow it and the retired state was confirmed on the day (plan section 4; registry `w03-htb-writeup-guidelines`) |
| Course files carry no solution | the course names an exercise, says what to practise and what evidence to keep, never the solution, even for retired content (`docs/curriculum/v2/AUTHORING.md`) |
| No vendor text | no verbatim vendor course text, module text or room text in course files, notes meant for sharing, or the portfolio; own words and a citation by name (plan section 0.3) |
| Solution exposure is declared | reading a walkthrough, a hint beyond the hint ladder, or an AI-generated solution to an exercise is recorded in the lab record; that exercise no longer counts as independent or unseen evidence. A published solution used without acknowledgment is an evidence-integrity failure (plan section 9.4). Do not select for week 12 a challenge whose complete solution was read (plan section 5) |
| Shared infrastructure | no scan or probe of anything but the assigned target; no persistence left on a shared or resettable target; reset or terminate the instance when done; no evasion of rate limits or anti-abuse measures |
| Platform notices | a warning, restriction or contact from a platform is a stop condition (STOP-09) |
| Exam rules | a certification body's exam rules govern its exam only. This course's allowance of an AI critic does not extend to exam conditions; the plan states that OSCP+ restricts AI chatbot use and automation (plan sections 13 and 16); the week 6 and week 12 registries record that search did not confirm this (NOT_CONFIRMED), so it stays ASSUMED and the official guide is reread before any exam (`docs/curriculum/v2/WEEKS/W06/RESOURCES.json`, `docs/curriculum/v2/WEEKS/W12/RESOURCES.json`) |

### 8.2 Register of platforms (the owner fills the date; this file has read none of them)

| Platform | Designated target means | Terms to read before first use | Registry evidence | State |
|---|---|---|---|---|
| Hack The Box (Academy, labs, Starting Point, Sherlocks) | the module lab, machine, challenge or Sherlock assigned in the exercise | platform rules, Acceptable Use Policy, streaming and write-up guidelines | `w01-htb-platform-rules`, `w03-htb-writeup-guidelines` in `docs/curriculum/v2/WEEKS/W01/RESOURCES.json` and `docs/curriculum/v2/WEEKS/W03/RESOURCES.json`: SEARCH_CHECKED 2026-11-18 that sharing how an active machine, challenge or Sherlock was solved is forbidden, that write-ups are allowed only for retired content and Starting Point and Tier 0 modules, and that an Acceptable Use Policy took effect 2026-04-01. The pages were not opened | terms NOT READ by the owner |
| TryHackMe | the room's own machines | terms of use, room and write-up rules | ASSUMED; room availability and tier are checked per room in the registry | NOT READ |
| PortSwigger Web Security Academy | the lab instance the Academy provides | terms of the Academy, rules on scanners and automation | ASSUMED | NOT READ |
| OverTheWire | the wargame server for the named game | the game's rules page | ASSUMED | NOT READ |
| pwn.college | the course's challenge environment | the platform's rules on sharing solutions | ASSUMED | NOT READ |
| CyberDefenders, LetsDefend | the downloadable or hosted case | terms and write-up rules; evidence files are not redistributed | ASSUMED | NOT READ |
| OWASP Juice Shop, OWASP crAPI (self-hosted) | the instance run inside `G9-TGT-01` | the project licences | ASSUMED | NOT READ |
| CloudGoat, Kubernetes Goat (self-deployed) | resources in the approved sandbox or a lab cluster | project README and licence; provider policy | ASSUMED | NOT READ |

The pointer for every registry fact in this file is `docs/curriculum/v2/RESOURCE_REGISTRY.md` (generated later from the week registries). Until the owner records the date a platform's terms were read, class F for that platform is unavailable.

## 9. Stop conditions

A stop condition ends the exercise at once. "Stop" means: cease issuing commands, interrupt any foreground tool, do not "fix forward", do not clean up before evidence is preserved, and open an incident record (section 10). Resuming needs a fresh approval.

| Code | Trigger | First action |
|---|---|---|
| STOP-01 | uncertainty whether an address, account, URL or action is in scope | stop; re-read the scope line; if still uncertain it is out of scope |
| STOP-02 | any traffic, response or tool output involving an address not in the scope line | stop; record the address and time; section 10, class B1 |
| STOP-03 | a lab guest reaches, or appears able to reach, the household LAN, the host's other addresses, or the Internet; an unexpected network interface, bridge or route appears | stop; contain at the guest (section 10.3); class B2 |
| STOP-04 | an isolation test that applies to the stack fails or is inconclusive, or its evidence is stale after a change | do not start or continue intrusive work; section 10, class B2 |
| STOP-05 | real data appears: a credential, token, personal datum or organization datum in a guest, fixture, log, prompt or repository | stop; do not copy it further; class B3 |
| STOP-06 | an unexplained change of the host's routes, firewall, VPN state, Docker networks, or sustained host CPU, memory, disk or temperature pressure | stop; capture state before any fix; class B2 |
| STOP-07 | the time window ended or the planned time of the block is exceeded | stop; document the blocker; switch to the approved alternative without claiming the original passed (plan section 10.3, item 7) |
| STOP-08 | doubt about the provenance or integrity of evidence: a result cannot be reproduced, an output was not saved, a hash differs | stop scoring that evidence; class B5 |
| STOP-09 | a platform warning, block, ban or contact; a terms change that bears on the exercise | stop; read the notice; do not work around it |
| STOP-10 | a real vulnerability appears in a platform's own infrastructure or in third-party software outside the designated target | stop exploiting; record minimal evidence; report privately through the platform's or vendor's disclosure channel; no public disclosure (class B4) |
| STOP-11 | a cleanup step fails: a temporary route, rule, listener or exposed service cannot be shown removed | keep the exercise stopped until removal is proven by the retest of the relevant isolation test |
| STOP-12 | a cloud action outside the window, an unexpected resource, a principal outside the scenario, or spend above the limit | stop; revoke the exercise credentials; class B6 |
| STOP-13 | in the malware workspace: a network interface or device appears, a sample runs that was not meant to, the clipboard or a shared channel works, a crash or hang of the workspace | stop; do not reuse the workspace; class B7 |
| STOP-14 | in the agent lab: a tool call with a side effect outside the scratch store, a broker bypass, an action on a real resource | stop; class B8 |
| STOP-15 | fatigue or error pattern: three consecutive commands that did not match their expected output and are not understood | stop; write the hypothesis; resume next block |
| STOP-16 | the owner revokes the approval | stop; no argument, no wrap-up beyond containment |

## 10. Incident procedure for a safety breach

Plan section 10.3 item 5: stop testing, contain within approved authority, report, preserve evidence. The order here puts preservation before reversal, because evidence is lost by a revert or a power-off (RFC 3227 order of volatility, registry `w08-rfc3227`, SEARCH_CHECKED).

### 10.1 Breach classes

| Class | Meaning | Containment (within approved authority) | Notify | Score effect (section 11) |
|---|---|---|---|---|
| B1 | scope violation: traffic or action toward an address or account not in scope | stop the source tool; ladder C1 to C3 on the source guest | the owner of the affected address, if it is a third party; the platform for a hosted exercise | safety failure |
| B2 | containment failure: a vulnerable or test guest is reachable from, or reached, the LAN, host or Internet; an unintended route, NIC or rule | ladder C1 to C4 on the guests involved; the router is shut down (C4) if inter-zone leakage is the issue | people responsible for the LAN; the owner's own household if devices may have been contacted | safety failure |
| B3 | secret or real-data exposure: in a file, commit, prompt, log or image | stop; remove from view; do not delete the only copy until preserved; rotate or revoke the secret | the issuer of the credential; any person whose data it is | safety failure if real production credentials were used; otherwise evidence-integrity review |
| B4 | third-party exposure or a vulnerability found outside the designated target | stop exploitation; minimal evidence | the platform or vendor privately | safety failure if out-of-scope activity caused it |
| B5 | evidence-integrity failure: a fabricated or unobserved result, an undeclared solution, a missing record of a destructive step | quarantine the affected evidence; mark it | none external | evidence-integrity failure |
| B6 | cloud: spend above the limit, an unexpected resource, public exposure, a principal outside the scenario | revoke exercise credentials; destroy the scenario if the owner approves; section 5.3 | the provider if abuse or compromise is suspected | safety failure if unauthorized provisioning |
| B7 | malware workspace: a boundary property failed or a sample ran unintended | ladder C2 to C4 on `G9-MAL-01`; discard its disk | none external | safety failure |
| B8 | agent lab: an effect outside the scratch store or a broker bypass | stop the agent; preserve the broker log and state digest | none external | safety failure |

### 10.2 Phases

| Phase | Action | Time target (PROPOSED) |
|---|---|---|
| 1. Stop | cease commands; interrupt foreground tools; note the time (UTC and local) and what was running | at once |
| 2. Contain | the least destructive step of the ladder in section 10.3 that stops the effect; nothing beyond the approved authority; no change to household devices, the VPN or the host firewall to contain a lab guest | within minutes |
| 3. Preserve | capture state in the order of volatility (section 10.4) before any revert, shutdown beyond what containment needed, or cleanup; hash the files; write who captured what and when | before anything is reversed |
| 4. Report | open the incident record (below); notify third parties as the class table says | within 24 hours of detection; third-party notices as soon as the owner can |

### 10.3 Containment ladder (the owner's pre-approval is in the RoE; none is effective until the owner signs it)

Command rows follow the project format: target, purpose and expected output, side effects, privilege, safe scope, rollback.

| Step | Command (proposed text) | Target | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| C1 | stop the foreground tool (interrupt) | the running tool in the guest | the tool stops; no output expected | in-flight probes are cut; partial results | guest user | the tool you started | restart only with a new approval |
| C2 | `virsh -c qemu:///system domif-setlink <domain> <interface-or-mac> down` (URI is ASSUMED until Stage 00) | one interface of one lab guest | cuts the virtual link, keeps memory; prints a line saying the device was updated | guest sees link down; connections break | libvirt system connection (root or the libvirt group, ASSUMED) | the domain named in the incident | same command with `up` |
| C3 | `virsh -c qemu:///system suspend <domain>` | one lab guest | freezes vCPUs, keeps memory; prints that the domain was suspended | guest clock stops; peers time out | as C2 | the domain named in the incident | `virsh -c qemu:///system resume <domain>` |
| C4 | `virsh -c qemu:///system shutdown <domain>` | one lab guest, or `G9-RTR-01` to cut all inter-zone paths | asks the guest to power off; may be ignored by a compromised or hung guest | volatile state is lost after shutdown | as C2 | the domain named in the incident | `virsh -c qemu:///system start <domain>`, then restore a verified baseline if state is in doubt |
| C5 | `virsh -c qemu:///system destroy <domain>` | one lab guest | immediate power-off, like pulling the plug | volatile state lost; disk may be inconsistent | as C2 | only if C2 to C4 cannot stop an out-of-scope effect that continues, and after the time is recorded | `virsh -c qemu:///system start <domain>`, then restore a baseline |

Not part of containment: deleting or undefining a guest, deleting images, editing the router rules or host firewall, touching the VPN, Docker or household devices. Those wait until evidence is preserved and the owner has approved them as class C.

### 10.4 Evidence capture (order of volatility, all proposed, all read-only unless stated)

| Step | Command (proposed text) | Target | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| E1 | `date -u +%Y-%m-%dT%H:%M:%SZ` and `date +%Y-%m-%dT%H:%M:%S%z` | the host | the capture time in UTC and local with offset | none | user | the host | none needed (read-only) |
| E2 | `virsh -c qemu:///system dumpxml <domain>` | one guest, live configuration | the guest's devices and networks as they are now; save the output to the evidence folder | writes the file you redirect to | libvirt connection | the incident's guests | delete the copy only after the incident closes |
| E3 | `virsh -c qemu:///system domiflist <domain>` | one guest | its NICs, networks and MACs | none | libvirt connection | the incident's guests | none needed (read-only) |
| E4 | `ip -br addr`, `ip route show table all`, `ip -6 route show table all`, `ip rule show` | the host | interfaces, all route tables, policy rules | none | user | the host | none needed (read-only) |
| E5 | `sudo nft list ruleset` | the host firewall | the rules in force; may reveal VPN endpoint addresses: redact before sharing | none | root (sudo) | the host | none needed (read-only) |
| E6 | `sudo ss -tupan` | the host | listening sockets and connections with owning processes | none | root (sudo) | the host | none needed (read-only) |
| E7 | router, service and guest logs, exported as text through the guest's own console procedure | the guests | the guests' view of the event | the guest writes an export | guest admin | the incident's guests | none needed |
| E8 | `sha256sum <files> > <evidence folder>/hashes.txt` | the evidence files | a hash list of what was captured | writes the hash file | user | the evidence folder | delete the hash file; recompute |

### 10.5 Incident record

Fields: incident id; class; detection time (UTC and local with zone) and how detected; stop condition fired; scope line and RoE ids; what was observed (OBSERVED) separately from what is inferred (INFERRED) and unknown; systems and data affected; containment steps with exact commands and times; evidence list with hashes and who captured it; third-party notices and their time; preliminary root cause; corrective actions; the isolation tests re-run and their results; resumption approval; closure date. The record is append-only.

### 10.6 Resuming

Work resumes only when the cause is stated, the corrective action is applied under a class C or D record, the relevant isolation tests (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 11) are re-run on the corrected stack with results recorded, and the owner signs a new RoE or an amendment. A breach of class B3 also requires that the exposed secret is rotated or revoked and its issuer informed.

## 11. Safety and integrity in scoring

From plan section 9.4, restated.

| Gate | Failure examples | Effect |
|---|---|---|
| Safety gate (automatic failure) | out-of-scope activity; use of real production credentials; an exposed vulnerable service that is not contained or remediated; unauthorized public cloud provisioning; destruction of others' data; knowingly falsified evidence | the practical is not passed; learning credit may stand in theory |
| Evidence integrity | missing target or scope context; fabricated commands or outputs; a test claimed passed with no observed result; a destructive tool run without a record; an unacknowledged published solution; a false positive claimed as a verified finding | the practical is not passed |
| Acceptable partial performance | a blocked lab setup with documented diagnostics; correctly identified false positives; an unfinished challenge with accurate enumeration and hypotheses; partial detections with missing telemetry stated | scored on what was demonstrated; remediation prescribed; no invented success |

The weekly and final scores are in `docs/curriculum/v2/INSTRUCTOR_GUIDE.md` (the plan's 100-point split and its gates). This file adds only that the safety gate and the evidence-integrity gate are prerequisites to any numeric pass.

## 12. Rules-of-engagement template for one lab

The learner copies the template into a private record, fills it, and signs it. An unsigned or incomplete RoE means classes E to I are unavailable for that exercise. NIST SP 800-115 provides a rules-of-engagement structure (registry `w06-nist-sp-800-115`, SEARCH_CHECKED); this template is the course's own and does not reproduce it.

```text
RULES OF ENGAGEMENT <roe-id>        version <n>        status PROPOSED / APPROVED / CLOSED
Exercise: <scope-id>  <title>       Day id: <Dnn>     Week: <Wnn>

1. AUTHORIZATION
   Owner statement (verbatim): "I authorize the activities in section 3, from the sources in
   section 2, against the targets in section 2, for the window in this section, and nothing else."
   Window: <start>/<end> <IANA zone>, UTC equivalent <start>/<end>
   Platform or provider terms read: <platform, document, date read, or none>
   Isolation tests current for this stack and policy: <test ids, run ids, date, baseline id>

2. TARGETS AND SOURCES
   | target id | class | address or URL | basis of authority (owned / intentionally vulnerable / platform-designated / sandbox) | state and version |
   Source id(s): <G9-ATK-01 / host browser / named guest>
   Expected inventory of the stack: <guest ids, baseline ids, MAC inventory reference>

3. PERMITTED ACTIVITY
   Technique classes allowed: <named classes>
   Tools allowed (name, version, where run): <list>
   Accounts: <synthetic accounts and roles only; no real credentials>
   Rate and volume limits: <limits>
   Data: <synthetic only; where evidence is stored>

4. EXCLUDED
   Always: the G9 host, the household LAN (192.168.1.0/24 at the observation), the Internet,
   every address not in section 2, other users and other platform machines, any production
   system, any real credential or data. The only exception is the single address that the
   RoE of isolation test T5 names, for the probes that test lists.
   Lab specific: <list>

5. ENVIRONMENT PRECONDITIONS
   Baseline id taken before start: <id>      Resource fit checked: <memory, disk, vCPU>
   Temporary exceptions requested (route, rule, listener, port): <none / list with removal step>
   Retest after removal: <isolation test ids>

6. STOP CONDITIONS
   STOP-01 to STOP-16 apply. Lab-specific additions: <list>

7. CONTAINMENT PRE-APPROVAL (effective only when signed)
   Steps C1 to C4 of section 10.3 of SAFETY_AUTHORIZATION: <approved / not approved>
   Step C5: <approved only if C2 to C4 cannot stop an out-of-scope effect / not approved>

8. COMMAND REGISTER
   | id | exact command | target id | where run | privilege | expected output | side effects | rollback | approval state |

9. EVIDENCE PLAN
   Artifacts to keep, their names, who captures them, hash method, UTC and local time format.

10. ROLLBACK AND CLEANUP
   Steps, evidence that each was done, who checks.

11. COMMUNICATION
   Who is told, when, how, in each breach class of section 10.1.

12. SIGNATURE
   Owner name, date, time, zone.  Operator name (the same person), date, time, zone.

13. CLOSURE (appended after the window)
   Actual start and end (UTC and local); deviations; stop condition fired (code or none);
   cleanup verified (evidence); isolation retest results; status CLOSED / STOPPED / BREACH <class>.
```

## 13. Adversarial failure modes of plan 14.1, with the control here

| Failure mode (plan 14.1) | Control in this file | Where it is verified |
|---|---|---|
| Vulnerable guest bridged to the LAN | N-02, 3.2 NIC rule, STOP-03, class B2 | isolation tests T5, T6, T8 in `docs/curriculum/v2/LAB_ARCHITECTURE.md` |
| Libvirt host reachable from a guest | 3.1 (no host address on lab bridges), residual-risk statement in 3.2 | tests T5 and T7; the host-exposure design (section 7 of the same file) |
| VPN fallback default route bypass | N-03, N-13 | tests T5 to T7 with host-side captures |
| Docker image or `docker.sock` compromises the host | N-07, 3.1 | container inspection in the target guest (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 9) |
| Live malware outside containment | section 6, N-08, STOP-13 | NIC and device inventory of `G9-MAL-01` |
| Unintended cloud spending or exposure | section 5, N-11, STOP-12 | section 5.3 steps 5 to 10 |
| AD snapshot restore inconsistency | the AD recovery rules (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 10) | test T9, AD variant |
| No event because the source was disabled | not a safety control; handled in weeks 3 and 7 | source and SIEM verification in the week guides |
| LLM tool call crosses authority | section 7, N-12, STOP-14 | broker trace and state digest |
| Scanner output treated as fact | not a safety control; handled by the evidence-integrity gate (section 11) | manual reproduction in each week |

## 14. Departures from the plan

| What | Why |
|---|---|
| Target identifiers use the grammar of section 2.4 | the plan requires a target id and gives no format; the guests need stable names that the scope lines, tests and keys share |
| Scope lines are one line, so that the day guide's `Lab authorization:` line can hold one | `docs/curriculum/v2/AUTHORING.md` fixes that line as a single line |
| "Contain within approved authority" is defined as a ladder with a pre-approval inside the RoE | the plan does not say what authority exists at the moment of a breach; a pre-approved ladder is the least destructive answer (PROPOSED) |
| The malware workspace has no interface at all, not "disconnected adapters" | a disconnected adapter is still a device; no adapter is stricter and loses nothing the plan teaches (PROPOSED reading of plan section 3.2) |
| No-clipboard, no-shared-folder and no-guest-channel rules apply to every guest, not only the malware workspace | a compromised target guest is hostile to the host as much as a sample (PROPOSED) |
| Hosted-platform VPN access is placed outside the lab zones | the plan says "no route to the Internet during active training" for guests and says nothing on how a platform VPN is reached; the lab cannot both have no Internet route and run a platform VPN (open decision O-02) |
| Billing limit is layered, with no figure | a budget alert is a notification, not a hard stop; the amount is the owner's (ASSUMED provider behavior, unchecked) |
| Stress and denial-of-service tests are excluded | not in the plan's syllabus; they threaten the host (PROPOSED) |

## 15. Open decisions for the owner

| ID | Decision | Default until decided |
|---|---|---|
| O-01 | Sign the standing rules of section 2 and the containment pre-approval of section 10.3 | none signed; no class B to I action available |
| O-02 | How a hosted platform's VPN profile is reached (disposable guest outside the lab zones, or the host with recorded routing interaction), or browser-based environments only | browser-based environments only |
| O-03 | Whether a cloud sandbox is approved for week 11, with account, region, limit and teardown executor | no sandbox; substitution of section 5.4 |
| O-04 | Whether any S3 sample is ever used | no; S3 is class J |
| O-05 | The path by which a model's text reaches the agent broker (recorded transcript, or a model outside the lab with synthetic text only) | recorded transcript |
| O-06 | The date on which each platform's terms were read (section 8.2) | not read |
