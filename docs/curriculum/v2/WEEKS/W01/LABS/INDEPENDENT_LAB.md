# Independent lab W01-INDEPENDENT: assess an audit-intake service and its SSH daemon against a written policy

- Status: PROPOSED. Nothing in this file was run. Commands that this lab proposes are listed with every column in the cleanup section; the assessment itself is yours to plan.
- Lab id: W01-INDEPENDENT. Week 1, day D03 block C (150 minutes, no hint for the first 45). Plan source: section 5 week 1, day 3 block C "assess a deliberately misconfigured SSH or service lab", and section 6 row 1 "different user, group and service context".
- This is the modified variant of `GUIDED_LAB.md`: the same mechanisms in a different context. Principals, groups, paths, service, port, SSH settings and the policy all differ from the guided lab. A task, not steps.
- Fixtures: `docs/curriculum/v2/WEEKS/W01/LABS/fixtures/independent/` and `d03/`. The learner-visible policy is `independent/access_policy.txt`. The provisioning file `independent/provision_independent.sh` plants the state: do not read it, or the configuration files it installs, before the attempt. Their hashes in `MANIFEST.md` are the commitment. The instructor map is in `ANSWER_KEYS/LABS_KEY.md`.
- Limit stated openly: the learner and the instructor are one person, so this lab is modified, not unseen (`docs/curriculum/v2/FIXTURES/README.md`, section 3.5). It differs from every guided fixture; it is not a scored attempt.

## Why it matters

Real assessments rarely start with "here is the fault". They start with a policy that says what should be true and a host whose state may or may not match it. The skill is to turn each policy line into a falsifiable check, to read the state the system actually has instead of the state somebody meant it to have, and to say which deviations matter and how much, no more and no less.

Three habits carry over from the guided lab and are tested here in another context. The configured value is not the effective value: a setting written in a file can lose to another setting that the program reads first. A permission fault has a boundary where it acts: one wrong group or mode on a directory can produce an availability symptom for one account and an exposure for another from a single cause. A delegation is as wide as its pattern: a rule that looks narrow can be matched by an argument string its author did not imagine. And a fourth is new: some exposures are not in the permission model at all, such as a value in a place that every account can read.

The lab ends where a finding ends: the correction acts on the cause, the permitted behaviour still works, and the forbidden behaviour fails in a near-miss form of the test that showed it.

## Scope and authorization

PROPOSED and unsigned. A line without a signed rules-of-engagement record is void.

```text
SCOPE W01-INDEPENDENT | TARGET G9-TGT-01 | CLASS E | ADDR 10.77.20.20 (the service and the SSH daemon are exercised only through 127.0.0.1 inside the guest) | FROM G9-TGT-01 console | WINDOW 2026-11-18T13:45/2026-11-18T16:15 America/Chicago | ALLOW read-only enumeration; access tests as the synthetic accounts named in the policy through sudo from the administrator account; changes under /srv/g9toy-audit, /etc/g9toy-audit, /etc/systemd/system/g9toy-audit.service, /etc/sudoers.d/40-ops, /etc/ssh/sshd_config and /etc/ssh/sshd_config.d/ with a configuration check before every reload; one SSH probe that sends no credential | DENY every other address, any password guess or authentication attempt with a password, any real key, any change outside the listed paths, package installation, the router and the other guests, scanning of any kind | STOP STOP-01..STOP-16 | ROE <record id> | TERMS none
```

Allowed accounts: `labadmin` (administrator), `svcaudit` (service), `labuser3`, `labuser4` (synthetic). No password is ever set or used. A finding is claimed only for a state you observed; a policy line you could not check is reported as NOT CHECKED, with the reason.

Stop conditions to watch: STOP-02 (any traffic to an address not in the scope line), STOP-03 (a listener or route that makes the guest reachable from outside its zone), STOP-05 (anything that looks like a real credential), STOP-07 (the 150 minutes end), STOP-11 (a change you cannot show reversed).

## Environment

| Item | Value |
|---|---|
| Guest | `G9-TGT-01`, DMZ `10.77.20.20/24`, one NIC, no route to the LAN or the Internet (isolation tests of the guided lab, part B, current for this guest) |
| Stack | the target alone: 2 vCPU, 3 GiB, 40 GiB thin; `G9-ATK-01` and the router stay powered off (a day guide may use fewer guests than the week's stack, never more) |
| Principals | `labadmin`, `svcaudit`, `labuser3` (group `auditors`), `labuser4` (group `ops`) |
| Services | `g9toy-audit.service` (the toy service of the guided lab in a second instance, TCP port 8090) and the guest's SSH daemon, reached over loopback |
| Policy | `independent/access_policy.txt`, six requirements R1 to R6 |
| Provisioning | `bash provision_independent.sh <fixtures-root>` as root in the guest from the read-only medium; it refuses to run unless `/etc/g9-lab-guest` names `G9-TGT-01`. It keeps the original `sshd_config` as `/root/g9-sshd_config.orig` and writes `/etc/g9-lab-state` with the time |
| Setup time, ESTIMATE, not measured | 20 to 30 minutes to build the medium, mount it and provision; not counted in the 150 minutes of the block |
| Task time | 150 minutes: 45 alone before any hint, then the hint ladder of the day guide; the last 20 minutes for the record |
| Tests current before intrusive work | T8, T6, T7 and T5 of `docs/curriculum/v2/LAB_ARCHITECTURE.md` section 11 for this guest, from the guided lab; if they are stale or NOT RUN, this lab is BLOCKED |

### Hosted alternative and paper variant when the local lab is blocked

Nothing local may be assumed. Replace the setup-dependent work only, record the substitution, and never write that a guest was assessed.

| Variant | What to practise | Evidence to keep | Estimate (ESTIMATE) | What it cannot show |
|---|---|---|---|---|
| paper: the fixtures as a configuration review | read `independent/access_policy.txt`, then the configuration files of the folder `independent/`; derive the effective value of each SSH setting by the rule of the program, derive who may do what on each path from the modes and groups you can infer, read the delegation rule as the program would match it | the effective-value table with your reasoning, one access decision per account and requirement, the findings in the record format, every claim labelled INFERRED because nothing was run | 90 to 120 min | any observed state; a fix you applied; a retest |
| hosted: `w01-htb-machine-meow` (HTB Starting Point, Tier 0; access UNKNOWN; needs an account and the lab VPN or the in-browser attack machine; class F) | reasoning about which exposed service's configuration is the weakness on a platform-designated target, by class and not by steps | the scope line, the terms-read date, the enumeration with sources and times, the class of weakness in your words, the independence declaration. No flag, no credential, no screenshot | 45 to 90 min | a fix and a retest of your own system |
| hosted: `w01-thm-linux-system-hardening` (TryHackMe; tier UNKNOWN, confirm on the room page before assigning) | SSH hardening, user accounts, services, firewall, audit settings | which setting you changed and what it should change in the effective behaviour, in your words | 45 to 90 min | the precedence of drop-in files, delegation rules, your own policy |
| hosted: `w01-pwncollege-users` (Untangling Users, free account) | user switching and delegation as mechanisms | the permission fact that mattered per challenge, in your words | 30 to 45 min | a flawed rule to assess |

Until the owner has recorded how a platform VPN profile is reached (open decision O-02 of SAFETY_AUTHORIZATION, default "browser-based environments only"), `w01-htb-machine-meow` is unavailable unless the in-browser attack machine is available to the owner's tier. Platform terms must be read and dated before first use (class F). Never publish a flag, a credential or a solution step.

## Preconditions

| Id | Condition | If not met |
|---|---|---|
| I1 | a signed RoE and the approved command list for the scope line | no command is run |
| I2 | `G9-TGT-01` is at its clean baseline (the guided lab's step 10 done, or a fresh baseline), with exactly one NIC and no guided-lab state | restore it first; two lab states in one guest confuse the evidence |
| I3 | the guest has `sudo`, `systemd`, `sshd` (the SSH daemon), `namei`, `ss`, Python 3 and `stat`; `getfacl` is optional | a missing tool is a missing dependency: install nothing, record the gap and use another source |
| I4 | the fixture medium for `independent/` and `guided/g9toy_reportd.py` exists; its hashes equal `MANIFEST.md` | rebuild the medium |
| I5 | before provisioning you wrote, outside the guest, the task plan: how you will turn each of R1 to R6 into a check | write it; a plan written after the first command earns nothing |
| I6 | a second copy of the original SSH configuration exists in the record, with its hash | provisioning keeps one in the guest, which a mistaken edit could also touch |

## Task

An audit-intake service and the SSH daemon of `G9-TGT-01` were set up for a small team. Two messages arrived. One says that `labuser3`, an auditor, cannot read the audit records. The other is a review request: "does this host meet the written policy?"

Assess the host against `independent/access_policy.txt`.

1. Before any action, write the scope block (targets, addresses, accounts, allowed actions, exclusions, time limit, stop rule) and a test plan that turns each requirement R1 to R6 into at least one falsifiable check and says what each check would show.
2. Enumerate the state with the access you have and the commands you approved. Every fact needs its source (command, file, line) and its time. Say what you did not or could not observe.
3. For each requirement, decide MET, NOT MET or NOT CHECKED, with the evidence. Distinguish a deviation from the policy from something that merely looks unusual.
4. For each deviation: state the cause at the control or trust boundary (what made it possible, not the symptom), the preconditions and privileges it needs, what it allows and what it does not, and the strongest alternative explanation with the test that separates them.
5. Correct at least the three deviations with the greatest exposure to the policy's intent. Each correction must act on the cause, state its side effects and rollback, and be followed by a positive retest (the permitted behaviour of the policy still works) and a negative retest (the forbidden behaviour fails, in a form that differs from the first test in one variable). List the remaining deviations with their proposed corrections and mark them NOT APPLIED.
6. Write the lab record of `docs/curriculum/v2/TEMPLATES/LAB_RECORD.md`, with the status the evidence supports, and a three-line baseline note for each corrected item: what the secure state is, how to verify it in one command, what would break it.

What cannot be assumed: that `sudo` works for every account, that a configuration file is the state the daemon runs with, that a service's owner is the account the policy names, that every unusual setting is a fault, that an absent log line means an absent action.

Catalogue of the classes of fault this lab may hold (the classes, not their instances): the effective value of a setting differs from the written one; a group or mode on a directory differs from the role; a delegation rule matches more than its author meant; a service runs or listens with more reach than its role needs; a secret sits where an unprivileged account can read it; a parent directory or an account's file mode blocks an operation that the target's own mode allows. Some items in the host are unusual and compliant. Report a compliant item as compliant, not as a finding.

## Positive validation

After your corrections, record with times and the artifacts that:

| Id | What must work | Why it is the positive case |
|---|---|---|
| IP1 | an auditor reads the records the policy lets auditors read | the correction must not remove the permitted access |
| IP2 | the service answers on its permitted address and port, as its service account, and reports a request count | the correction must not break the service |
| IP3 | an operator can restart the service through the delegation the policy allows and can place the one file the policy allows | the narrowed delegation must still do its job |
| IP4 | the SSH daemon's configuration validates and a reload succeeds; the effective values of the settings the policy names equal the policy's | the correction must leave the daemon running with the intended settings |
| IP5 | the administrator still has full administration | the corrections must not lock the host |

## Negative and safety validation

| Id | What must fail or stay absent | Why it separates apparent from verified success |
|---|---|---|
| IN1 | an operator can neither read nor change a record, and a record cannot be created, changed or removed by anyone but the service account | the same four operations as before the fix, one variable changed |
| IN2 | the delegation rule can no longer be made to write outside the one permitted file, tested with an argument string of the same class that showed the weakness, against a synthetic target inside the lab tree | a fix that blocks only the exact first test is a bypass waiting to happen |
| IN3 | the effective SSH configuration offers no password method, shown without sending any credential | the written value and the effective value are different facts |
| IN4 | the secret is not readable through the manager's property listing by an unprivileged account | an unprivileged account is the only observer that proves it |
| IN5 | no listener exists outside the permitted address and port | the socket inventory with the owning process, as administrator |
| IN6 | no change outside the listed paths, no password was set or tried, no real key exists, the guest has no new route or NIC | the command log against the approval record; the isolation tests re-run if anything network-related changed |
| IN7 | an over-tight correction (for example making every record root-only) fails IP1 and IP2 and is rejected | least privilege is the narrowest setting that still works, not the narrowest setting |

## Evidence

Kept in `<evidence-folder>/w01/independent/`, hashed before any key is opened. `docs/curriculum/v2/WEEKS/W01/EVIDENCE_CHECKLIST.md` gives the pass or fail test of each.

| Artifact | Content |
|---|---|
| scope block and test plan | timestamped before the first command |
| enumeration | the table of facts with source, time and zone; the list of what was not observed |
| hypothesis log | each hypothesis with its predicted observation, written before the test; revisions as dated added lines |
| requirement table | R1 to R6 with MET, NOT MET or NOT CHECKED and the deciding evidence |
| finding records | one per deviation in the format of `docs/curriculum/v2/TEMPLATES/PENTEST_FINDING.md`, status Confirmed, Unconfirmed or Not retested |
| correction record | the exact commands with status words, expected output, observed output, side effects, rollback |
| retest record | the positive and the negative retest of each corrected item, with times |
| command log and hint log | every command with the approval state; the minute and level of each hint, or "none" |
| lab record | the template, status set last; independence declaration: hint level, published solution read or not, AI assistant used or not |
| hash list | SHA-256 of every file and of the bundle with its time |

Reference observations and scoring are in `ANSWER_KEYS/LABS_KEY.md` under "Independent lab". Do not open them before the record is saved and hashed.

## Rollback and cleanup

Preferred: restore the guest to its clean baseline, after the evidence is copied out as text and hashed. If the baseline is not used, reverse in this order and prove each step.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| IC1 | `sudo systemctl disable --now g9toy-audit.service` | G9-TGT-01 | stops and disables the service (the provisioning enabled it) | removal of the enablement link and a stop | the service is gone | sudo in the guest | this lab's unit | `sudo systemctl enable --now g9toy-audit.service` |
| IC2 | `sudo rm /etc/systemd/system/g9toy-audit.service /etc/sudoers.d/40-ops` then `sudo rm -r /etc/g9toy-audit /srv/g9toy-audit /var/lib/g9toy-audit` then `sudo systemctl daemon-reload` | G9-TGT-01 | removes the unit, the delegation file, the configuration, the data and the state | no output | deletes only this lab's paths | sudo in the guest | the listed paths | reprovision from the medium |
| IC3 | `sudo rm /etc/ssh/sshd_config.d/10-image.conf /etc/ssh/sshd_config.d/60-hardening.conf` then `sudo cp -p /root/g9-sshd_config.orig /etc/ssh/sshd_config` then `sudo sshd -t` then `sudo systemctl reload ssh.service` | G9-TGT-01 | restores the guest's own SSH configuration; the check comes before the reload | no output from the check; the reload succeeds | the daemon runs with its original configuration | sudo in the guest | the three SSH paths | reprovision from the medium |
| IC4 | `sudo userdel --remove labuser3` then `sudo userdel --remove labuser4` then `sudo userdel svcaudit` then `sudo groupdel auditors` then `sudo groupdel ops` | G9-TGT-01 | removes the synthetic accounts and groups | no output | removes two home directories | sudo in the guest | the five names | reprovision from the medium |
| IC5 | `sudo rm /etc/g9-lab-state /root/g9-sshd_config.orig` | G9-TGT-01 | removes the provisioning's marker and the saved copy after the restore is proven | no output | none beyond the two files | sudo in the guest | the two files | none: keep the saved copy until the restore is proven |
| IC6 | `ss -ltn` then `getent passwd labuser3 labuser4 svcaudit` | G9-TGT-01 | proves the cleanup | no `8090` listener; the second command prints nothing and exits with status 2 | none | user | the guest | none needed (read-only) |

Then shut the guest down and re-run the tests the change could affect plus T8. A cleanup step that cannot be shown stops the exercise (STOP-11).
