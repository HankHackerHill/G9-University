# Week 1 design notes: hand-over to the authors of DAY_01 to DAY_05

- Status: PROPOSED. Learner-safe: this file holds no answer, no fixture map and no fault of any item. Where a day needs one, it points to the learner files and says "the key holds it".
- Reader: the two authors who write `DAY_01.md` to `DAY_05.md`, `ASSESSMENT.md` and the day keys. The contract is `docs/curriculum/v2/AUTHORING.md`; the controlling plan text is section 5 week 1, section 6 row 1, section 7.2 seeds W01-Q01 to W01-Q08 and section 9.3 row 5.
- The week's design files are `WEEK_GUIDE.md`, `LABS/GUIDED_LAB.md`, `LABS/INDEPENDENT_LAB.md`, `LABS/FRIDAY_PRACTICAL.md`, `LABS/fixtures/` (manifest and 61 synthetic files), `EVIDENCE_CHECKLIST.md`, `ANSWER_KEYS/LABS_KEY.md`. Read the first four before writing a block. Do not open `ANSWER_KEYS/LABS_KEY.md` until your own day keys are written, and then only to align, never to copy.
- Housekeeping: the builder's check of a week lists a file under `LABS/` that is not named in AUTHORING.md as an unexpected file. This file is requested by the orchestrator at this path; if the message appears, the owner decides whether to move or delete it once the days are written.
- Nothing was run to write the week. Every command is proposed text; no output may be presented as observed unless it is marked `Sample (synthetic):`.

## 1. Rules that bite

- Key text is forbidden in learner files. The builder compares eight-word runs. Your day keys, the exam key and `LABS_KEY.md` must not repeat eight consecutive words of any learner file of any week, including the lab files and fixtures, and no learner file may hold a line that begins with an answer label. Reword.
- The quiz of a day is written from the mechanism, with data in the question for the applied items. Do not turn the drills of `LABS/fixtures/d02/` into questions: write new scenarios with new numbers. The same holds for the host samples of `d01/`.
- Angle-bracket placeholders go in backticks. No Markdown links; write an address as inline code and list it in `## Resources`.
- A day cites registry ids only. The registry now holds 46 entries; the eleven added on 2026-11-18 are the man-page, systemd, sudoers, Ubuntu SSH socket and two further pwn.college entries. Do not name a vendor's section titles you have not seen: the registry shows only that the HTB Linux Fundamentals module has 30 sections.
- Offense is taught only on owned, intentionally vulnerable, platform-designated or explicitly authorized targets, and each such exercise names its scope. Bandit passwords, flags and solution steps never appear.
- `Lab authorization:` is one line. Use the scope-line fields of `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` section 2.4; suggested ids per day are in section 3.

## 2. Names that must stay identical across files

| Kind | Names |
|---|---|
| Guests | `G9-ATK-01` (RED `10.77.10.10/24`, `52:54:00:77:10:0a`, network `g9-lab-red`, bridge `virbr91`), `G9-TGT-01` (DMZ `10.77.20.20/24`, `52:54:00:77:20:14`, `g9-lab-dmz`, `virbr92`), `G9-RTR-01` (ports `.1` of each zone; stack B and test T1 only). Domain names are the lower-case ids. The guest marker file is `/etc/g9-lab-guest` holding the guest id; the state marker is `/etc/g9-lab-state` |
| Stacks | stack A: ATK and TGT, no router, 6 vCPU, 9 GiB, 120 GiB; stack B: plus the router, 8 vCPU, 11 GiB, 140 GiB (35 percent of the budget). Memory rule at session start: available at least stack RAM plus 8 GiB |
| Guided accounts and groups | `labadmin` (administrator), `svclab` (service), `labuser1` (authorized, in `labusers` and `labreaders`), `labuser2` (unauthorized, in `labusers`); groups `labusers`, `labreaders` |
| Guided paths | `/opt/g9toy/g9toy_reportd.py`, `/etc/g9toy/g9toy.conf`, `/etc/systemd/system/g9toy.service`, `/srv/g9toy/reports/` (`q3_summary.txt`, `q3_detail.txt`, `q3_canary.txt`), `/srv/g9toy/spool/`, `/srv/g9toy/private/ledger.txt`, `/var/lib/g9toy/`; port 8080 on `127.0.0.1` |
| Independent accounts and paths | `svcaudit`, `labuser3` (group `auditors`), `labuser4` (group `ops`); `/srv/g9toy-audit/` (spool, scratch, restricted), `/etc/g9toy-audit/g9toy.conf`, `g9toy-audit.service`, `/etc/sudoers.d/40-ops`, drop-ins `10-image.conf` and `60-hardening.conf`; port 8090 |
| Friday accounts and paths | `labanalyst` (no sudo), `labuser6` (group `shippers`), `labuser7` (group `loaders`), `svcspool`; `/srv/g9toy-spool/` (outbox, inbox, scratch), `g9toy-spool.service`, drop-in `90-sandbox.conf`, `10-g9-lab.conf`; port 8100 |
| Day-2 drills | `d02/perm_drill_01.txt` (accounts `labuser1` to `labuser3`, tree `/srv/drill`), `d02/perm_drill_02.txt` (accounts `labuser4` to `labuser7`, tree `/srv/drill2`) |
| Host-side paths | `<evidence-folder>/w01/` for evidence, `<evidence-folder>/w01-toy/` for variant H0 of the guided lab; media recipe W01-R1 in the manifest |
| Linux account names | no dots (`useradd` may refuse them); synthetic placeholders such as `<synthetic-password>` only; no password is used anywhere |
| Test ids | T1 to T9 of `docs/curriculum/v2/LAB_ARCHITECTURE.md` section 11; row ids S-01 to S-09, F-01 to F-31, C-01 to C-03 of `docs/curriculum/v2/HOST_READINESS.md`; R-01 to R-07, X-01 to X-06, LA-01 to LA-07 of LAB_ARCHITECTURE |

## 3. Day by day: blocks, lab steps, fixtures, registry exercises

Minutes inside a block are estimates, not measurements. The blocks stay R 30, A 120, B 120, C 150, D 60 and nothing is added.

### D01, Monday 2026-11-16: privilege model, host diagnostics, isolation design

Suggested scope line ids: `W01-GUIDED-A` (class B, block B), `W01-GUIDED-B1` and `-B2` (classes D and E, block C), `W01-GUIDED-H` (class F, hosted substitute). Primary provider libvirt (documentation); secondary OverTheWire.

| Block | Content | Lab step and fixtures | Registry |
|---|---|---|---|
| R | no queue exists on day 1: warm-up tasks only (write from memory what you believe a process, a file mode and a network route are; one failed experiment from your own past in your words) | none | none |
| A | the 30-question entry diagnostic (plan section 7.1; about 50 min; a placement, not a mastery claim) then the Linux privilege model (real, effective, saved and filesystem IDs; capabilities; supplementary groups), UEFI to kernel to PID 1, a guest as a host process under a hypervisor, what the libvirt modular and monolithic daemons change, why a bridge is also a host interface | guided lab step 1 (scope and RoE, 15 min) | `w01-man7-credentials`, `w01-man7-capabilities`, `w01-libvirt-daemons`, `w01-libvirt-formatnetwork` |
| B | Stage 00: first the four synthetic hosts (25 min), then the real rows the tree selects, one at a time and approved; the arithmetic of fit on the synthetic budget before the real one; plan the two-guest lab | guided lab steps 2 and 3; `d01/host_diag_sample_01.txt` to `_04.txt`, `d01/host_budget_sample.txt` | `w01-libvirt-daemons`, `w01-ubuntu-libvirt`, `w01-htb-academy-setting-up` (UNKNOWN tier: optional reading, never a gate item) |
| C | local only if separately approved and Stage 00 is resolved: guided lab steps 4 and 5 and tests T8, T6, T7, T5. Otherwise the substitution: judge the two isolation evidence sets (45 min), write the paper topology and test plan (45 min), then hosted Bandit from level 0 for the remaining time (record the levels reached; they count toward the day-2 range) | `d01/isolation_evidence_set_01.txt`, `_02.txt`; `guided/netplan_*.yaml`, `guided/sysctl_lab.conf` | `w01-otw-bandit` (WARGAME, FREE), `w01-thm-linux-fundamentals-1` (ROOM, FREE_ACCOUNT, optional) |
| D | evidence manifest, baseline measurements, network diagram, unresolved blockers, flashcards | `EVIDENCE_CHECKLIST.md` day-1 rows | none |

The day needs a hands-on row in block B or C: use `w01-otw-bandit` (C). The day is not local-lab only.

### D02, Tuesday 2026-11-17: permissions, ownership, groups, pipes, shell semantics

Suggested scope ids: `W01-GUIDED-C` (class E, blocks A and D), `W01-GUIDED-H` (class F, blocks B and C). Primary OverTheWire; secondary pwn.college.

| Block | Content | Lab step and fixtures | Registry |
|---|---|---|---|
| R | the day-1 queue (one-day offset) | none | none |
| A | the access decision: class selection, search permission on every directory, the setgid, sticky and umask effects, ACL entries and the mask, group membership at login, mount and mandatory-access layers; shell semantics: pipes, redirection, exit status, word splitting. Worked drill 01 (45 min), then guided lab steps 6 and 7 or variant H0 (45 min). The common wrong models (a mode is the whole decision; adding a group changes running sessions; a wider file mode exposes the file; root can always write) each get a discriminating test | guided lab steps 6 and 7 or H0; `d02/perm_drill_01.txt` | `w01-man7-path-resolution`, `w01-man7-credentials`, `w01-man7-acl`, `w01-gnu-bash-manual` |
| B | Bandit levels 0 to 8 (page `bandit1.html` to `bandit9.html`; level N's task yields the password for level N plus one), 120 min, about 13 min a level (estimate). Record per level the access fact that mattered, never a password | none | `w01-otw-bandit-levels-0-8` (WARGAME, FREE) |
| C | unassisted Bandit progress with the 45-minute rule, then drill 02 with the deciding element written for each request | `d02/perm_drill_02.txt` | `w01-otw-bandit`, `w01-pwncollege-permissions` (MODULE, FREE_ACCOUNT, 45 to 75 min, optional) |
| D | guided lab steps 8 and 9 (priority if time is short: the negative and positive retests of step 9; the four tests of step 8 may be reduced to read and delete), or the H0 correction; the lab record; flashcards | guided lab steps 8 and 9 or H0 | none |

### D03, Wednesday 2026-11-18: systemd, services, users, sudo, SSH, AppArmor

Suggested scope ids: `W01-GUIDED-C` (extensions, class E), `W01-INDEPENDENT` (class E, block C), `W01-GUIDED-H` or a class F line for the hosted fallback. Primary OverTheWire; secondary pwn.college or TryHackMe.

| Block | Content | Lab step and fixtures | Registry |
|---|---|---|---|
| R | the day-2 queue and the day-1 queue | none | none |
| A | unit anatomy and drop-ins, enabled against active, the journal, users and credentials of a process, sudo grammar and the argument wildcard, SSH precedence and the StrictModes mechanism, the socket-activation caveat on some Ubuntu releases, AppArmor read-only, listening sockets (socket, bind, listen, accept; the kernel object behind a file descriptor) | guided extensions E1, E4, E5 (or paper: `d03/sudoers_sample.txt`, `d03/journal_ssh_sample.log`) | `w01-systemd-man-index`, `w01-systemd-exec`, `w01-sudoers-manual`, `w01-openssh-sshd-config`, `w01-ubuntu-ssh-socket-activation`, `w01-ubuntu-apparmor`, `w01-man7-bind`, `w01-man7-listen`, `w01-man7-ss` |
| B | about 70 min of Bandit levels 9 to 15 (pages `bandit10.html` to `bandit16.html`; say it in the guide), about 10 minutes a level; about 25 min of systemd and SSH investigation on the guest (extensions E2 and E3, read-only SSH and journal views); about 25 min to restore the guest (step 10) and provision the independent state at the end of the block | guided lab step 10; extensions E2, E3; `d03/journal_unit_sample.log`; provisioning from the medium with only `independent/` and `guided/g9toy_reportd.py` | `w01-otw-bandit-levels-9-15`, `w01-ubuntu-openssh-server`, `w01-systemd-analyze` |
| C | the independent lab, 150 minutes, no hint for 45. Fallbacks: the paper review of the fixtures, or `w01-htb-machine-meow` (HTB Starting Point, tier UNKNOWN, 45 to 90 min, class F, needs the platform's attack environment), or `w01-thm-linux-system-hardening` (tier UNKNOWN), or `w01-pwncollege-users` (30 to 45 min) | `LABS/INDEPENDENT_LAB.md`; the fixture folder `independent/` is opened only when the block starts | `w01-htb-machine-meow`, `w01-thm-linux-system-hardening`, `w01-pwncollege-users`, `w01-htb-platform-rules` |
| D | correct, retest, annotate logs; the baseline notes; flashcards | `d03/journal_ssh_sample.log` (annotation practice), `EVIDENCE_CHECKLIST.md` day-3 rows | `w01-thm-linux-process-analysis` (FREE_ACCOUNT, optional reading) |

### D04, Thursday 2026-11-19: Bash and Python, JSON, exceptions, subprocess safety

Suggested scope id: `D04-B` (class A: offline fixtures; any live run in the guest is a separate class E line). Primary Python Software Foundation (documentation); secondary CyberDefenders.

| Block | Content | Lab step and fixtures | Registry |
|---|---|---|---|
| R | queues of days 3, 2 and 1 | none | none |
| A | quoting and word splitting; argument lists against a shell string; timeouts; exit statuses; reading untrusted text: size limits, encoding, counts of what was rejected; JSON that is built, not concatenated; exceptions as part of the contract. Worked example on the clean log only | `d04/auth_sample_01.log` | `w01-python-subprocess`, `w01-gnu-bash-manual`, `w01-htb-academy-intro-bash` (UNKNOWN, optional), `w01-thm-bash-scripting` (UNKNOWN, optional) |
| B | design the inventory (accounts, processes, services, listeners) and the failed-authentication parser against the fixtures; state the output contract in your own schema; `w01-thm-python-basics` is the hosted warm-up if Python is rusty (FREE_ACCOUNT, estimate 45 min) | the passwd, group, proc, systemd and socket samples in `d04/` | `w01-python-subprocess`, `w01-thm-python-basics`, `w01-htb-academy-intro-python3` (UNKNOWN) |
| C | implement without a reference; inspect outputs and errors. The damaged log `d04/auth_sample_02.log` and `d04/passwd_sample_02` first appear here; do not use them in A or B. `w01-cyberdefenders-hammered` if the owner's tier opens it (UNKNOWN; its official walkthrough stays closed until the attempt is submitted; no free hosted equivalent was verified) | the damaged fixtures; recipe W01-R2 for a binary-damage case | `w01-cyberdefenders-hammered`, `w01-thm-linux-logs-investigations` (UNKNOWN) |
| D | negative tests: missing privilege (a copy of a fixture whose mode is removed), corrupted lines, absent utility; the ten acceptance tests of `EVIDENCE_CHECKLIST.md` | `EVIDENCE_CHECKLIST.md` IV1 to IV10 | none |

The day is not local-lab only; the hands-on row in B or C is `w01-thm-python-basics` or `w01-cyberdefenders-hammered`. State in the guide that block C rests on the local fixtures until a tier is confirmed (registry gap).

### D05, Friday 2026-11-20: knowledge exam and blind practical

Suggested scope ids: `W01-FRIDAY` (class E, local), `W01-FRIDAY-P` (class A, paper). Primary HTB Academy (review only after the answers are locked); the practical has no provider. Follow `docs/curriculum/v2/AUTHORING.md` for a Friday: block A refers to `ASSESSMENT.md` Part 1, B is the review or planning task, C the scored practical (Parts 2 and 3), D scoring and the report (Part 4). The day's quiz still has eight questions of its own, and its hint ladder keeps the four labelled lines, with line 1 saying that there is no hint in a scored attempt.

| Block | Content | Lab step and fixtures | Registry |
|---|---|---|---|
| R | the day-4 and day-2 queues (1-day and 3-day offsets); not part of the attempt | none | none |
| A | the knowledge exam; lock the answers and hash them at the end | `ASSESSMENT.md` | none |
| B | optional guide review only after the hash; the environment check (restore, tests current, the preparer's record sealed); no solution reading | `LABS/FRIDAY_PRACTICAL.md` preconditions | `w01-htb-academy-linux-fundamentals` (skills assessment, optional) |
| C | the practical in mode L, P or H; no hint, no key, no AI; the hash of the bundle at the end | `LABS/FRIDAY_PRACTICAL.md`; `friday/` (opened at the start of the clock) | `w01-otw-leviathan` (WARGAME, FREE) for the hosted pool; `w01-thm-linux-strength-training` final challenge only (UNKNOWN) |
| D | the report, the baseline note `W01-Linux-Baseline-and-Hardening.md`, scoring against the rubric, the scorecard | `EVIDENCE_CHECKLIST.md` day-5 and gate rows | none |

The exam author fills `ASSESSMENT.md` with: Part 1 (20 questions, the mix of the contract), Parts 2 and 3 paraphrasing `LABS/FRIDAY_PRACTICAL.md` without repeating its fault catalogue beyond the classes, Part 4, the remediation track, the submission. Competency of the assessment: `linux.blind-troubleshooting`, difficulty 3 (band 3 to 5), PROPOSED. Part 3 is of type F.

## 4. The eight seeds of section 7.2: where each lands

Each seed must appear recognisably among the week's day questions or Friday exam questions, with `Seed: W01-Qnn` on the Source line of its key. Planned placement (PROPOSED; the authors may move a seed if the question fits better elsewhere):

| Seed | Topic | Planned day and type | Supporting material |
|---|---|---|---|
| W01-Q01 | what controls whether a regular user can read a file | D02, an applied question with a listing in the question | `w01-man7-path-resolution`, `w01-man7-credentials`, `w01-man7-acl` |
| W01-Q02 | an unauthorized sudo rule as a boundary violation | D03, an applied question with a rule in the question | `w01-sudoers-manual` |
| W01-Q03 | how a process creates a listening TCP socket, the kernel abstraction | D03, a recall question | `w01-man7-bind`, `w01-man7-listen` |
| W01-Q04 | service status and enablement checked separately | D03, a recall question and one data question | `w01-systemd-man-index` |
| W01-Q05 | what distinguishes an isolated guest from one attached to a well-named switch | D01, an applied question with an evidence excerpt | `w01-libvirt-formatnetwork`, `LAB_ARCHITECTURE.md` section 11 |
| W01-Q06 | likely causes of a missing libvirt socket and the first diagnostic | D01, a recall question | `w01-libvirt-daemons`, `HOST_READINESS.md` section 7 |
| W01-Q07 | why malformed input and least privilege matter to an inventory script | D04, a recall or applied question | `w01-python-subprocess` |
| W01-Q08 | how a guest adapter or bridge can change an Ubuntu host's VPN policy | D01 (an applied question with route and policy-rule excerpts) or the Friday exam | `docs/curriculum/v2/HOST_READINESS.md` section 6.5, `LAB_ARCHITECTURE.md` section 7 |

Each day also has eight questions of its own in the mix five recall and three applied. Mechanism and evidence, never flags: a question that can be answered by knowing a command's name is rewritten. A question that repeats another in different words (inside the day, across the week, or with the Friday exam) is rewritten. The drills, host samples and journals of `LABS/fixtures/` are practice material, not question text.

## 5. What Thursday and Friday build on

Thursday builds on Tuesday and Wednesday. The fields of the inventory (account, groups, process identity and capabilities, unit state and user, listener and owner, the limits of an unprivileged view) are the facts the learner measured by hand on days 2 and 3. The failed-authentication lines are the same kind of record as the SSH excerpts of day 3. The negative tests (missing privilege, damaged input, absent tool) are the code-level counterpart of the day-3 lesson that a missing event or an empty list is not an absent thing. The inventory is also the enumeration tool of the Friday practical's local mode: the learner is expected to run it, unprivileged, in the guest.

Friday builds on all four days. Scope first and the approval trail come from day 1; the access decision method from day 2; configured against effective, enabled against active, delegation width and SSH precedence from day 3; the enumeration tool, the output contract and the habit of counting what was rejected from day 4. The baseline note asks for a one-command verification per element: the learner needs the commands of days 2 to 4. The Friday scenario differs from every guided and independent fixture by design; do not reveal which classes of fault each lab holds.

## 6. Hosted alternatives and blocked-lab wording

While Stage 00 is blocked nothing local exists. Each day guide gives the hosted or paper alternative named above, says what it cannot show, and labels every claim PROPOSED, INFERRED or NOT RUN as appropriate. The week's substitution table is in `WEEK_GUIDE.md` (section "Remediation"). A blocked lab is recorded BLOCKED, never TESTED or VERIFIED. Class F lines need the platform's terms read and dated by the owner; until then hosted work is unavailable.

## 7. Open points for the owner and the orchestrator

| Id | Point | Default until decided |
|---|---|---|
| O-W01-01 | the channel by which a guest's evidence leaves as text: `docs/curriculum/v2/LAB_ARCHITECTURE.md` defines one only for the malware workspace | typed notes or a console log; the day guides say so and do not assume a channel |
| O-W01-02 | how the read-only fixture media are built and attached (recipe W01-R1 names the tools as ASSUMED) | the learner types the short files by hand through the console |
| O-W01-03 | the guest operating system release: unit names, the SSH socket-activation default, package names and the presence of `getfacl` depend on it | ASSUMED Ubuntu Server LTS; the day guides tell the learner to read the guest's own manual pages first |
| O-W01-04 | whether the Bandit range ends at the page for level 16 (this design: yes, levels 9 to 15) | as designed |
| O-W01-05 | whether the host toy directory (variant H0) is class A, B or C | PROPOSED outside class C; each command needs individual approval |
| O-W01-06 | the HTB Academy module's section titles and tier, and the tier of every UNKNOWN registry entry | the registry labels stand; no gate depends on one |
| O-W01-07 | a person to prepare the Friday under protocol 1; if none, protocol 3 (self-prepared, never unseen) or mode H or P | stated on the scorecard |
| O-W01-08 | the builder lists this file as unexpected (AUTHORING.md names every file of a week) | keep or move after the days are written |
