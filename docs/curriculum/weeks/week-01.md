# Week 1 — Linux, Python and secure lab engineering

**Purpose.** Build the laboratory the next eleven weeks depend on and show, with evidence, that it is authorized, isolated and reproducible. Then make the two working tools of the program usable: the Linux command line (files, permissions, processes, logs) and Python for handling security data safely. Everything later is judged by the habits set here: state the scope, record the baseline, change one thing at a time, keep the evidence.

**Lab stack.** One Linux workstation with a hypervisor (KVM/QEMU with libvirt, or VirtualBox), 16 GB RAM or more (32 GB is needed from week 6), 250 GB of free SSD, Python 3.11 or newer with `venv`, and git. Guests attach only to an isolated virtual network. Evidence lives in `~/g9lab/evidence/`, owned by you, mode 0700, never inside a guest. Synthetic data only: no real credentials, no real personal data, no network targets that are not yours.

**Competencies.** `lab.authorization`, `linux.filesystem`, `linux.processes-services`, `python.secure-parsing`, `python.testing-fixtures`, `linux.evidence-completeness`, `linux.hardening-baseline`, `linux.blind-troubleshooting` (pack `linux-python-systems`).

**Accelerator.** An experienced administrator can take the Day 1 diagnostic first and treat days 2 and 3 as review; the Saturday assessment still happens, unchanged.

## D01 — Lab authorization and boundaries

**Hand in.** Lab design + baseline — a dated scope statement, a boundary diagram with the isolation proof, and the recorded clean state of the lab (snapshot ids, listening ports, services, file hashes) with your first diagnostic scores.

### Objectives
1. Define lab authorization and boundaries — write a one-page scope statement: owner, permitted systems and actions, forbidden targets, data rules, time window and stop condition [lab.authorization]
2. Map host/guest and evidence isolation — draw the host, guest, network and evidence-store boundaries and prove with commands that a guest cannot reach the internet, the host's services or the evidence store [lab.authorization]
3. Establish diagnostic baseline — record the clean state of the lab (snapshot ids, listening ports, running services, hashes of key files) and score yourself on the week's diagnostic [linux.hardening-baseline]

### Recall (15 min)
Nothing to recall yet, so take the entry check instead. Without notes, write one line each: What is the difference between authorization and scope? What makes a lab isolated rather than merely separate? Name three facts you would record to prove that a system returned to a known state.

### Theory (45 min)
- Authorization is permission from someone entitled to give it, for named systems, actions and times. Scope is the boundary of that permission. A lab is authorized because it is yours and stays inside what you wrote down; an engagement against anyone else's systems needs their written permission first.
- Isolation is demonstrated, not declared. List the trust zones (host, guests, evidence store), then the flows that are allowed between them. Every other flow must fail, and you show that it fails.
- A baseline is a recorded, reproducible known-good state. Without it you cannot tell drift from damage, and you cannot prove a restore worked.
- Read the planning and rules-of-engagement guidance in NIST SP 800-115 and note the elements a rules-of-engagement document names.

### Guided lab (120 min)
1. Create `~/g9lab/{vms,images,evidence,work,scripts}`; make `evidence` mode 0700. Record `uname -a`, `lsblk`, `ip -br address` into `evidence/host-0.txt` and its `sha256sum`.
2. Install the hypervisor. Define an isolated virtual network with no forwarding (in libvirt, a network definition without a `forward` element) and no DHCP uplink.
3. Create one minimal Linux guest named `lin-target`, attach it only to that network, and take a snapshot named `clean-0`.
4. Prove isolation from the guest: no default route to the internet (a request to a public address fails within three seconds), no access to host services other than the ones you list, no mount of the evidence directory. Record each command and its observed output.
5. Capture the baseline inside the guest: listening sockets (`ss -tulpn`), running units (`systemctl list-units --type=service --state=running`), installed package list, and `sha256sum` of `/etc` and of the binaries in `/usr/bin`. Store the files in `evidence/` and hash them from the host.
6. Take the diagnostic: for each of the eight competencies of the week, do one small task from memory and score it 0 (could not), 1 (with documentation), 2 (from memory). Keep the scores.

### Independent lab (120 min)
Without opening your guided-lab notes, build a second guest `lin-aux` and bring it to the same state: same isolation, same baseline capture. Then compare the two baselines and explain every difference as intended, drift or error. Finally write the rules of engagement for a pretend assessment of a web application you will build in week 3: three things in scope, three out of scope, who can stop it, how evidence is handled.

### Defence and verification (60 min)
Write a drift check (shell or Python) that recomputes the baseline of a guest and reports added, removed and changed items. Make three deliberate changes (a new listening port, an edited configuration file, a new user) and confirm that all three are reported and that an untouched guest reports nothing. Then restore `clean-0` and confirm the check is silent.

### Evidence and report (60 min)
Assemble the hand-in as a short report with these parts: the scope statement (one page, dated, signed by you as owner); the boundary diagram with allowed flows labelled; the isolation proof with commands, outputs and timestamps; the baseline files with their SHA-256 values; the diagnostic scores; and a list of what the baseline cannot show. It is accepted when a stranger could rebuild the lab and reach the same baseline from the report alone.

### Resources
- https://csrc.nist.gov/pubs/sp/800/115/final | External | SP 800-115 (September 2008) | NIST publication, freely available
- https://libvirt.org/formatnetwork.html | External | current documentation; pin the libvirt version you install | open documentation, ASSUMED
- https://man7.org/linux/man-pages/ | External | current man-pages project; pin the release you read | freely readable, ASSUMED

### Watch for
Calling a network "isolated" because it has no gateway configured on the guest: check the host's side too. Keeping evidence inside the machine you are testing. A baseline taken after you already changed something.

## D02 — Filesystem, permissions and ownership

**Hand in.** Permissions worksheet — a target access matrix, the commands that implement it, the observed result of every cell, and the diagnosis of one deliberate failure.

### Objectives
1. Explain filesystem hierarchy — place 15 given paths into their purpose in the Filesystem Hierarchy Standard and justify each placement without notes [linux.filesystem]
2. Demonstrate permission/ownership controls — configure owner, group, mode bits, the setgid and sticky bits and one POSIX ACL so that a stated access matrix holds, and verify every cell with test users [linux.filesystem]
3. Diagnose controlled access failure — from a "permission denied" report, find the failing path component and its exact cause (mode, owner, ACL, mount option or security module) and fix it with the smallest change [linux.filesystem, linux.processes-services]

### Recall (15 min)
Without notes: list the five elements of a scope statement. Why does the baseline not prove the system is secure? Which command showed you the guest had no route out, and what did it print?

### Theory (45 min)
- The hierarchy has purposes, not folders: `/etc` host configuration, `/var` variable state and logs, `/usr` installed software, `/run` runtime state, `/proc` and `/sys` kernel views, `/dev` devices, `/tmp` shared scratch space.
- Access is decided per path component: the kernel needs execute (search) permission on every directory on the way, then the right bit on the file. Read `path_resolution(7)` and `credentials(7)`.
- Mode bits apply to owner, group and others; setuid and setgid change the effective identity of an executable, setgid on a directory passes the group to new files, the sticky bit stops deleting files you do not own. An ACL adds named users and groups, limited by its mask.
- Permissions can also be refused by a mount option (`ro`, `noexec`, `nosuid`, `nodev`) or by a security module such as AppArmor or SELinux even when the mode bits allow the access.

### Guided lab (120 min)
1. Restore `clean-0`. Create users `alice`, `bob`, `carol` and a group `analysts` with `alice` and `bob`.
2. Build `/srv/case/{intake,work,reports}`. Write the target matrix first (a table of path by user with r, w, x): `intake` writable by analysts but files deletable only by their owners; `work` shared by the group with new files inheriting the group; `reports` readable by `carol` and not writable by her.
3. Implement it with `chown`, `chmod` (including setgid and the sticky bit), `setfacl` for `carol`, and verify each cell with `sudo -u USER test -r|-w|-x PATH` and `namei -l PATH`.
4. Fill the matrix with the observed result next to the intended one. Any difference is a finding.
5. Break one cell on purpose (remove execute on a parent directory) and write down the exact sequence you followed to find it.

### Independent lab (120 min)
Prepare three different access failures in a fresh clone of the guest (a directory without search permission, a script on a `noexec` mount, an ACL mask that hides a grant). Draw one at random without looking at the others, and diagnose it from the symptom and your method alone. Repeat with a second one. Record time to diagnosis and which instrument found the cause.

### Defence and verification (60 min)
Write a permission audit that lists, for a given tree, every world-writable file or directory without the sticky bit, every setuid or setgid file, and every file owned by no existing user. Run it on `/srv/case` and on the guest's `/usr` and `/etc`, explain each finding as expected or a problem, and confirm the audit reports your planted problem.

### Evidence and report (60 min)
Hand in the worksheet: matrix (intended and observed), commands, the diagnosis log of the deliberate failure, and the audit output with your reading of each finding. It is accepted when every matrix cell has an observed result and each difference is explained.

### Resources
- https://refspecs.linuxfoundation.org/fhs.shtml | External | FHS 3.0 | published standard, freely readable, ASSUMED
- https://man7.org/linux/man-pages/man7/path_resolution.7.html | External | current man-pages project | freely readable, ASSUMED
- https://man7.org/linux/man-pages/man5/acl.5.html | External | current man-pages project | freely readable, ASSUMED

### Watch for
Testing as root, which passes everything. Forgetting the ACL mask. Fixing a failure by `chmod 777`: the smallest change that satisfies the matrix is the answer.

## D03 — Processes, services and logging

**Hand in.** Host diagnostic report — a map of one service from unit file to process, sockets and restrictions; the places its events are recorded; and a replayable repair log.

### Objectives
1. Analyze processes/services — map one running service from unit file to process tree, open files, sockets, cgroup and user, and explain what restricts it [linux.processes-services]
2. Inspect system logging — locate where an event is recorded (journal, authentication log, audit), query it by unit, time and priority, and state what the logs cannot tell you [linux.processes-services, linux.evidence-completeness]
3. Reproduce troubleshooting workflow — break and repair a service in a documented sequence of hypothesis, test, observation, change and check that another person can replay [linux.evidence-completeness]

### Recall (15 min)
Without notes: what does the sticky bit do on a directory? What reads the ACL mask? Draw the access path from `alice` to `/srv/case/reports/x.txt` and name every permission checked.

### Theory (45 min)
- A service is a unit definition, a process (or tree) in a control group, an identity, a set of open files and sockets, and restrictions (sandboxing options, capabilities, resource limits).
- Logs are produced by different writers with different trust: the journal collects what services write to standard streams and the system logger; authentication events come from PAM and the SSH daemon; the audit subsystem records kernel-observed events by rule. Logs can be missing, rotated, clock-skewed or written by the thing you are investigating.
- A troubleshooting method beats intuition: state the symptom, form competing hypotheses, choose the test that separates them, change one thing, observe, check the symptom, write it down.

### Guided lab (120 min)
1. In `lin-target` install a small network service of your choice (for example a web server) and enable it.
2. Map it: `systemctl status`, `systemctl cat`, `ps -eo pid,ppid,user,cgroup,cmd --forest`, `ss -tulpn`, `ls -l /proc/PID/fd`, and the unit's sandboxing options from `systemd-analyze security UNIT`.
3. Find its events three ways: `journalctl -u UNIT --since`, the service's own log file, and an audit rule watching its configuration file. Trigger one event and find it in each place; note the differences in detail and timing.
4. Break the service three ways (wrong ownership of its configuration, a port already in use, a missing dependency). For each, write the symptom, the competing hypotheses, the test that separated them and the repair.

### Independent lab (120 min)
In a fresh clone, install a different service you have not used and prepare two faults for it. Draw one at random and diagnose it using only the method and the logs, writing the log as you go in the format of the guided lab. Then give your log to a clean clone and replay it step by step: it must reproduce the fault and the repair.

### Defence and verification (60 min)
Harden the service from step 2: restrict it with unit sandboxing options so that `systemd-analyze security` improves, then prove the service still works and that a deliberately forbidden action (writing outside its state directory) now fails. Write the one-line rationale for each option you added.

### Evidence and report (60 min)
Hand in the diagnostic report: service map, the three places the event appeared with a comparison, the three repair logs, and the hardening diff with its proof. It is accepted when the repair logs can be replayed by someone else and the report states at least two things the logs could not show.

### Resources
- https://www.freedesktop.org/software/systemd/man/ | External | current systemd manual pages; pin the systemd version of your guest | open documentation, ASSUMED
- https://man7.org/linux/man-pages/man7/cgroups.7.html | External | current man-pages project | freely readable, ASSUMED

### Watch for
Reading a log you cannot date. Changing two things at once. Confusing "the service started" with "the service works".

## D04 — Parsing security records in Python

**Hand in.** Parser tests and review — a parser for one record format, its fixtures, and a short review of what it refuses and why.

### Objectives
1. Parse structured security records in Python — write a parser for one log format (authentication log lines or JSON lines) that yields typed records with validated fields [python.secure-parsing]
2. Handle malformed inputs safely — define and test the behaviour for truncated, oversized, mis-encoded, duplicated and hostile lines without a crash, unbounded memory use or a silent drop [python.secure-parsing]
3. Test parser with fixtures — build a fixture suite (valid, boundary, malformed) that fails when the parser is deliberately broken [python.testing-fixtures]

### Recall (15 min)
Without notes: which three places record that a service started? Name two things a log cannot prove. State your troubleshooting method in six words, and write the one command that lists the sockets a service holds open.

### Theory (45 min)
- Parsing is a security boundary: input is hostile until it has been parsed into a type with limits. Decide the contract first: accepted forms, maximum sizes, character encoding, what happens to a rejected line (counted, quarantined, reported).
- Safe by default means small, explicit and testable: dataclasses or typed dictionaries, explicit `datetime` handling in UTC, no `eval`, no unbounded regular expressions on untrusted input, streaming rather than reading whole files.
- Tests are the specification. A fixture suite should be able to say "this input must be rejected" as clearly as "this one must be accepted".

### Guided lab (120 min)
1. Create a repository with `src/`, `tests/` and `fixtures/`; use a virtual environment and a pinned `requirements.txt`.
2. Write a streaming parser for one format. Required fields, types and ranges are written down first. Timestamps become timezone-aware UTC values.
3. Add a rejection path: each rejected line is counted by reason and kept (bounded) for review.
4. Add tests for ten valid lines and ten malformed ones. Run them.

### Independent lab (120 min)
Extend the parser to a second format without changing the first one's behaviour. Write a hostile fixture set: a 50 MB line, a line with invalid UTF-8, a line with embedded control characters that could forge a log entry, a duplicated record, a timestamp in the future, a field with a surprising type. State beforehand what the parser must do for each, then run them. Measure peak memory on the 50 MB line.

### Defence and verification (60 min)
Break the parser on purpose in four ways (accept a type it should refuse, drop the size limit, swallow an exception, mishandle timezones) and confirm the suite fails each time. Add a test that proves each defence by failing without it. Record which of your four breakages the original suite did not catch.

### Evidence and report (60 min)
Hand in the repository state (commit id), the test output, and a one-page review: contract, rejected-input table, memory measurement, and the breakages the suite missed with the tests you added. It is accepted when the suite fails for every deliberate breakage.

### Resources
- https://docs.python.org/3/ | External | Python 3.11 or newer documentation; use the version you run | open documentation, ASSUMED
- https://docs.pytest.org/ | External | current pytest documentation; pin the version you install | open documentation, ASSUMED

### Watch for
Parsing with a regular expression that backtracks on a long line. Local time with no zone. A test that passes because the fixture never reaches the code under test.

## D05 — Version control, runbooks and evidence completeness

**Hand in.** Technical runbook — a runbook that rebuilds the day-3 failure and repair on a fresh machine, and a gap list for the week's evidence.

### Objectives
1. Apply version-control concepts — keep the lab files, parser and notes in git with meaningful commits, develop a change on a branch and revert a bad change [python.testing-fixtures]
2. Produce reproducible runbook — write a runbook that a peer can follow to rebuild the day-3 failure and repair on a fresh guest with identical results [python.testing-fixtures, linux.evidence-completeness]
3. Assess evidence completeness — review your own week's evidence against a checklist of who, what, when, where, how, hash and limits, and list the gaps and what would close them [linux.evidence-completeness]

### Recall (15 min)
Without notes: what makes a parser's rejection path testable? Name two inputs that were dangerous in your own fixtures. What does the drift check from day 1 not detect?

### Theory (45 min)
- Version control records why as well as what: small commits, messages that state intent, branches for experiments, tags for states you will refer to. A secret committed once is committed forever; keep secrets out and rotate any that leak.
- A runbook is executable documentation: prerequisites, exact commands, expected output at each step, a stop condition, and a rollback. If a step needs judgement, say what the judgement is based on.
- Evidence is complete when a reader can answer: who produced it, what it shows, when, from where, by what method, with what integrity check, and what it does not show.

### Guided lab (120 min)
1. Put `~/g9lab/scripts`, the parser repository and your runbooks under git (the evidence directory is not committed; its hashes are).
2. Make a change on a branch, commit it in two steps, merge it, then introduce a defect, find the commit with `git bisect` or `git log -S`, and revert it.
3. Write the runbook for the day-3 failure of your choice, with expected output beside each command.
4. Execute your runbook on a fresh clone of `clean-0` exactly as written. Every step that did not work as written is a defect in the runbook: fix it and run again until the run is clean.

### Independent lab (120 min)
Give your runbook to a second environment: a clean clone, with a different user and no memory of your earlier session (restart the guest, discard your shell history). Follow it again as if you had not written it, and keep a list of every place you relied on something that was not written down. Then do the same for the drift check of day 1.

### Defence and verification (60 min)
Add a pre-commit check that refuses a commit containing common secret patterns (private key headers, long random-looking tokens, passwords in configuration) and prove it blocks a planted fake secret and allows a normal change. State what it cannot catch.

### Evidence and report (60 min)
Hand in the runbook, the git history excerpt that shows the branch, the bad change and its revert, and the completeness review: a table of every piece of evidence from days 1 to 4 with the seven questions answered, the gaps, and the action to close each. It is accepted when the runbook succeeded on a fresh guest and every gap has an action.

### Resources
- https://git-scm.com/book/en/v2 | External | Pro Git, 2nd edition | Creative Commons licence stated by the book, ASSUMED
- https://docs.python.org/3/library/hashlib.html | External | Python 3.11 or newer documentation | open documentation, ASSUMED

### Watch for
A runbook written from memory after the fact. Evidence without a hash or a time. Secrets in history.

## D06 — Week 1 assessment (blind practical)

**Hand in.** Week 1 assessment — the report of the blind practical: root causes, verified fixes, residual risk and the evidence for each statement.

### Objectives
1. Blind Linux troubleshooting — diagnose and repair a host with several planted faults (permissions, service, logging, resource) using only a documented method [linux.blind-troubleshooting]
2. Independent security-data analysis — answer questions about an unseen log set with a parser you adapt, stating confidence and what the data cannot show [python.secure-parsing, linux.blind-troubleshooting]
3. Report remediation and limitations — deliver a report with root causes, verified fixes, residual risk and the evidence behind each statement [linux.evidence-completeness]

### Blind practical (120 min)
Open the assessment in the application, start the attempt and read the task statement. Obtain the scenario from the source you prepared before the week (see the protocol in the task statement); do not look at the faults before the clock starts. Work only from your method, your tools and your own notes. Stop at 120 minutes.

### Remediation (60 min)
Grade yourself against the rubric criteria before you read any answer key. Then, for every criterion you scored below the level you wanted, find the cause in your method (not in the tool), fix the gap by a short targeted exercise, and redo the part of the practical that failed on a fresh copy of the scenario.

### Report (60 min)
Write the report in the structure of the task statement: summary, per-fault root cause with evidence, repair with its verification, residual risk, what you could not determine. Attach the evidence with its hashes.

### Performance review (60 min)
Compare your grade with the week's targets: independent execution without hints, root cause stated rather than symptom, evidence quality, verification of the fix, honest limitations. Record the three habits that cost you the most points and the one change to your method for next week.

### Assessment
- Code: W01-BLIND
- Title: Week 1 blind practical: Linux troubleshooting and security-log analysis
- Kind: BLIND_PRACTICAL
- Competency: linux.blind-troubleshooting
- Difficulty: 3

Task statement:
You have 120 minutes. You are given a Linux guest in your isolated lab that has been changed in ways you have not seen, and a set of authentication and service logs you have not read. Work only inside the lab and only on the objects named in your scope statement.

Scenario source (choose before the week begins and do not inspect): a guest and log set prepared by another person from the fault catalogue below, or, if you work alone, a guest in which a script you wrote on an earlier day but have not read since has applied three faults chosen with a die from the catalogue. Faults are drawn from: a permission or ownership change that breaks a service's access to its data; a service that fails to start or restarts in a loop; logging that has stopped, rotated away or been redirected; a resource exhaustion (disk, inodes, memory, process limit); a configuration change that opens a listening port that should not exist.

Deliverables, in this order. First, a written troubleshooting log made as you work: symptom, hypotheses, test, observation, change, check. Second, for every fault you found: the root cause (not the symptom), the smallest change that repairs it, and a check that proves the repair. Third, an analysis of the log set with your adapted parser: the questions you were asked answered with counts and examples, your confidence in each answer, and what the logs cannot show. Fourth, a report of at most three pages with the residual risk and the evidence hashes.

You are assessed on: independent execution without outside help during the block; root cause stated rather than symptom; quality and completeness of evidence; verification of each fix; honest statement of limitations. The pass rule is non-compensatory: every criterion must reach the pass level.

### Resources
- https://man7.org/linux/man-pages/ | External | current man-pages project; pin the release you read | freely readable, ASSUMED

### Watch for
Looking at the fault list before the clock starts, which makes the practical not blind. Fixing a symptom and stopping. Writing the report from memory instead of from the log.
