# Week 5 — Host security and privilege boundaries

**Purpose.** A host is a set of principals and the rules that say what each can make the machine do. This week you learn to read those rules on Windows and on Linux, find the places where a low-privilege principal can become a higher one, record what a defender would see when that happens, and write down a hardened state that you can test by behaviour rather than by reading configuration. The Saturday assessment gives you a host you have not seen and asks for the same four things: diagnose, explain, repair, prove.

**Lab stack.** The week 1 Linux guests plus a Windows guest from a Microsoft evaluation image (verify the terms and the evaluation period when you download it; it must outlast the program), on the isolated network. Windows: PowerShell, the Sysinternals tools (Process Explorer, Autoruns, AccessChk, Sysmon) and event viewing with `Get-WinEvent`. Linux: `sudo`, `getcap`, `auditd`, `systemd-analyze`, Lynis or OpenSCAP. Every weakness in the week is one you plant yourself in a snapshot-restorable guest, with harmless markers instead of payloads.

**Competencies.** `win.principals-services`, `linux.privilege-boundaries`, `win.token-service-controls`, `host.persistence-telemetry` (pack `windows-identity-ad`), `linux.hardening-baseline` and `linux.evidence-completeness` revisited.

**Accelerator.** A systems administrator who can list a host's service accounts, capabilities and autostart locations from memory in the Monday recall block can treat days 25 and 28 as review; days 26, 27 and 29 and the Saturday assessment stay.

## D25 — Windows principals, services and event records

**Hand in.** Windows architecture review — the principals and services of the Windows guest drawn with their privileges, the session you reconstructed from event records, and the boundaries marked as security boundaries or conveniences.

### Objectives
1. Model Windows principals/services — draw the principals of a Windows host (users, groups, security identifiers, service accounts, virtual accounts, the system account) and the services that run as each, with the privileges and token groups that follow [win.principals-services]
2. Interpret event records — read logon, special-privilege, process-creation and service-installation events, with the right auditing enabled, and reconstruct what a session did [win.principals-services]
3. Map privilege boundaries — mark where one principal can act as another (service control, impersonation, scheduled tasks, remote procedure endpoints) and which of those boundaries the vendor treats as a security boundary [win.principals-services]

### Recall (15 min)
Without notes: which setting of a sudo rule makes it unsafe, and which capability would you refuse on an interpreter? List five places a host can be made to run code at boot. Which of your week 4 mitigations had a test that failed without it?

### Theory (45 min)
- Every principal has a security identifier. A process runs with an access token that lists the user, the groups, the privileges (enabled or not) and an integrity level. Services run under accounts chosen by their configuration: the system account, the local service and network service accounts, virtual accounts, or a named account.
- Auditing is off for much of what matters until you turn it on: logon and logoff, special privileges assigned to a new logon, process creation (with the command line), and service installation each have their own policy and event. System Monitor (Sysmon) adds process, network, file and registry events with a configuration of your choice.
- The vendor's servicing criteria state which boundaries it defends (for example between user mode and kernel, or between sessions) and which are conveniences (for example elevation prompts).
- Read the Windows security documentation on access tokens and logon types and the Sysmon page.

### Guided lab (120 min)
1. In the Windows guest enable advanced audit policy for logon, special logon, process creation with command-line logging, and security system extension (service installation). Install Sysmon with a configuration you have read line by line.
2. Create a local user, a group, a virtual-account service and a service under a named account. For each running service list the account and the privileges (`whoami /all` run as that account, `sc qc`, Process Explorer's token view).
3. Perform a short scripted session as the new user (log on, start a program, install a benign service, create a scheduled task). Collect the events and write the session's timeline with the event numbers.
4. Draw the principals-and-services diagram and mark, for each arrow, whether the boundary is a security boundary or a convenience, with your reason.

### Independent lab (120 min)
Prepare three benign session scripts on three clones (each performs a different mix of logon, process start, account change, service and task creation) and discard the scripts' text. Draw one at random and reconstruct what happened from the event logs alone. Then replay your reconstruction against the script and score it: what you got right, what the logs could not show.

### Defence and verification (60 min)
Write a PowerShell script that verifies the audit configuration is present (reads the policy, then proves it by generating a logon, a process start and a service installation and finding the events). Run it on the guest before and after you disable one policy and show it reports the gap. Add a summary of logon type and source for each successful logon in the last day.

### Evidence and report (60 min)
Hand in the diagram with boundary classification, the timeline of the session with event numbers, the auditing configuration and the verification output. It is accepted when every statement in the timeline cites an event and the diagram's classifications each carry a reason.

### Resources
- https://learn.microsoft.com/en-us/windows/security/ | External | current Windows security documentation; note the Windows release you read it for | vendor documentation, terms to be checked, ASSUMED
- https://learn.microsoft.com/en-us/sysinternals/downloads/sysmon | External | current Sysmon page; pin the Sysmon version you install | vendor documentation, terms to be checked, ASSUMED
- https://www.microsoft.com/en-us/msrc/windows-security-servicing-criteria | External | current servicing criteria; note the date you read it | vendor page, ASSUMED

### Watch for
Reading a logon event without its logon type. Assuming command-line auditing is on. Calling a prompt a boundary because it feels like one.

## D26 — Linux privilege boundaries

**Hand in.** Linux privilege review — the privilege paths found in the prepared guest with the exact reason each works, the smallest fix for each, and the tests as the low-privilege user.

### Objectives
1. Explain sudo/capabilities/service permissions — explain how sudo rules, file capabilities, setuid files and service sandboxing each grant or limit privilege and what a given rule really allows [linux.privilege-boundaries]
2. Analyze controlled Linux misconfiguration — find in a prepared guest the planted privilege paths (a sudo rule that allows an editor or interpreter, a writable file run by root, a capability on an interpreter, a setuid helper) and show in the lab how each would let a low-privilege user act as root [linux.privilege-boundaries]
3. Verify hardening — remove each path with the smallest change and prove as the low-privilege user that it no longer works while the legitimate task still does [linux.privilege-boundaries, linux.hardening-baseline]

### Recall (15 min)
Without notes: what does a Windows service's account decide? Name three events that record a service being created or a user gaining special privileges. What is the difference between a privilege and an access right?

### Theory (45 min)
- Root is a user identity with the kernel's permission checks bypassed; capabilities split that power into units, and a process or a file can hold some without being root. A setuid program runs with its owner's identity; one that can be made to start a shell or write a file is a path to root.
- A sudo rule is a grant of "run this command as that user". Wildcards, commands that can start other programs (editors, pagers, interpreters), environment settings that are kept, and writable directories on the command's path all widen it. Read the sudoers manual and keep a list of "commands that can break out" such as the GTFOBins catalogue.
- Privileged programs that execute files others can write (cron jobs, systemd units, scripts found through `PATH`) hand their identity to whoever can write the file.
- Containers change the picture: root in a container is not root on the host only if the configuration holds.

### Guided lab (120 min)
1. Build `lin-priv` from `clean-0` with four planted paths: a sudo rule for an editor, a root-owned cron script that is writable by a group your user belongs to, a file capability on a scripting interpreter, a setuid copy of a helper that starts programs.
2. As the low-privilege user find each with `sudo -l`, `find / -perm -4000 -type f 2>/dev/null`, `getcap -r / 2>/dev/null` and a review of root's scheduled jobs. For each, write why it works, then show in the lab, with a harmless marker (a file created by root), that it does.
3. Fix each with the smallest change: restrict the sudo rule to a fixed command, correct the file mode, remove the capability, remove the setuid bit or the program.
4. Re-run the discovery commands and the markers as the low-privilege user.

### Independent lab (120 min)
Prepare three more guests, each with two paths drawn from a list you wrote, and draw one at random. Find the paths without reading the list, using enumeration commands and logic only. For each, record the evidence, the reason it works and the fix. Then spend thirty minutes on the OverTheWire Bandit levels that involve cron and setuid programs and note how the same reasoning applies.

### Defence and verification (60 min)
Write an audit script that reports sudo rules that allow shells or interpreters, setuid and setgid files, file capabilities, files executed by cron or systemd that are writable by anyone but their owner, and world-writable directories on root's `PATH`. Run it on the planted guest (it must report all four paths) and on the fixed guest (it must be quiet for them), and compare with the baseline of day 1.

### Evidence and report (60 min)
Hand in the four paths with their reasons and evidence, the fixes, the before and after outputs of your audit, and the markers as proof. It is accepted when each path has a one-sentence reason it works and a test as the low-privilege user that fails after the fix.

### Resources
- https://www.sudo.ws/docs/man/sudoers.man/ | External | current sudoers manual; match it to the sudo version of your guest | open source documentation, ASSUMED
- https://man7.org/linux/man-pages/man7/capabilities.7.html | External | current man-pages project | freely readable, ASSUMED
- https://gtfobins.github.io/ | External | current catalogue; note the date you read it | open source project, licence to be checked, ASSUMED
- https://overthewire.org/wargames/bandit/ | External | current game; levels as of the day you play | free wargame, terms to be checked, ASSUMED

### Watch for
Testing as root, which finds nothing. A fix that removes the demonstration but not the reason. A sudo rule fixed for the command but not for its arguments.

## D27 — Windows tokens and service controls

**Hand in.** Windows privilege review — the weaknesses found in the prepared service configuration with evidence, the fix and test for each, and the telemetry that would show their abuse.

### Objectives
1. Analyze Windows token/service controls — explain how access tokens, privileges, integrity levels and service configuration (account, access control list, binary path, recovery actions) determine what a process can do [win.token-service-controls]
2. Investigate controlled privilege failure — find in a prepared Windows guest the planted weaknesses (an unquoted service path, a service directory writable by users, a service whose configuration users may change, a service account holding a dangerous privilege) and show the effect in the lab [win.token-service-controls]
3. Document remediation/detection — for each weakness write the fix, the test that proves it and the event or telemetry that would show its abuse [win.token-service-controls]

### Recall (15 min)
Without notes: draw the privilege paths of your Linux guest in one sentence each. Which of them needed a writable file and which a command allowed by sudo? Write the command that lists file capabilities.

### Theory (45 min)
- A token carries privileges such as the ability to impersonate another user or to debug processes; service accounts often hold some by design. A privilege that lets a process act with another principal's identity makes any flaw in that process a path to higher privilege.
- A service is a configuration: account, binary path, start type, an access control list on the service object, and access control lists on the files and directories it runs from. If a path with spaces is not quoted, Windows tries shorter names first; if a low-privilege user can write to any directory it passes through, or can change the service's configuration, the user chooses what the service account runs.
- Controls: quote paths, tighten directory and service access control lists, run services as the least privileged account that works (virtual accounts or managed service accounts), restrict what may run (application control in audit mode first), remove unneeded privileges.
- Read the documentation on access tokens, service security and access rights, and the AccessChk page.

### Guided lab (120 min)
1. Create two services on the Windows guest, one with an unquoted path under a directory with a space and a directory writable by a normal user, and one whose service object grants a normal user the right to change its configuration. Use a harmless program that writes its own identity to a file.
2. As a normal user enumerate with `sc qc`, `icacls` and `accesschk` and find both weaknesses without being told. Show the effect: the service account runs your harmless program.
3. Fix both: quote and relocate, tighten the access control lists, change the account. Repeat the enumeration and the demonstration and show they fail.
4. Turn on application control in audit mode, run the harmless program and find the audit events.

### Independent lab (120 min)
Prepare three variants on clones (a service on a user-writable path, a scheduled task running a writable script, a service account with an unneeded privilege). Draw one at random. Enumerate as a normal user using only built-in tools and the Sysinternals set, state the weakness and the evidence, then fix it. Record how long the enumeration took and what you would automate.

### Defence and verification (60 min)
Write a PowerShell audit that lists services with unquoted paths containing spaces, services running as accounts outside an allow-list, binary directories writable by non-administrators, and service access control entries that grant low-privilege groups configuration rights. Run it on the weak and fixed guests. Add the detection: the events and the Sysmon registry events that would show a service's path or account changing, and test that they appear.

### Evidence and report (60 min)
Hand in the enumeration output as the normal user, the three weaknesses with the harmless-program evidence, the fixes, the audit output before and after, and the telemetry for each. It is accepted when each weakness shows evidence before and a failing demonstration after.

### Resources
- https://learn.microsoft.com/en-us/windows/win32/secauthz/access-tokens | External | current Windows documentation | vendor documentation, terms to be checked, ASSUMED
- https://learn.microsoft.com/en-us/windows/win32/services/service-security-and-access-rights | External | current Windows documentation | vendor documentation, terms to be checked, ASSUMED
- https://learn.microsoft.com/en-us/sysinternals/downloads/accesschk | External | current AccessChk page; pin the version you run | vendor documentation, terms to be checked, ASSUMED
- https://lolbas-project.github.io/ | External | current catalogue; note the date you read it | open source project, licence to be checked, ASSUMED

### Watch for
Fixing the path and leaving the writable directory. Testing as an administrator. An audit that cannot read the access control lists it is judging.

## D28 — Persistence mechanisms and telemetry

**Hand in.** Host visibility matrix — each persistence mechanism by operating system, the telemetry that records its creation, whether that telemetry is on by default, the configuration needed, the check you wrote and whether it was tested.

### Objectives
1. Map common persistence mechanisms — catalogue where code can be made to run again after a restart or a logon on Windows and Linux (autostart keys and folders, services, scheduled tasks and timers, shell profiles, SSH keys, event subscriptions) and tie each to its technique name in a public knowledge base [host.persistence-telemetry]
2. Identify associated telemetry — for each mechanism state the log, event or file change that records its creation and use, whether it is on by default and what must be configured [host.persistence-telemetry]
3. Propose detection checks — write a baseline-and-compare check for five mechanisms that reports new or changed entries and test it by creating a benign instance of each [host.persistence-telemetry]

### Recall (15 min)
Without notes: which two unquoted-path conditions must hold for a service weakness to be usable? What does a virtual account give you over the system account? Name the events you would expect when a service is installed.

### Theory (45 min)
- Persistence is any way of making code run again without the actor returning. The same places are used legitimately: that is why the baseline matters. Windows: run keys, the startup folder, services, scheduled tasks, event subscriptions in the management instrumentation. Linux: systemd units and timers, cron, shell profile files, SSH authorized keys, preload files and udev rules.
- A public knowledge base of adversary techniques (ATT&CK) names these, groups them under the persistence tactic, and lists the data sources that can see them. Use it for vocabulary and coverage, not as a checklist to tick.
- Telemetry exists only if enabled: object-access and registry auditing, Sysmon registry and file events, the audit subsystem with watches on the right paths, file integrity monitoring. Know which of your hosts log which mechanism today and which silently do not.

### Guided lab (120 min)
1. On both guests create one benign instance of five mechanisms each (for example a run key, a scheduled task, a service, a systemd timer, an SSH key). Each writes a marker when it fires.
2. Find the creation of each in the telemetry. Where nothing was recorded, say why and enable the source.
3. Reboot or log on again and confirm each fired.
4. Fill in the visibility matrix for every mechanism you listed, with the technique name next to each.

### Independent lab (120 min)
Prepare three clones, each with three hidden benign persistence entries drawn from mechanisms you chose, and hide the list. Draw one clone at random and find all three using enumeration tools and logs only. Record what found each, what you almost missed and which mechanism your enumeration did not cover at all.

### Defence and verification (60 min)
Extend the drift check of day 1 into five persistence checks: unit and timer files, cron entries, shell profiles and authorized keys on Linux; run keys, scheduled tasks and services on Windows. Create a new benign instance of each after the baseline and show every check reports it; show the checks are silent on an unchanged host; name the mechanism you could not cover and why.

### Evidence and report (60 min)
Hand in the matrix, the evidence of each creation event, the five checks with their before and after output, and the list of mechanisms with no telemetry. It is accepted when each cell of the matrix is backed by an observation or says "no telemetry", and each check has been shown to fire.

### Resources
- https://attack.mitre.org/tactics/TA0003/ | External | ATT&CK persistence tactic; pin the ATT&CK release you read | MITRE terms of use, ASSUMED
- https://learn.microsoft.com/en-us/sysinternals/downloads/autoruns | External | current Autoruns page; pin the version you run | vendor documentation, terms to be checked, ASSUMED
- https://github.com/linux-audit/audit-documentation | External | current repository; note the commit you read | open source project, ASSUMED

### Watch for
A matrix filled from a list rather than from a test. Enabling logging after the event and calling it detection. A baseline taken on a host that was already compromised by your own earlier experiment.

## D29 — Hardened baseline

**Hand in.** Hardening verification — the hardened configuration of each guest as testable checks, the ranked list of failures with the reasoning, and behavioural tests of ten settings.

### Objectives
1. Establish hardened baseline — define a hardened configuration for a Linux and a Windows guest from a published benchmark, record it as testable checks and apply it [linux.hardening-baseline]
2. Prioritize host weaknesses — assess the guests against the baseline, group the failures by the risk they carry in your lab, and justify an order of repair with evidence [linux.hardening-baseline, linux.privilege-boundaries]
3. Test configuration controls — for ten chosen settings prove the setting is in effect by a behavioural test and not only by reading the configuration [linux.hardening-baseline, win.token-service-controls]

### Recall (15 min)
Without notes: name the five persistence mechanisms you checked and the telemetry for each. Which of your five checks would have missed the mechanism you did not cover? What does a baseline let you tell apart?

### Theory (45 min)
- A hardening baseline is a published, reviewable list of settings with a rationale (the CIS benchmarks, the DISA security technical implementation guides, the vendor's security baselines). Use one as a source of checks, not as a goal: every setting has a cost, and an applied setting that breaks the service will be undone.
- Prioritize by the risk in context: exposure (is the service reachable), privilege (what does a compromise give), and ease (does it need a user or only a network). Order the repairs, do the high-risk ones first, record the ones you accept and why.
- A setting is proven by behaviour: ask the service to do the forbidden thing and see it refuse. A setting that is only read from a file may not be in effect (overridden, not reloaded, wrong scope).
- Scanners (Lynis, OpenSCAP) give breadth and a first list; they do not replace a test you wrote and understood.

### Guided lab (120 min)
1. Pick a published benchmark for your Linux guest and one for the Windows guest. Choose forty checks that apply to your lab and record each as a check with an expected result.
2. Run a scanner (Lynis or OpenSCAP on Linux, a vendor baseline comparison on Windows) and your checks against the unhardened guests. Group the failures by risk and write the order of repair with reasons.
3. Apply the first twenty. Re-run and record what changed and what broke.
4. Choose ten settings and write a behavioural test for each: for example refuse a root login over the network, refuse a password login, fail to run a program from the temporary directory, refuse an unsigned script, find a blocked legacy protocol.

### Independent lab (120 min)
Harden a third guest of a different release with the same baseline using only your checks and tests. Compare it with the first: which settings differed, which tests failed first time, which checks you had to rewrite. Update your tests so they run on both.

### Defence and verification (60 min)
Turn the ten behavioural tests into a suite that runs in a minute (`pytest` for Linux, Pester or PowerShell for Windows). Run it on the hardened and unhardened guests. Update the day 1 drift check to the new baseline and show it reports a change to one hardened setting. Record what the suite cannot tell you (for example whether a setting would survive an update).

### Evidence and report (60 min)
Hand in the check list with results before and after, the ranked failures with reasoning, the accepted risks, the behavioural tests and their output, and the updated drift check. It is accepted when each of the ten settings has a test that fails on the unhardened guest and passes on the hardened one.

### Resources
- https://www.cisecurity.org/cis-benchmarks | External | benchmark edition for your operating system release; note the version | registration may be required and redistribution is restricted; terms to be checked, ASSUMED
- https://public.cyber.mil/stigs/ | External | STIG for your operating system release; note the version | US government publication, ASSUMED
- https://cisofy.com/lynis/ | External | current release; pin the version you run | open source project, ASSUMED
- https://www.open-scap.org/ | External | current release; pin the version you run | open source project, ASSUMED

### Watch for
Applying a benchmark wholesale and breaking the service. A test that reads the configuration file and calls it behaviour. Skipping the reason for each accepted risk.

## D30 — Week 5 assessment (blind practical)

**Hand in.** Week 5 assessment — the report of the blind practical: the privilege failures found, why each works, the controls applied and the evidence that they hold.

### Objectives
1. Diagnose unfamiliar host case — enumerate an unfamiliar host as a low-privilege principal and find the privilege paths and the persistence entries [linux.privilege-boundaries, win.token-service-controls]
2. Explain privilege failure — state for each path why it works (the grant, the writable object, the trust it abuses) in terms of the boundary it crosses [linux.privilege-boundaries]
3. Report controls and evidence — recommend the smallest fix and the telemetry for each, apply and test them, and state the residual risk with evidence [linux.evidence-completeness, host.persistence-telemetry]

### Blind practical (120 min)
Open the assessment in the application, start the attempt and read the task statement. Take the host from the source you fixed before the week and do not read its setup notes before the clock starts. Stop at 120 minutes.

### Remediation (60 min)
Grade yourself against the rubric before reading any answer key. For every criterion below your target find the cause in your method (the enumeration you skipped, the boundary you did not name) and repair it with a short exercise. Then redo the failed part on a fresh host.

### Report (60 min)
Write the report in the structure of the task statement: per path the evidence, the reason, the fix and its test, the telemetry, the persistence entries, residual risk and what you did not cover. Attach the evidence with SHA-256 values.

### Performance review (60 min)
Compare the grade with the week's targets: independent execution, enumeration as the low-privilege principal, reasons stated in terms of boundaries, tests that fail after the fix, honest limits. Record the three habits that cost you most and one change for next week.

### Assessment
- Code: W05-BLIND
- Title: Week 5 blind practical: privilege paths and persistence on an unfamiliar host
- Kind: BLIND_PRACTICAL
- Competency: linux.privilege-boundaries
- Difficulty: 3

Task statement:
You have 120 minutes. You are given a Linux guest, a Windows guest or both on your isolated lab network that you have not seen, with a low-privilege account to start from. Work only inside the lab and on the guests named in your scope statement. Use harmless markers; do not install anything that runs after the exercise.

Scenario source (fixed before the week, not inspected): a host prepared by another person from the catalogue below, or, working alone, a clone faulted by a script you wrote earlier but have not read since, with paths chosen by a die. Paths are drawn from: a permissive sudo rule; a root-run script or unit writable by others; a file capability or setuid file that can start programs; an unquoted service path or user-writable service directory; a service whose configuration a normal user can change; a service account with an unneeded privilege; benign persistence entries in at least two mechanisms.

Deliverables, in this order. First, an enumeration log made as the low-privilege principal: commands and findings. Second, for each privilege path the evidence, the reason it works in terms of the boundary it crosses, and a harmless demonstration. Third, the persistence entries found and what found them. Fourth, for each path the smallest fix, a test as the low-privilege principal that fails after it, and the telemetry that would show abuse. Fifth, residual risk and what you did not cover. Sixth, a report of at most four pages with the evidence hashes.

You are assessed on: independent execution during the block; enumeration performed as the low-privilege principal; reasons stated in terms of boundaries rather than tool output; tests that fail after the fix; evidence quality; honest limits. The pass rule is non-compensatory: every criterion must reach the pass level.

### Resources
- https://attack.mitre.org/tactics/TA0004/ | External | ATT&CK privilege escalation tactic; pin the ATT&CK release you read | MITRE terms of use, ASSUMED

### Watch for
Enumerating as root or as an administrator. Describing the tool output instead of the boundary. Leaving a planted entry in place after the assessment.
