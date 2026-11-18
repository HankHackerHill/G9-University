# Instructor key: G9 entry diagnostic (day 1)

Instructor only. Not part of the learner's view. The learner file is `docs/curriculum/v2/ASSESSMENTS/ENTRY_DIAGNOSTIC.md`.

- Reader: the owner, as the instructor, after the answers are locked by hash (section 1.1).
- Source of requirements: plan sections 0.3, 7.1, 8.1 and 9.2 of `docs/curriculum/v2/MASTER_PLAN_V2.0.md`; rules of `docs/curriculum/v2/AUTHORING.md`; marking style of `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`; track rule of `docs/curriculum/v2/STUDENT_GUIDE.md` (section 9).
- Status: PROPOSED, written 2026-11-18. No answer of the learner was scored, and nothing was run, deployed, scanned, installed or tested on the G9 host or in any lab. Every sample value is invented. Plan section 8.1 is quoted for each of E01 to E30 as "Plan concept" (rendered in ASCII); everything else in the key is expansion by the author.
- Labels: PROPOSED, ASSUMED, INFERRED, VERIFIED (only with evidence in the repository), NOT RUN. A statement about a product default or a standard that the registry does not hold is ASSUMED, with a pointer to `docs/curriculum/v2/RESOURCE_REGISTRY.md` (generated later from the week registries). Registry ids named below (for example `w08-rfc3227`) are the ids in the week registries as read on 2026-11-18; they are `SEARCH_CHECKED`, which means a search result was seen and the page was not opened. The generated registry wins over this file.

> **Non-execution statement.** Nothing here was run. A command or code that appears in this key is proposed text, with its row in section 8.3 where it is a command; it is never an instruction to run it and never says that running it is authorized. Reference values that depend on arithmetic or on the behaviour of a language were recomputed by the author with the Python 3.13 standard library on 2026-11-18 (a local computation, no lab, no host and no network); the script is not part of the repository, so those values are not VERIFIED in the sense of this repository. They can be recomputed by hand.

## 1. Opening and scoring rules

### 1.1 When to open

| Rule | Detail |
|---|---|
| Lock first | open this file only after the SHA-256 and the time of every answer file are written in the day 1 record (`docs/curriculum/v2/INSTRUCTOR_GUIDE.md`, section 6) |
| Log | write the time of the opening and the file in the key log |
| Early opening | a question seen before the lock is not placed: record it and score it 0. The placement of its domain is provisional |
| Never copied | the text of this file is never copied into the learner file, into a flashcard written beside an unsolved item, or into the evidence bundle of a Friday |
| Retest | the day 30 and day 60 retests are a new sitting with the same rules. This form is then seen (open point O-D-02) |

### 1.2 Scoring grammar

| Item | Rule |
|---|---|
| Multiple choice | 1 point for the correct letter, 0 otherwise. A letter marked `?` still scores. The reason written beside the letter is not scored; read it for the one-line miss analysis |
| Constructed answers (short answer, scenario, classification) | each question lists essential points (E) and supporting points (S) and its own Score line. Score 1: every E point and the number of S points the Score line asks for. Score 0.5: at least half of the E points (rounded up) but not enough for 1. Score 0: otherwise |
| Meaning, not words | credit an answer that states the same mechanism, evidence or limit in other words. Alternatives that are accepted are listed per question |
| Citing the sample | where the question says to cite or to use the sample, a point that ignores the sample is not earned. Plan section 7.1 asks that full responses cite packet fields, event sources or software properties |
| Overclaim | an overclaim listed for the question and stated as a fact caps the score at 0.5. The same text quoted as something that is wrong costs nothing |
| Part 3 | scored by the items of section 5, in half-point steps. A blank item scores 0. A listed overclaim removes that item's points |
| Points | Part 1 is worth 30, Part 2 is worth 10, Part 3 is worth 40. Part 3 is not added to Parts 1 and 2: it feeds the practical pass of section 6 |
| Recording | write the points of each question in the sheet of the learner file (section 5 of that file), and record the time taken, the questions marked `?`, and any environment note (plan section 9.1 asks for time, failures and confidence) |

### 1.3 What the points mean

The diagnostic is ungraded placement. There is no pass mark, no gate and no scorecard grade (plan section 9.2; `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md`, section 9). The points select a track per domain by the adaptive rule of plan section 1.3 and are compared with the day 30 and day 60 retests. A score is never reported as a level of skill, and a placement is never used to excuse or to skip an assessment. Item scores are noisy: a domain holds between four and six questions, so one question moves a four-question domain by 25 points.

## 2. Register of questions

Derived from the tag lines of the learner file. The competency ids are those of the eight packs under `data/competency-packs/*/pack.json`; every difficulty lies inside the band of its competency (checked by the author against `difficulty_band` of each competency on 2026-11-18). `none` means that no competency of the packs fits; ADR-0023 records that the packs hold none for reverse engineering, containers and cloud IAM.

| ID | Plan label | Domain | Competency | Difficulty | Format |
|---|---|---|---|---:|---|
| E01 | Linux | Linux and virtualization | `linux.filesystem` | 1 | multiple choice |
| E02 | Linux | Linux and virtualization | `linux.privilege-boundaries` | 3 | short answer |
| E03 | Linux | Linux and virtualization | `linux.processes-services` | 2 | multiple choice |
| E04 | Linux | Linux and virtualization | `linux.privilege-boundaries` | 3 | multiple choice |
| E05 | Network | Networking | `net.tcp-states` | 2 | multiple choice |
| E06 | Network | Networking | `net.layers-addressing` | 1 | multiple choice |
| E07 | Network | Networking | `recon.discovery-interpretation` | 3 | multiple choice |
| E08 | Network | Networking | `net.pcap-analysis` | 3 | multiple choice |
| E09 | Network | Networking | `net.routing-segmentation` | 3 | short answer |
| E10 | Python | Python | `web.injection` | 2 | short answer |
| E11 | Python | Python | `python.secure-parsing` | 3 | short answer |
| E12 | Windows | Windows and Active Directory | `win.principals-services` | 1 | multiple choice |
| E13 | Windows | Windows and Active Directory | none (nearest: `ad.forest-trust-model`) | 2 | short answer |
| E14 | AD | Windows and Active Directory | `ad.kerberos` | 3 | multiple choice |
| E15 | AD | Windows and Active Directory | `ad.delegation-permissions` | 3 | short answer |
| E16 | Web | Web and API | `web.injection` | 2 | multiple choice |
| E17 | Web | Web and API | `web.authz-idor` | 2 | multiple choice |
| E18 | Web | Web and API | `web.xss-contexts` | 2 | multiple choice |
| E19 | Detection | Detection and DFIR | `det.criteria-fp` | 3 | multiple choice |
| E20 | Detection | Detection and DFIR | `det.event-sources` | 3 | multiple choice |
| E21 | Applied Linux | Linux and virtualization | `linux.evidence-completeness` | 3 | scenario |
| E22 | Applied Network | Networking | `net.failure-diagnosis` | 4 | scenario |
| E23 | Applied Identity | Windows and Active Directory | `win.token-service-controls` | 3 | scenario |
| E24 | Applied Web | Web and API | `web.authz-idor` | 2 | scenario |
| E25 | Applied DFIR | Detection and DFIR | `det.normalization` | 3 | scenario |
| E26 | Applied RE | Reverse engineering and reporting | none | 4 | scenario |
| E27 | Applied Cloud | Cloud and AI | none | 4 | scenario |
| E28 | Applied AI | Cloud and AI | `agent.workflow-authority` | 3 | scenario |
| E29 | Applied KVM | Linux and virtualization | `linux.processes-services` | 2 | scenario |
| E30 | Reporting | Reverse engineering and reporting | none | 3 | scenario |
| E31 | added (DFIR) | Detection and DFIR | `dfir.chain-of-custody` | 2 | short answer |
| E32 | added (incident response) | Detection and DFIR | `ir.scoping-triage` | 3 | scenario |
| E33 | added (RE) | Reverse engineering and reporting | none | 4 | scenario |
| E34 | added (malware triage) | Reverse engineering and reporting | `malware.static-triage` | 3 | classification |
| E35 | added (containers) | Cloud and AI | none | 3 | multiple choice |
| E36 | added (reporting) | Reverse engineering and reporting | none | 2 | classification |
| E37 | added (detection engineering) | Detection and DFIR | `det.sigma-rules` | 4 | scenario |
| E38 | added (Python) | Python | `python.secure-parsing` | 3 | scenario |
| E39 | added (Python) | Python | `python.secure-parsing` | 3 | scenario |
| E40 | added (AI application) | Cloud and AI | `llm.output-schema-tools` | 4 | scenario |

Counts: 15 multiple choice (14 among E01 to E20, which meets "at least 12"; the fifteenth is E35), 7 short answer, 16 scenario, 2 classification. Difficulty 1: 3 questions; 2: 11; 3: 20; 4: 6; 5: none. Correct letters of the multiple-choice questions: A 4 (E03, E07, E14, E19), B 4 (E05, E12, E18, E35), C 3 (E01, E08, E16), D 4 (E04, E06, E17, E20).

Points available by domain (Parts 1 and 2 / Part 1 only): Linux and virtualization 6 / 6; Networking 6 / 6; Python 4 / 2; Windows and Active Directory 5 / 5; Web and API 4 / 4; Detection and DFIR 6 / 3; Reverse engineering and reporting 5 / 2; Cloud and AI 4 / 2.

## 3. Part 1 key: E01 to E30

### 3.1 E01 to E20

#### E01 (multiple choice, 1 point)

- Plan concept (8.1): "Owner read/write; group read; others none; mode does not override ACLs, mount or mandatory policies."
- Correct option: C.
- Why: `0640` is `rw-` `r--` `---`: the owner reads and writes, the group reads, everyone else has no access through the mode bits.
- Distractors: A is `0764` (`rwx` `rw-` `r--`). B is `0660` (the group may also write). D is `0460` (owner read only, group read and write).
- Limits to know (not needed for the point): the bits are one input to the decision. A directory in the path needs the search (execute) bit; an ACL entry or its mask can change the group class; a read-only mount, `noexec` or a mandatory policy can deny what the mode allows; `root` or a process with `CAP_DAC_OVERRIDE` bypasses the mode bits.

#### E02 (short answer, 1 / 0.5 / 0)

- Plan concept (8.1): "Real UID identifies invoking identity; effective UID controls many permission checks; SUID/service privilege contexts may differ."
- Essential: E1 the real UID is the identity of the user who started the process. E2 the effective UID is the identity the kernel uses for most permission checks (file access, and others). E3 one legitimate security consequence of a difference: a set-user-ID program runs with its owner's effective UID, so an ordinary user can perform one controlled privileged task (for example changing their own password entry), and a flaw in such a program is a privilege-escalation route; or a daemon starts as root to bind a privileged port and then drops to an unprivileged UID to limit the damage of a later compromise; or the real UID tells a privileged program who invoked it, so it can authorize the caller.
- Accepted alternatives: mention of the saved set-user-ID (it lets a program drop and later regain privilege); of the filesystem UID (it normally follows the effective UID); of `sudo` or `su` as the mechanism that creates the difference.
- Overclaims to reject: "an effective UID of 0 is unrestricted" (capabilities, namespaces and mandatory policy can still limit it); "the real UID decides file access"; "set-user-ID programs are insecure by design" (they are a controlled mechanism; the risk is in flaws and in their input handling).
- Score: 1 = E1, E2, E3. 0.5 = two of the three. 0 = fewer.

#### E03 (multiple choice, 1 point)

- Plan concept (8.1): "Enabled means intended boot activation; active describes present unit state; both may differ."
- Correct option: A.
- Why: `is-enabled` reports whether the unit is set to start at boot (or by its target or trigger); `is-active` reports its present state. `enabled` with `inactive` is an ordinary combination.
- Distractors: B is the opposite combination (`disabled` with `active`). C: a masked unit prints `masked` for `is-enabled`, not `enabled`. D is false: after boot an enabled unit can be inactive because an administrator stopped it, because a start condition was not met, because a one-shot unit has finished, or because it is socket-activated and has not been triggered yet.
- Overclaims to reject (in a written reason): "enabled means running"; "inactive means the unit failed" (a failed unit prints `failed`).

#### E04 (multiple choice, 1 point)

- Plan concept (8.1): "DAC decisions depend on ownership/discretionary permissions; MAC imposes policy independent of owner wishes."
- Correct option: D.
- Why: under DAC the owner (and privileged users) set the mode bits and ACLs. AppArmor confines a program with an administrator-loaded profile that the owner of an object cannot loosen; it is checked in addition to DAC, so both must allow the access.
- Distractors: A is false: AppArmor is a Linux Security Module enforced in the kernel; a user-space tool only loads profiles. B is false: a profile only adds restrictions and the DAC checks still apply. C is false: AppArmor mediates files, capabilities and network access, so the two overlap on the same operation.
- Notes: AppArmor is path-based and per program; a process with no profile is unconfined; a confined process running with effective UID 0 is still confined by its profile.

#### E05 (multiple choice, 1 point)

- Plan concept (8.1): "SYN, SYN-ACK, ACK; record state/sequence coordination."
- Correct option: B.
- Why: the client sends SYN with its initial sequence number, the server replies SYN-ACK with its own initial sequence number and an acknowledgment of the client's, and the client acknowledges the server's. Each side picks the initial sequence number of its own direction.
- Distractors: A is not a TCP exchange (FIN ends a connection). C is the TLS handshake, which runs after TCP is established. D has the right flags and a false claim: the server does not choose the client's sequence numbers.
- Notes: a simultaneous open and data carried in the SYN (TCP Fast Open) exist and are not "normal". Reference: RFC 9293 (TCP), ASSUMED standards knowledge; the evidence text of `w02-rfc8446` names it.

#### E06 (multiple choice, 1 point)

- Plan concept (8.1): "/24 = 24 network bits; traditional host range 254 usable (network/broadcast excluded); note special contexts."
- Correct option: D.
- Why: a /24 has 8 host bits, so 256 addresses; `10.77.20.0` is the network address and `10.77.20.255` the directed broadcast address, leaving 254 usable hosts (`10.77.20.1` to `10.77.20.254`).
- Distractors: A reverses the two parts (2^24 minus 2 is 16,777,214). B treats only the network address as reserved (255). C treats every address as assignable (256).
- Special contexts (S, credit if mentioned): a /31 point-to-point link has two usable addresses (RFC 3021), a /32 is a single host; in the lab one address of each range goes to the router port, leaving 253; `10.0.0.0/8` is private address space (RFC 1918). ASSUMED standards knowledge.

#### E07 (multiple choice, 1 point)

- Plan concept (8.1): "Open indicates reachable service response; filtered indicates state not determinable from probes due to filtering/no response; it does not prove no service."
- Correct option: A.
- Why: Nmap divides ports into six states; `filtered` means that Nmap cannot determine whether the port is open because packet filtering prevents its probes from reaching the port (`w02-nmap-port-scanning-basics`). It establishes neither that the port is closed nor that no service listens; it does not locate the filter.
- Distractors: B confuses the states with version detection. C confuses the states with host discovery. D is false: a filtered port may or may not have a listener.
- Notes: `closed` is a reset from the target; `open|filtered` and `closed|filtered` exist for scan types that cannot separate the cases.

#### E08 (multiple choice, 1 point)

- Plan concept (8.1): "IP/port, timings, handshake/certificate facts depending TLS and capture; not decrypted HTTP application content absent keys/decryption."
- Correct option: C.
- Why: addresses, ports, timing and record sizes are visible; the ClientHello is cleartext (versions, cipher suites offered, usually the server name indication); the server's choice is visible; whether the certificate is visible depends on the version (in TLS 1.3 it is encrypted after the ServerHello; ASSUMED standards knowledge, RFC 8446 is `w02-rfc8446`). The method, path, headers and body are inside encrypted records.
- Distractors: A is false (TLS protects the whole application stream). B is false: IP and TCP headers are not encrypted by TLS. D is false: the method and path are application data inside encrypted records, not in the record header.
- Notes: Encrypted Client Hello hides the name where it is deployed (ASSUMED); the server name is "usually" visible, which is why option C says so.

#### E09 (short answer, 1 / 0.5 / 0)

- Plan concept (8.1): "Firewall kill-switch policy and route selection must be evaluated under disconnect/failover; main routing table alone is incomplete."
- Essential: E1 when the tunnel interface goes down its route disappears and the next default route takes over; in the sample that is the second default route, through `eth0`, so traffic leaves through the physical gateway unless a policy stops it. E2 a kill switch is a policy that blocks non-tunnel egress; the metric only orders the routes while both exist, and the behaviour under disconnect or failover has to be tested, not read from the table.
- Supporting: S1 route selection takes the longest matching prefix first and uses the metric only between routes of equal specificity, so a more specific route or a policy-routing rule can send traffic around the tunnel. S2 other paths: IPv6 (an empty `ip -6 route` does not prove it is disabled), DNS, local-network traffic, container and virtual-machine bridges, applications bound to the physical interface. S3 the evidence that would settle it: a capture on the physical interface during a controlled disconnect, and the firewall counters, taken under an approved procedure.
- Not shown by the sample: it holds routes only; it cannot show firewall rules, so it neither proves nor disproves a kill switch.
- Overclaims to reject: "the lower metric guarantees no leak"; "the VPN client's setting proves the kill switch works".
- Score: 1 = E1, E2 and one S. 0.5 = E1 or E2 alone, or two S points. 0 = otherwise.

#### E10 (short answer, 1 / 0.5 / 0)

- Plan concept (8.1): "Shell interprets metacharacters and unsafe string composition, enabling unexpected commands; use argument arrays and validation."
- Essential: E1 with `shell=True` the string goes to a shell (`/bin/sh -c` on POSIX, the command interpreter on Windows), which interprets metacharacters (`;`, `|`, `&`, `$()`, backticks, redirection, globbing, newline). E2 untrusted text composed into that string can add or change commands, so it becomes command injection with the privileges of the Python process. E3 the mitigation: pass an argument list without a shell (`shell=False`, the default), validate input against an allow-list, avoid the shell by using library functions, and run with least privilege.
- Supporting: S1 `shlex.quote` is a last resort for POSIX; with an argument list the program can still be fed an argument that starts with `-` (argument injection), so validate or use `--`. Reference: `w01-python-subprocess` (SEARCH_CHECKED).
- Overclaims to reject: "`shell=False` makes the call safe in every case"; "escaping quotes is sufficient".
- Score: 1 = E1, E2, E3. 0.5 = two of the three. 0 = fewer.

#### E11 (short answer, 1 / 0.5 / 0)

- Plan concept (8.1): "Timeouts, exceptions, permission errors, malformed addresses, retries/rate limiting, route errors, scope limits."
- Points (credit any, count them): P1 timeouts on connect (and read), so the tool cannot hang. P2 distinct handling and reporting of the outcomes: refused (a reset), timeout (no answer), no route or unreachable, name-resolution failure, permission error; not collapsed into "closed". P3 validation of input: malformed addresses, ports outside 1 to 65535, a host name against a literal address. P4 scope control: an allow-list of authorized targets and ranges, checked on the resolved addresses (IPv4 and IPv6), refusing the rest. P5 rate and concurrency limits, and retries with back-off, so the tool does not flood a target. P6 resource handling and logging: sockets closed (context managers), specific exceptions (no bare `except: pass`), log of target, time and result without secrets, clear exit codes.
- Score: 1 = at least four of the six points. 0.5 = two or three. 0 = one or none.
- Overclaims to reject: "a try/except around everything is enough"; "run it as root so permission errors disappear".

#### E12 (multiple choice, 1 point)

- Plan concept (8.1): "Local account identities are machine-specific; AD principals come from directory domain, with domain authentication and policy context."
- Correct option: B.
- Why: a local account lives in the machine's own account database and is authenticated by that machine. A domain identity lives in the directory held by the domain controllers, is authenticated by a domain controller (Kerberos or NTLM) and can be used on any machine of the domain, subject to policy and permissions.
- Distractors: A swaps where the accounts live. C is false: they are different objects with different identifiers, and a password change on one does not change the other. D is false: local accounts have passwords and group memberships.
- Notes: a domain-joined machine also has local accounts; a cached logon can work while the domain controller is unreachable. Reference: the Microsoft AD DS documentation named in plan section 4 (ASSUMED here; the hub page is NOT_CONFIRMED in `W03/RESOURCES.json`).

#### E13 (short answer, 1 / 0.5 / 0)

- Plan concept (8.1): "GPO distributes policy to scoped targets; application depends on link/filtering, refresh, permissions, client processing, conflicts."
- Essential: E1 purpose: central, repeatable configuration and enforcement of settings (security and audit settings, registry-based policy, scripts, firewall, preferences) for the users and computers in scope. E2 one limitation, any of: it applies only to objects in the scope of a link (site, domain or OU) and not to a security group as such (groups only filter); clients must reach a domain controller and its SYSVOL share and process the policy at startup, sign-in or a background refresh, so a change is not instant; precedence and conflicts (order of local, site, domain and OU, Enforced, Block Inheritance); security or WMI filtering excludes objects; a local administrator can change local state between processing; preferences are not enforced like policy.
- Supporting: S1 a default background refresh of about 90 minutes with a random offset of up to 30 minutes for members, 5 minutes for domain controllers (ASSUMED; `w03-ms-group-policy-overview` records that GPOs link to sites, domains and OUs, that computer policy applies at startup and user policy at sign-in, and that there is a background refresh). S2 the policy is stored in the directory and in SYSVOL, so replication delay applies.
- Overclaims to reject: "a GPO can be applied to a security group" (it is applied through a container; a group can only filter); "a GPO prevents a local administrator from changing anything".
- Score: 1 = E1 and E2. 0.5 = one of them. 0 = neither.

#### E14 (multiple choice, 1 point)

- Plan concept (8.1): "TGT issued by KDC enables requesting further tickets; service ticket grants authenticated access to a specific service."
- Correct option: A.
- Why: after the client authenticates to the key distribution center it receives a TGT (encrypted for the KDC's `krbtgt` account, so the client cannot read it) that it presents to request service tickets without presenting its credentials again. A service ticket is issued for one service principal, encrypted with that service's key, and presented to that service.
- Distractors: B inverts the issuers. C is false: a TGT does not carry the password hash, and services never receive it. D is false: the two tickets have different purposes, keys and recipients.

#### E15 (short answer, 1 / 0.5 / 0)

- Plan concept (8.1): "Groups can inherit rights transitively through nested membership and ACEs; effective access may exceed intended direct grants."
- Points: P1 membership is transitive: a member of an inner group is effectively a member of every outer group, and the access token carries the security identifiers of all of them. P2 a permission given to an outer group, or an entry inherited by many objects, reaches every nested member; effective access is the union of all grants along every path, with explicit denies evaluated first. P3 the path is hidden: reading one ACL does not show who ends up with access, and the right to change a nested group's membership is itself a privilege path. P4 control: compute effective membership recursively, limit nesting depth, design groups by least privilege, restrict who can edit membership.
- Alternatives: a worked example (A in B in C, C holds a write entry) earns P1 and P2.
- Overclaims to reject: "nesting never matters because only direct entries count"; "deny entries always win over allow" (order of evaluation matters).
- Score: 1 = at least three of the four points. 0.5 = two. 0 = one or none.

#### E16 (multiple choice, 1 point)

- Plan concept (8.1): "SQL statement syntax remains separate from bound user parameters, preventing input from being parsed as query structure for that class."
- Correct option: C.
- Why: the statement with placeholders is fixed and parsed first; values are bound afterwards as data, so a value cannot change the structure of the statement.
- Distractors: A describes escaping, a different and more fragile technique. B limits impact and does not stop the injection. D is not a mechanism of any driver.
- Limits to know (S): a parameter cannot stand for an identifier (a table or column name, a sort direction), so those need an allow-list; dynamic SQL inside a stored procedure, or a value that is later concatenated (second-order injection), stays vulnerable. Reference: `w04-owasp-wstg-sql-injection`.

#### E17 (multiple choice, 1 point)

- Plan concept (8.1): "Authn verifies principal; authz verifies entitlement for *each* resource/action in context."
- Correct option: D.
- Why: authentication establishes which principal is calling; object-level authorization decides for each request whether that principal may perform that action on that object.
- Distractors: A is false: the server decides; the browser cannot be trusted with it. B puts session expiry (authentication state) under authorization. C inverts the two.

#### E18 (multiple choice, 1 point)

- Plan concept (8.1): "Reflected XSS uses server-reflected untrusted content; DOM XSS uses client-side unsafe source to sink flow (may be server-independent)."
- Correct option: B.
- Why: in reflected XSS the server puts request data into its response and the browser runs it as active content. In DOM-based XSS script already in the page reads an attacker-influenced source (the URL fragment, a query value, `postMessage` data) and writes it to a dangerous sink (`innerHTML`, `document.write`, `eval`), so the payload need not be in the server's response; a fragment is never sent to the server.
- Distractors: A describes stored XSS and invents a browser limit. C is false: neither runs on the server, and both reach a victim's browser. D is false: the method is not the distinction.

#### E19 (multiple choice, 1 point)

- Plan concept (8.1): "Negative cases estimate false positive noise and prevent broad matchers passing only by catching synthetic positives."
- Correct option: A.
- Why: a rule passes its positive test if it is broad enough, so only benign look-alikes show how much noise it makes and where its boundary lies.
- Distractors: B, C and D invent SIEM behaviour: no SIEM needs benign events to initialize its indexes; a positive event also shows collection; severity is a judgment, not a count of benign matches.

#### E20 (multiple choice, 1 point)

- Plan concept (8.1): "Network event category must be enabled/configured and collected; no event does not mean no traffic."
- Correct option: D.
- Why: Sysmon's network connection event (Event ID 3) is disabled by default because of volume and must be enabled in the configuration (ASSUMED; the plan states it, and `w07-ms-sysmon-events` and `w03-ms-sysmon-events` record a search result that says so; the page was not opened). Beyond that, an event reaches the SIEM only if Sysmon runs, its log is collected and the event is ingested. The absence of an event proves only that none was recorded.
- Distractors: A is false: Event ID 3 covers connections that a process initiates and connections it accepts (an `Initiated` field tells which; ASSUMED, Sysmon documentation). B is false: Event ID 1 is process creation. C is false: a firewall does not suppress Sysmon events.
- Notes: do not confuse Windows Security event ids with Sysmon ids (plan, week 3 caveat).

### 3.2 E21 to E30

For these questions the reference answer is derived from the sample data of the learner file, as plan section 8 requires ("the correct answer must reflect actual fixture data rather than a general guess").

#### E21 (scenario, 1 / 0.5 / 0)

- Plan concept (8.1): "May appear filtered/unreachable depending firewall response; verify capture, local listener, host firewall, route and scan method."
- Reference answer from the samples: the scanner is `10.77.10.10`. Port 22 is reported `open`: the listener inventory shows `sshd` on `0.0.0.0:22`, and the input chain accepts `tcp dport 22 ip saddr 10.77.10.10`, so the SYN is answered. Port 8080 is reported `filtered`: the listener inventory shows a process listening on `0.0.0.0:8080`, but no rule of the input chain accepts it, and `policy drop` discards the SYN without any reply, so a connect scan gets no answer. If the chain held a rule that rejects with a TCP reset, the scan would report `closed`; if it rejected with an ICMP administratively-prohibited error, `filtered`.
- Essential: E1 port 22 open, with the accepting rule named. E2 port 8080 filtered, with the policy drop (no accepting rule) named. E3 the listener line shows that a local listener exists, while the scan result does not show whether a service listens: `filtered` is not `closed` and does not prove absence. E4 at least two items of independent additional evidence (see below).
- Additional evidence (credit any two): a capture on the target interface or at the router showing the SYN arriving and nothing leaving; the firewall's counters or log for the dropped packets; a connection from the host itself over the loopback interface (the chain accepts `iif "lo"`) to show the service answers; the same listener inventory taken on the host; the route and path between the two hosts; the scan method and its timing and retries.
- Overclaims to reject: "8080 is reported closed" or "open"; "the scan proves that nothing listens on 8080"; "the firewall chain proves the port is unreachable from everywhere" (the sample shows one chain, not every table or hook).
- Score: 1 = E1 to E4. 0.5 = two or three of them. 0 = one or none.

#### E22 (scenario, 1 / 0.5 / 0)

- Plan concept (8.1): "Packet loss, filtering/drop, unreachable/no route, target down, asymmetric routing, capture point blind spot."
- Reading of the sample: four identical SYN segments (same source port, same sequence number) at +0, +1.003, +3.020 and +7.051 seconds. The gaps (1.003, 2.016, 4.031 s) double, which is the usual retransmission back-off of a TCP client whose SYN gets no answer (an initial timeout of one second is a common default; ASSUMED). The capture holds no SYN-ACK, no reset and no ICMP error. It was taken on the sender, so it shows what left and what came back to that interface.
- Plausible causes and an observation that would separate each (credit any three distinct causes): 1. a filter drops the SYN silently (the host's firewall or a router rule): firewall counters or logs, a capture on the target. 2. the SYN is lost in transit (a faulty link or congestion): captures at both ends and at the router; unlikely if the loss is constant. 3. the target is down or its interface is down: whether other ports or a host-discovery probe answer, the target's console, in scope only. 4. no route, or the next hop cannot be resolved, and no ICMP error is generated or returned (errors can be rate-limited or filtered): the router's routing and neighbour tables. 5. asymmetric return path: the SYN-ACK leaves by another interface and is dropped by a stateful device, or never returns: a capture on the target's egress and the router logs. 6. the target's listen queue is full, so the SYN is dropped (the kernel keeps counters of listen overflows and drops): the target's socket statistics. 7. a capture-point blind spot: the reply came to an interface or in a form that this capture does not hold: capture on the right interface without a restrictive filter.
- Not a cause: "the port is closed" (a closed port answers with a reset unless a filter drops it).
- Essential: E1 at least three distinct plausible causes. E2 for at least two of them, the observation that would support or exclude it. E3 reads the sample correctly (retransmissions of one SYN, no reply of any kind, one capture point).
- Overclaims to reject: one cause stated as certain from this capture; "the target is down" asserted as fact; "three connection attempts" (it is one attempt, retransmitted).
- Score: 1 = E1, E2, E3. 0.5 = E1 without E2, or E2 with two causes. 0 = otherwise.

#### E23 (scenario, 1 / 0.5 / 0)

- Plan concept (8.1): "Excessive inherited write permissions on privileged executable/configuration; remove unauthorized write ACE and verify proper owner/service trust."
- Reference answer from the samples: the service `svc-lab` starts automatically as `LocalSystem` and runs `C:\LabApps\svclab.exe`. The folder `C:\LabApps` carries an explicit entry that gives `CORP\Lab Users` Modify, inherited by files and subfolders (`(OI)(CI)(M)`), and the file holds it as an inherited entry (`(I)(M)`). `lab.user1` belongs to that group, so it can replace the file; the next start of the service runs the replacement with the highest local privilege. The cause is the combination of a write permission for ordinary users on the code of a high-privilege service. Correction: remove the Modify entry of `CORP\Lab Users` from the folder (or stop inheritance onto the service's folder and remove the inherited entry), keep full control for `SYSTEM` and `Administrators` and at most read and execute for others, and keep service binaries in a location that only administrators can write. A further correction is to run the service under a dedicated low-privilege account instead of `LocalSystem`. Verify with a positive test (an administrator can still update the file and the service starts) and a negative test (an attempt by `lab.user1` to write the file is denied; a file-access audit event exists where object-access auditing is on).
- Essential: E1 root cause with the entries cited. E2 impact: code execution as `LocalSystem` when the service starts (privilege escalation). E3 the least-privilege correction.
- Supporting (one needed for 1): S1 positive and negative retest. S2 a dedicated low-privilege service account. S3 binaries in an administrator-only location.
- Overclaims to reject: "removing `lab.user1` from the group fixes it" (the group is the grant; other members keep it); "mark the file read-only" (an attribute is not an access control entry); "change only the file's ACL" without stopping the inheritance (the entry returns at the next propagation, and new files inherit it).
- Score: 1 = E1, E2, E3 and one S. 0.5 = two of E1 to E3. 0 = fewer.

#### E24 (scenario, 1 / 0.5 / 0)

- Plan concept (8.1): "IDOR/BOLA; server must evaluate caller-to-object/action authorization rather than trusting client ID."
- Reference answer from the sample: the cookie identifies `lab.user1`, yet the second response returns invoice 1002 whose `owner` is `lab.user2`. Login (authentication) is not sufficient. The missing control is object-level authorization enforced on the server for every request and action: the principal comes from the session or token, never from a request parameter; the check compares it with the object's owner or access list (or the query is scoped to the principal); a failure answers 403 or 404 with no object data. It belongs in a central authorization layer, not in the client. Negative test: `lab.user1` requests 1002 and is refused; positive test: `lab.user1` still gets 1001; the write and delete methods are tested the same way.
- Essential: E1 login does not authorize each object. E2 the missing server-side object-level check keyed to the session's principal.
- Supporting (one needed for 1): S1 cites the sample (the owner field differs from the cookie's user). S2 a negative test with a second principal and a positive test for the owner.
- Overclaims to reject: "use random identifiers and the problem is solved" (obscurity, not a control); "hide the link in the page"; "a client-side check"; "CORS or a firewall stops it".
- Score: 1 = E1, E2 and one S. 0.5 = E1 or E2. 0 = neither.

#### E25 (scenario, 1 / 0.5 / 0)

- Plan concept (8.1): "Timestamp source, original value/offset/timezone, DST/clock drift, clock sync status, normalization method and provenance."
- Reference answer from the sample: (a) for each source record the original timestamp text as found, its format and precision (Log B has no year, no zone and a precision of one second), the zone or offset and how it is known (stated in the line, configured on the device, or assumed), the daylight-saving state on that date, the clock source and its synchronization state with any measured offset or drift, the time of collection or ingestion if it differs from the event time, the normalization rule and tool used, and the provenance (file, host, export, hash, who and when). (b) `Nov 16 08:07:30` in `America/Chicago` is Central Standard Time on that date (UTC minus 6 hours; daylight saving time ended on Sunday 2026-11-01), so Log B's line is `2026-11-16T14:07:30Z`, on the stated assumptions about zone and year. Log A is `14:07:31.402Z`, so the firewall line precedes the `sshd` line by 1.402 seconds as recorded. Because Log B has a precision of one second, the true gap is between about 0.4 and 1.4 seconds if both clocks agree, and the clocks of the two machines may differ by an unknown offset. The order is plausible (a firewall passes the SYN before `sshd` accepts a key after the handshake) and is not proven by the timestamps alone.
- Essential: E1 original value and zone or offset recorded, not only the normalized time. E2 clock source, synchronization state or drift. E3 Log B converted to `14:07:30Z` with the assumption stated. E4 the limit on the order (precision, unknown offset).
- Supporting (one needed for 1): S1 the daylight-saving rule and the missing year. S2 the normalization method. S3 provenance of each export.
- Overclaims to reject: "convert to UTC and discard the original"; "assume the router is on UTC"; "the timestamps prove the firewall event caused the login".
- Score: 1 = E1 to E4 and one S. 0.5 = two or three of E1 to E4. 0 = fewer. A conversion with the wrong offset earns no E3.

#### E26 (scenario, 1 / 0.5 / 0)

- Plan concept (8.1): "Reproduce with stable input, inspect root cause/control, assess reachability and attacker influence, mitigations/OS context; not all crashes are exploitable."
- Reference answer from the sample: what the note supports: a reproducible crash (5 of 5 runs, in two builds, signal 11) on a 23-byte input, which is an availability defect in the parser. What it does not support: that a memory-safety violation exists (a segmentation fault can be a null dereference, an out-of-bounds read or write, or a stack overflow), that an attacker controls the faulting address, the size or the data, that the code path is reachable by an attacker (the parser reads a file named on its command line and contains no network code, so remote exploitability would need a network-facing component that passes attacker data to it), and what mitigations the deployed build has. Tests: minimize and replay the input; rebuild with a sanitizer, or use a debugger, to learn the fault address, the instruction and whether it is a read or a write; read the source for the root cause (bounds, null, integer overflow, lifetime); vary the input bytes to see which of them control the fault; determine how the parser is deployed, with what privilege, and whether untrusted network input can reach it; test the deployed build with its mitigations (ASLR, PIE, NX, stack canaries, hardened allocator); after a fix, replay the input and a corpus as regression.
- Essential: E1 treats crash, vulnerability and remote exploitability as three claims with different evidence. E2 tests for a vulnerability (root cause and class, attacker control). E3 tests for remote exploitability (reachability, privilege and context, mitigations). E4 applies the sample (5 of 5, no network code, class unknown).
- Overclaims to reject: "a segmentation fault is remote code execution"; "a sanitizer report proves exploitability"; "no sanitizer report means safe".
- Score: 1 = E1 to E4. 0.5 = two or three of them. 0 = fewer.

#### E27 (scenario, 1 / 0.5 / 0)

- Plan concept (8.1): "Trust policy identifies who can assume role; permission policy specifies actions/resources the assumed role can use; both matter."
- Reference answer from the sample: Role `LabReaderA` has a trust-policy finding: its principal is `arn:aws:iam::111122223333:root`, which names the whole account (it is not the root user), so every principal of that account that also holds `sts:AssumeRole` permission for this role in its own policies can assume it; the intent was one principal, `lab.user1`. Its permission policy is narrow (`s3:GetObject` on the objects of `example-lab-reports`) and has no finding. Correction: name `arn:aws:iam::111122223333:user/lab.user1` (or one specific role) as the principal, add conditions where useful, and review which identities hold the permission to assume it. Role `LabReaderB` has a permission-policy finding: its trust policy names only `lab.user1`, as intended, but `s3:*` on `*` lets the session perform every storage action on every bucket and object (read, write, delete, change policies). Correction: `s3:GetObject` on `arn:aws:s3:::example-lab-reports/*`, plus `s3:ListBucket` on the bucket ARN only if listing is needed. A trust policy controls who may obtain a session of the role; a permission policy controls what that session may do. The effective access is the combination of both, and other layers (a permission boundary, a session policy, an organization policy, resource policies) can narrow it further.
- Essential: E1 Role A classified as a trust-policy finding, for its principal. E2 Role B classified as a permission-policy finding, for its action and resource wildcards. E3 states the distinction (who can assume the role, against what the session can do). E4 names a least-privilege correction for at least one role.
- Overclaims to reject: "Role B's trust policy is too broad" (it names one user); "`:root` means the root user" (it means the account); "if the permission policy is narrow the trust policy does not matter".
- Notes: AWS-style policy semantics are ASSUMED knowledge (not in the registry); plan section 4 names the AWS security pillar (`w11-aws-security-pillar`, ASSUMED). No account exists.
- Score: 1 = E1 to E4. 0.5 = two or three. 0 = fewer.

#### E28 (scenario, 1 / 0.5 / 0)

- Plan concept (8.1): "Lower-trust retrieved text not authority; deterministic tool allowlist/argument validation/capability checks, least privilege, audit, deny default."
- Reference answer from the sample: the chunk is data returned by retrieval; its text cannot grant a permission. The model may propose a call of `delete_backup`. A deterministic broker outside the model, which holds the credentials and decides from the authenticated user (`lab.user1`) and this agent's grant, refuses it because the tool is not in the grant, whatever the model says. Further layers: typed and validated arguments, read-only credentials for a read-only agent, approval for any write, a log entry for the denied call, deny by default. Test: simulate a fully compliant model that emits the call, and show that the broker denies it and that the backups are unchanged.
- Essential: E1 the retrieved text is data and never authority. E2 an enforcement point outside the model decides, by allow-list and the user's authority, deny by default.
- Supporting (one needed for 1): S1 typed arguments, least-privilege credentials, approval for sensitive writes. S2 audit of the denial, and a test with a compliant model that checks the downstream state. S3 explicitly does not rely on the prompt, on refusal or on filtering phrases.
- Overclaims to reject: "add a line to the system prompt telling the model to ignore instructions in documents" (not a control); "filter the phrase 'ignore previous instructions'" (incomplete); "the model will refuse".
- Score: 1 = E1, E2 and one S. 0.5 = E2, or E1 with one S. 0 = otherwise.

#### E29 (scenario, 1 / 0.5 / 0)

- Plan concept (8.1): "Missing/uninstalled unit, inactive socket/daemon, modular daemon/URI mismatch, deployment configuration; safely inspect systemd libvirt matching unit files."
- Reference answer: the message says that the path does not exist (a permission problem prints "Permission denied"). Plausible explanations: no libvirt daemon package is installed; a daemon is installed but its socket or service is not enabled or started (socket activation not triggered, or the service failed); a modular deployment, where `virtqemud` listens on `/run/libvirt/virtqemud-sock` and nothing provides the legacy `libvirt-sock` (the legacy path is provided by the monolithic `libvirtd`, or by `virtproxyd` in a modular host; ASSUMED, and the plan's own diagnostic lists `virtproxyd`); a different connection URI than the one the client used (system against session); the client runs in a container or another mount namespace that lacks `/run/libvirt`. The registry records that both daemon styles are built by default and that `virtqemud` is socket-activated (`w01-libvirt-daemons`, SEARCH_CHECKED). A safe next diagnostic is read-only: list the installed libvirt unit files and their state (the row in section 8.3), which changes nothing. The plan's rule applies: the missing socket is an investigation, not a permission to install or restart packages, and follow-up diagnostics are chosen from the returned evidence.
- Essential: E1 at least three plausible explanations. E2 a safe next diagnostic that is read-only (an inspection of units and their state), proposed and not run.
- Supporting (one needed for 1): S1 "no such file" is a missing path, unlike a permission error. S2 the modular daemon and URI mismatch (`virtqemud-sock` against `libvirt-sock`). S3 the plan's no-action rule (no install, no restart before the cause is known).
- Overclaims to reject: "install `libvirt-daemon` and restart it" as a first step; "KVM is broken" or "the CPU lacks virtualization" (the missing socket says nothing about either).
- Score: 1 = E1, E2 and one S. 0.5 = E1 alone, or E2 with two explanations. 0 = otherwise.

#### E30 (scenario, 1 / 0.5 / 0)

- Plan concept (8.1): "Mark unverified/inconclusive, provide supporting and contradictory evidence and missing reproduction conditions; never label confirmed critical from scanner only."
- Reference answer from the sample: list the item as scanner-reported and Unconfirmed (not reproduced), not as a confirmed finding and not as a false positive. Show the scanner's severity (Critical) attributed to the scanner, with its name, version and check identifier (the sample does not record them, which is itself a gap); give the tester's own assessment as pending or not assessed. Record the evidence both ways: the banner `ExampleHTTPd/2.4.1` on port 8000 supports it weakly (a banner is not proof of the build; a vendor fix can keep the banner); the manual test (two requests, both HTTP 404, no state change) is contradicting evidence for that trigger, with what was done, when, against what, in scope. Record the missing conditions: the exact build and patch level were not inspected; the preconditions of the issue (configuration, authentication, the exact trigger) are unknown; coverage is limited to two requests. State the next verification step (an authenticated check of the installed build or the vendor's advisory, a retest with the right preconditions) and a compensating control if the risk is worth it.
- Essential: E1 status Unconfirmed or unverified, not Confirmed. E2 the severity is attributed to the scanner and not adopted as the tester's own.
- Supporting (two needed for 1): S1 evidence recorded both ways, and what the manual test did. S2 missing reproduction conditions named. S3 a next verification step. S4 does not call it a false positive without proof (for example a verified patched build).
- Overclaims to reject: "Confirmed Critical" or "Critical" as the report's own severity; "false positive" with no evidence; deleting the item from the report.
- Score: 1 = E1, E2 and two S. 0.5 = E1 and E2 only, or one E with two S. 0 = otherwise.

## 4. Part 2 key: E31 to E40

### 4.1 What the 30 questions leave thin, and the choice

Counts are those of the 30 questions of the plan by the scoring domains of `docs/curriculum/v2/STUDENT_GUIDE.md` (section 9). A domain with two or three questions places a learner by 50 or 33 points per question; Part 2 gives the thin domains more questions, on topics the 30 do not reach, and ties each to a day of the plan.

| Domain | Questions in the 30 | Topics the 30 do not reach | Added | Plan day it prepares | Domain total |
|---|---:|---|---|---|---:|
| Python | 2 | reading code for path containment; parsing malformed log lines | E38, E39 | day 4 (parsing, exceptions, subprocess safety), day 18 | 4 |
| Detection and DFIR | 3 | evidence integrity and custody; volatility and containment; reading a detection rule | E31, E32, E37 | days 33, 36, 39 | 6 |
| Reverse engineering and reporting | 2 | reading source for a memory-safety defect; static triage; labelling evidence in a finding | E33, E34, E36 | days 41, 44; the reports of days 30 and 60 | 5 |
| Cloud and AI | 2 | containers; structured model output used downstream | E35, E40 | days 52, 54, 55 | 4 |
| Linux and virtualization, Networking, Windows and Active Directory, Web and API | 6, 6, 5, 4 | not extended: each already holds four to six questions, and Part 3 adds a practical item for each | none | | 6, 6, 5, 4 |

Still thin after Part 2, and left so on purpose (the cap is ten questions, and the weekly quizzes cover them): DNS resolution and caching, cookies, CORS and token validation, Windows event identifiers beyond Task 4, software supply chain and SBOM, memory forensics, cloud networking and storage exposure, data leakage of language-model applications.

### 4.2 E31 to E40

#### E31 (short answer, 1 / 0.5 / 0)

- Not in the plan. Related plan seed: W08-Q01, "observed bytes unchanged relative to prior hash, not proof original acquisition accurate" (plan section 8.2).
- Essential: E1 what it establishes: the copy holds the same bytes as the file that produced the reference value; with SHA-256, a coincidental match is negligible. E2 what it does not establish: that the image faithfully represents the original device (write-blocking, a complete read, read errors), who made it and when, that the reference value is authentic when it travelled beside the file through a channel that could alter both, and that the content is benign. E3 at least four custody facts: who handled it and when (with the zone); from whom to whom and how it was transferred; the hash value, the algorithm and the tool and version that computed it; where and on what media it is stored; working copy against original and read-only handling; the source device and the acquisition method (tool, write blocker).
- Overclaims to reject: "the hash proves the evidence is authentic or admissible"; "a match proves nothing changed since the incident" (only since the hash was computed); "MD5 is enough" (the question names SHA-256; a collision-prone hash cannot show integrity against an adversary).
- Score: 1 = E1, E2, E3. 0.5 = two of the three. 0 = fewer.

#### E32 (scenario, 1 / 0.5 / 0)

- Not in the plan. Related plan seed: W08-Q07 ("Which containment action could destroy valuable volatile evidence?").
- Reference answer: collect the most volatile evidence first and do not shut the machine down before collection is complete (RFC 3227, `w08-rfc3227`, SEARCH_CHECKED): the contents of memory, the process list and the network connections first (small snapshots of processes and connections can precede or follow a memory capture, and the memory image holds them too), then temporary file systems, then the disk, then remote logs and archived media. Containment that keeps evidence: cut the guest's network path (disconnect the virtual interface, move it to an isolated switch, block it at the router) and capture memory through the hypervisor or an approved tool before any restart. Actions that destroy evidence: power-off or reboot (memory and temporary file systems are lost, shutdown scripts run), killing the process before it is captured, deleting files, cleaning with an antivirus tool, reverting to a snapshot or reinstalling, and a long live session that changes state. Record every action with its UTC time, the command, the operator, and the hash of each capture; work on copies.
- Essential: E1 volatility order, memory and live state before disk. E2 containment that preserves evidence (isolate the network; no power-off or reboot first). E3 the actions that destroy evidence.
- Supporting (one needed for 1): S1 a record of every action with time and operator, hashes, copies, minimal footprint; the trade-off between containing quickly and watching the activity is a documented decision. S2 scope and authority: a lab guest, the owner's approval; an unexpected network connection from a lab guest is an isolation event for the incident procedure of `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` (stop, contain within approved authority, preserve evidence, report).
- Overclaims to reject: "power off at once is always the safest"; "a disk image is enough, memory does not matter"; "kill the process first".
- Score: 1 = E1 to E3 and one S. 0.5 = two of E1 to E3. 0 = fewer.

#### E33 (scenario, 1 / 0.5 / 0)

- Not in the plan. Related plan seeds: W09-Q04 and W09-Q06 (sanitizers; a crash is not remote code execution).
- Reference answer: (a) an out-of-bounds write (a classic buffer overflow caused by an unchecked length) at line 9: `memcpy` copies `out->len` bytes, taken from the first input byte, into a 16-byte array. (b) A first input byte greater than 16 (up to 255) makes the copy longer than `data`. If the input is shorter than 1 plus that byte, `memcpy` also reads past the end of the input buffer (an over-read), and line 8 assumes the input holds at least one byte. (c) An AddressSanitizer build would report a buffer overflow with a WRITE of the copied size at line 9, as a heap-buffer-overflow or a stack-buffer-overflow depending on where `out` lives (`data` is the last member, so the bytes past it leave the object), and possibly a READ overflow of `in`. The report shows that an invalid access happened for that input; it does not prove that the attacker controls what is written beyond the input bytes, that the process can be redirected, or any impact; it covers only inputs that were run. (d) Fix: pass the input length; reject when it is below 1, when the first byte exceeds `sizeof out->data`, or when the input holds fewer than 1 plus that many bytes. Regression tests: length 16 accepted; 17 and 255 rejected; an empty input rejected; an input one byte too short rejected; and the original failing input replayed under the sanitizer.
- Essential: all four of (a) to (d).
- Overclaims to reject: "the sanitizer report proves remote code execution"; "cast `len` to `int`" as the fix; "bound the copy only with the destination size and ignore the input length".
- Score: 1 = all four points. 0.5 = two or three. 0 = one or none.

#### E34 (classification, 1 / 0.5 / 0)

- Not in the plan. Related plan seed: W09-Q01 and day 41 ("document what static properties do and do not prove").
- Reference classification from the notes:

| Statement | Label | Reason |
|---|---|---|
| S1 a 64-bit x86-64 ELF executable | OBSERVED | the `file type` line says so |
| S2 imports networking and process-starting functions | OBSERVED | `socket`, `connect`, `send` and `fork`, `execve` are in the import list |
| S3 connects to a remote server when run | INFERRED | the imports and the strings (`GET / HTTP/1.0`, `%s:%d`, `usage: %s host port`) show a capability and a likely use; an import is not a call, and only disassembly or a safe dynamic observation shows that it runs |
| S4 packed or encrypted | UNSUPPORTED | an entropy of 6.1 for `.text` is in the ordinary range for compiled code; values near 8 point to compressed or encrypted data (ASSUMED rule of thumb); the notes show readable strings and a normal import list |
| S5 malware | UNSUPPORTED | capability is not intent; the same imports appear in benign network tools; a verdict needs behaviour, origin and context |

- Handling precautions (any two): do not execute the file; hash it and keep the original read-only; analyze only in a disposable guest with its adapters disconnected, no shared folders, no shared clipboard and no real credentials, never joined to the domain (plan section 3.2); do not open it with a viewer that parses it automatically; do not share its name or link in a form that can be run by accident.
- Score: 1 = at least four of the five labels correct, a reason for each INFERRED or UNSUPPORTED label, and two precautions. 0.5 = at least three labels correct, or four correct with fewer than two precautions. 0 = otherwise.
- Overclaims to reject: "it is packed" from one entropy value; "it connects to a server" stated as observed; "it is malware" stated as a conclusion.

#### E35 (multiple choice, 1 point)

- Not in the plan. Related plan seeds: W11-Q02, and plan section 3.2 ("Do not mount host Docker socket, production secrets, SSH agent or sensitive files into deliberately vulnerable containers").
- Correct option: B.
- Why: the socket is the control interface of a daemon that runs as root on the host. A process that can talk to it can ask the daemon to start a container with host paths mounted and extra privileges, which is close to root on the Docker host. The `:ro` flag makes the mount point read-only; it does not limit the requests that a connected client may send, and on Linux the read-only flag does not stop a connection to a socket (ASSUMED; check against the Docker documentation, which is not in the registry).
- Distractors: A is false for the reason above, and privileged mode is a second serious setting. C is false: privileged mode (all capabilities, host devices, no default seccomp or AppArmor confinement), `pid: host` (the host's process namespace) and the socket each extend the container's authority over the host. D is false: namespaces and cgroups are not a boundary against a process that holds the daemon's API or runs privileged, and containers share the host kernel.
- Notes (S): remove the socket mount, drop `privileged`, drop capabilities, run as a non-root user with a read-only root file system, prefer rootless or user-namespace-remapped runtimes; if an API must be exposed, expose a narrowed proxy and treat it as privileged.

#### E36 (classification, 1 / 0.5 / 0)

- Not in the plan. Related plan seeds: W06-Q07 and W12-Q04 (what a finding needs; how to report a plausible but unconfirmed issue).
- Reference classification:

| Sentence | Label | Reason and the observation that would settle it |
|---|---|---|
| S1 `lab.user1` can run `find` as root without a password | OBSERVED | EV-1 lists the rule for `lab.user1` on `lab-web01` at the time of the capture (what sudo's policy reports) |
| S2 `lab.user1` can obtain a root shell | INFERRED | follows from EV-1 and EV-2 (find can run another program) but is not shown; settle it with a harmless in-scope demonstration through the rule that only prints the effective user identity, captured with its time |
| S3 every account can do the same | UNSUPPORTED | one account was tested; sudo rules can be per user or per group; settle it by reading the policy and listing the rights of other in-scope accounts |
| S4 the rule was planted by an attacker | UNSUPPORTED | no evidence of origin or time; settle it with change history, file times and authentication logs, if attribution matters at all |
| S5 a retest after removal showed the path closed | UNSUPPORTED (also NOT RUN) | no retest is in the evidence; writing it would claim a test that was not done, which is an evidence-integrity failure |

- Score: 1 = at least four of the five labels correct and at least two settling observations named. 0.5 = at least three labels correct. 0 = fewer. S5 may be labelled NOT RUN or UNSUPPORTED.
- Overclaims to reject: S2 labelled OBSERVED; S5 labelled anything that treats the retest as done.

#### E37 (scenario, 1 / 0.5 / 0)

- Not in the plan. Related plan seeds: W07-Q04, W07-Q05 and day 33.
- Reference answer: (a) a process-creation source that carries the full command line: Windows Security event 4688, which needs the audit of process creation and, separately, the policy "Include command line in process creation events" (not configured by default, so the command-line field stays empty; `w07-ms-event-4688-cmdline`), or Sysmon Event ID 1 with Sysmon installed and configured (`w07-ms-sysmon`). The log must be collected and ingested, and the Sigma conversion must map `process_creation`, `Image` and `CommandLine` to the fields of that source. If the command-line field is empty, the second selection can never match. (b) A legitimate management or deployment agent, or an administrator's script, that starts `powershell.exe` with an encoded command matches both selections. (c) Misses (any one): the parameter in an abbreviated form such as `-e` or `-ec` (PowerShell accepts shortened parameter names; ASSUMED); `pwsh.exe` (PowerShell 7), because the `Image` condition names `powershell.exe`; a renamed or copied binary, for the same reason; a command line that ends with the parameter (the pattern needs a space after it).
- Essential: E1 source and configuration. E2 a valid benign match. E3 a valid miss with its reason.
- Notes: Sigma string matching is case-insensitive, so `-ENC` matches and is not a miss; the list under `CommandLine|contains` is an OR; reference `w07-sigma-rules-basics`.
- Score: 1 = E1, E2, E3. 0.5 = two of the three. 0 = fewer.

#### E38 (scenario, 1 / 0.5 / 0)

- Not in the plan. Related plan seeds: W04-Q02 (path traversal) and day 4 (parsing and exceptions).
- Reference answer: two distinct inputs that escape `BASE`: (i) a name with parent-directory components, such as `../private/notes.txt`: the join keeps them and the operating system resolves them to a file outside `BASE`; (ii) an absolute name, such as `/srv/lab/private/notes.txt`: `os.path.join` discards the earlier components when a later one is absolute. A third: a symbolic link inside `BASE` that points outside. Correct containment (proposed text, not run in the repository; its behaviour was recomputed by the author with a temporary directory on 2026-11-18):

```python
import os

BASE = os.path.realpath("/srv/lab/reports")

def read_report(name):
    path = os.path.realpath(os.path.join(BASE, name))
    if os.path.commonpath([BASE, path]) != BASE:
        raise PermissionError("outside the report directory")
    with open(path, "r", encoding="utf-8") as fh:
        return fh.read()
```

  The check resolves the final path (symbolic links included) and compares whole path components. A `startswith(BASE)` test is wrong: `/srv/lab/reports-old/x` begins with `/srv/lab/reports`. Deleting `../` from the string is also wrong (it can be bypassed and does not stop an absolute name). `pathlib.Path.resolve()` with `is_relative_to` (Python 3.9 or later) is equivalent.
- Further hardening (S, not required for 1): an allow-list of file-name patterns; refusing absolute names and separators; a race between the check and the open (open with a directory descriptor, or refuse links) ; running with a least-privilege account and a read-only view of the directory; failing closed and logging.
- Essential: E1 two distinct escaping inputs (a symbolic link counts as one of the two). E2 a correct containment check that does not use a prefix comparison.
- Overclaims to reject: "strip `../`"; "reject names that contain `..`" alone (an absolute name passes); `startswith(BASE)`.
- Score: 1 = E1 and E2. 0.5 = E1 or E2. 0 = neither. Reference: `https://docs.python.org/3/` (library `os.path` and `pathlib`; named in the W02 registry, ASSUMED here for these two modules).

#### E39 (scenario, 1 / 0.5 / 0)

- Not in the plan. Related plan seeds: W01-Q07 and day 4 ("parse example failed-auth logs"; "negative tests for corrupted log lines").
- Reference answer: (a) The function splits the first line into 14 fields and counts `lab.user2`; for the second line it takes field 8, which is the word `invalid` (the user name `admin` is field 10), and counts `invalid`; the third line has 7 fields, so `parts[8]` raises `IndexError`, and the exception leaves the function: nothing is returned, the counts so far (`lab.user2` 1, `invalid` 1) are lost, and the fourth line is never read. (b) Defects: D1 a fixed field position assumes one layout, so the `invalid user` form is silently attributed to the wrong name; D2 a truncated or malformed line aborts the whole run, and nothing records that a line was bad. (c) Fix (proposed text, not run in the repository; its result on the four lines was recomputed by the author: `{'lab.user2': 2, 'admin': 1}` with 1 line skipped):

```python
import re

FAILED = re.compile(
    r"^\w{3} +\d{1,2} \d{2}:\d{2}:\d{2} \S+ sshd\[\d+\]: "
    r"Failed password for (?:invalid user )?(?P<user>\S+) from (?P<ip>\S+) port (?P<port>\d+) ssh2$"
)

def count_failures(lines):
    counts, skipped = {}, 0
    for line in lines:
        m = FAILED.match(line.rstrip("\n")[:2000])
        if m is None:
            skipped += 1
            continue
        counts[m["user"]] = counts.get(m["user"], 0) + 1
    return counts, skipped
```

  Tests that would catch each defect: one fixture line for each known form (a normal line, an `invalid user` line, a user literally named `invalid`), asserting the user counted; a truncated line, an empty line and a very long line, asserting no exception and a skipped count of 1 each; a final run asserting that the total of counted and skipped lines equals the number of lines. The user name is attacker-controlled text: treat it as inert data (never as a format string, a path or a command).
- Essential: E1 the trace (an `IndexError` on the third line; the earlier counts are lost; `invalid` is counted for the second line). E2 two defects (positional parsing that breaks on the `invalid user` form; no handling of malformed lines). E3 a fix that parses by structure and neither crashes nor drops lines silently. E4 tests for each defect, including malformed input.
- Overclaims to reject: "wrap the loop in `try/except: pass`" (it hides the bad line and miscounts the second line); "use index 10" (it breaks the first form).
- Score: 1 = E1 to E4. 0.5 = two or three. 0 = fewer.

#### E40 (scenario, 1 / 0.5 / 0)

- Not in the plan. Related plan seeds: W11-Q05, W11-Q06 and the output-handling item of `data/competency-packs/*/pack.json` (`llm.output-schema-tools`).
- Reference answer: both fields are model output, and model output is untrusted data that a pasted email (a lower-trust source) can steer (indirect prompt injection). Failure 1: the application runs `sql` with its own database account, so a steered reply becomes an arbitrary query with that account's rights (data theft or change). Failure 2: it inserts `summary` into the page as HTML, so a steered reply becomes script in the operator's browser (stored XSS). Corrections: never execute model-written SQL; let the model choose from a closed set of actions, each mapped to a fixed parameterized query with typed and validated arguments; use a database account that can only do what that action needs, scoped to the signed-in user's tickets; encode `summary` for its output context (render it as text); validate the reply against a strict schema (types, lengths, an enumeration of actions), keeping in mind that a well-formed reply can still carry a harmful value; require approval for state-changing actions; log the exchange.
- Essential: E1 executing model-written SQL is an injection path with the application's privileges. E2 inserting the summary as HTML is a script-injection path. E3 corrections that do not depend on the model's behaviour (fixed actions, parameterized queries, least privilege, output encoding, schema validation).
- Supporting (one needed for 1): S1 approval for writes. S2 logging. S3 a test with an adversarial email fixture, assuming the model complies.
- Overclaims to reject: "tell the model to be careful or to sanitize its own output"; "a classifier for malicious emails is the control"; "valid JSON means safe".
- Score: 1 = E1, E2, E3 and one S. 0.5 = two of E1 to E3. 0 = fewer.

## 5. Part 3 key: the practical baseline

Every reference below is derived from the synthetic data written in the learner file, and from nothing else. Part 3 is 40 points in half-point steps. Task points by domain: Linux and virtualization 7 (Task 1); Networking 4 (Task 2) and 4 (Task 3); Python 3 (Task 2); Windows and Active Directory 4 (Task 4); Detection and DFIR 3 (Task 3) and 2 (Task 4); Web and API 5 (Task 5); Reverse engineering and reporting 2 (Task 5); Cloud and AI 6 (Task 6). The sum is 40. A task point is lost for a blank item, for an answer that contradicts the data, and for a listed overclaim.

### 5.1 Task 1: effective access (7 points, Linux and virtualization)

Reasoning: `/srv` is `drwxr-xr-x`, so everyone can search it. `/srv/lab` is `drwxr-x---` with owner `root` and group `analysts`: only `root` and members of `analysts` can search it (`lab.user1` and `svc.lab` are members; `lab.user2` is not). `/srv/lab/reports` is `drwxrwx---` with owner `svc.lab` and group `analysts`.

| Item | Reference answer | Points |
|---|---|---:|
| (a) `lab.user1`, `summary.txt` | read: yes. write: no. Not the owner (`svc.lab`); member of `analysts`; the group class of `-rw-r-----` is `r--` | 1 (0.5 read yes, 0.5 write no) |
| (b) `lab.user1`, `collect.sh` | read: yes. execute: yes. write: no. Not the owner; not in the group `svc.lab`; the other class of `-rwxr-xr-x` is `r-x` | 1 (0.5 read and execute, 0.5 write no) |
| (c) `lab.user2`, `status.json` | read: no. write: no. The file's mode would let anyone read and write, but `lab.user2` is not in `analysts` and cannot search `/srv/lab`, so the path cannot be resolved and the file's mode is never reached | 1.5 (0.5 both denied, 1.0 the reason) |
| (d) `svc.lab`, `notes.txt` | read: no. write: no. Not the owner (`lab.user1`); not in the group `lab.user1`; the other class of `-rw-------` is `---`. (It can still remove or rename the file, because it owns the directory: not asked.) | 1 (0.5 read no, 0.5 write no) |
| (e) who can remove `summary.txt` | `svc.lab` (owner of the directory, with write and search) and `lab.user1` (the group `analysts` has `rwx` on the directory); not `lab.user2`. Removing an entry needs write and search permission on the containing directory and search permission on the path; ownership of the file is not needed because the directory has no sticky bit | 1.5 (0.5 for each of the two users, 0.5 for the mechanism) |
| (f) one broader-than-needed entry | see the candidates below | 1 (0.5 for who can use the excess today, 0.5 for a valid entry with a change that keeps needed access) |

Candidates for (f); any other entry needs a consistent argument from the listing:

| Entry | Who can use the excess today | Smallest change that keeps the intended access |
|---|---|---|
| the directory `reports`, which has no sticky bit | `lab.user1` can delete or rename `summary.txt` and `collect.sh`, which it cannot write, and can then create a file of the same name; `svc.lab` owns the directory anyway. This is usable today | set the sticky bit on the directory (mode `1770`): then only the file's owner, the directory's owner or `root` can remove an entry; analysts keep reading and writing the files they may use |
| `status.json`, mode `rw-rw-rw-` | nobody gains anything today: `lab.user2` and any other account stop at `/srv/lab`, and `lab.user1` and `svc.lab` already write through the group and the owner. The excess is latent and becomes usable by every local account if `/srv/lab` or the group is opened | remove write for others (`rw-rw-r--`), or all bits of others (`rw-rw----`); owner and group keep their access |
| `collect.sh`, mode `rwxr-xr-x` | only members of `analysts` reach it today; the others' read and execute are not used | `rwxr-x---`, if the apparent purpose is use by analysts only; accept only with that argument |

Assumptions to state (credit, not required): the answers hold only for the stated ordinary semantics; `root`, a capability such as `CAP_DAC_OVERRIDE`, an ACL, a mount option or a mandatory profile would change them.

Overclaims to reject (the item's points are lost): "`lab.user2` can write `status.json` because its mode is `rw-rw-rw-`"; "only the owner can delete a file".

### 5.2 Task 2: subnets, scope and a scope check (7 points: Networking 4, Python 3)

| Item | Reference answer | Points |
|---|---|---:|
| (a) the four subnets | in the table below | 2 (0.5 for each row that is right in all fields; 0.25 for a row with three or four fields right) |
| (b) holders of `.20` and `.130` | `10.77.20.20` is in `10.77.20.0/26`; `10.77.20.130` is in `10.77.20.128/26`. They are in different subnets, so each host's stack treats the other as off-link and sends toward a gateway: they need a router (a layer-3 device) with an interface in each subnet, although both addresses lie in the same /24 | 1 (0.5 both subnets, 0.5 the reason) |
| (c) scope `10.77.20.0/26` | `10.77.20.20`: in scope. `10.77.20.70`: not. `10.77.20.130`: not | 1 for all three; 0.5 for two |
| (d) the flaw | a string prefix is not a network membership test: `10.77.20.` is the whole /24 and a /26 does not end at an octet boundary; the prefix also accepts any string that starts with it, including a host name, and the function does not check that the input is an address | 1 |
| (e) two inputs accepted and not authorized | any two of: `10.77.20.70`, `10.77.20.130`, `10.77.20.255`, `10.77.20.9.example.test` (a name, not an address), `10.77.20.` | 1 (0.5 each) |
| (f) a corrected function | uses the `ipaddress` module (or integer arithmetic with the mask): parse the input, return false when it is not an address, and test membership of the `10.77.20.0/26` network; fails closed | 1 (0.5 correct membership test, 0.5 invalid input refused) |

| Subnet | First usable | Last usable | Broadcast | Usable hosts |
|---|---|---|---|---:|
| `10.77.20.0/26` | `10.77.20.1` | `10.77.20.62` | `10.77.20.63` | 62 |
| `10.77.20.64/26` | `10.77.20.65` | `10.77.20.126` | `10.77.20.127` | 62 |
| `10.77.20.128/26` | `10.77.20.129` | `10.77.20.190` | `10.77.20.191` | 62 |
| `10.77.20.192/26` | `10.77.20.193` | `10.77.20.254` | `10.77.20.255` | 62 |

A reference function (proposed text, not run in the repository; its results for `10.77.20.20` true, `10.77.20.70` false, `10.77.20.130` false, `10.77.20.9.example.test` false and `::1` false were recomputed by the author):

```python
import ipaddress

SCOPE = ipaddress.ip_network("10.77.20.0/26")

def in_scope(addr):
    try:
        ip = ipaddress.ip_address(addr)
    except ValueError:
        return False
    return ip in SCOPE
```

Credit (S): an address of the other family is not a member and returns false; an IPv4-mapped IPv6 address returns false here and can be normalized with `ipv4_mapped` if it should be accepted; a name is not accepted, and a name that is resolved must have every resolved address tested. Note that `10.77.20.0` and `10.77.20.63` are inside the written range although they are the network and broadcast addresses: a scope line states a range, and which of its addresses are hosts is a separate question.

### 5.3 Task 3: reading a packet summary (7 points: Networking 4, Detection and DFIR 3)

| Item | Reference answer | Points |
|---|---|---:|
| (a) port 8000 | open. Packets 1 to 3 are the handshake (SYN, SYN-ACK, ACK); packet 4 is an HTTP request and packet 6 an HTTP 200 reply, so an application answered. A connect scan reports `open` | 1 (0.5 state, 0.5 evidence) |
| (a) port 8001 | closed. Packet 8 is a RST,ACK from the target itself in reply to the SYN of packet 7: the host is reachable and nothing accepts on that port. A connect scan reports `closed` | 1 |
| (a) port 8002 | filtered. The SYN of packet 9 (at 2.305 s) is retransmitted in packets 10 and 11 (at +1.001 s and +3.005 s from the first, a doubling back-off) and nothing answers: no reply, no reset, no ICMP error. This fits a silent drop on the path or at the host. A connect scan reports `filtered`. It is not `closed` (that would be a reset) and it does not show a listener or its absence, or where the drop is | 1 |
| (a) port 8003 and packet 13 | filtered. Packet 13 comes from `10.77.10.1`, the RED port of the lab router, not from the target: the router answered the probe with an ICMP destination-unreachable error, code "host administratively prohibited", so the policy at the router refused the probe. The target did not answer, and the capture does not show that the target ever saw the SYN. A connect scan reports `filtered` | 1 (0.5 state, 0.5 the router as the sender) |
| (b) two unsupported conclusions | any two of: a service listens on 8002 or 8003 (not shown); the target is down or unreachable (it answered on 8000 and 8001); the firewall of the target dropped the packets of 8002 (the capture does not show where the drop is); 8002 is closed (a closed port answers with a reset); the service on 8000 is healthy or secure (one request and one reply); anything about the target's operating system (no such field is shown); that the HTTP content is benign | 2 (1 each) |
| (c) a data source to settle 8002 | any one of: the router's or firewall's log or counters for the decision on that flow; a capture on the target's interface or on the DMZ port of the router; the target's firewall counters or log and its listener inventory; a probe from another vantage point in scope | 1 |

Note for scoring: the sequence and acknowledgment numbers of the summary are relative numbers, so the initial sequence numbers cannot be read from it (credit if a learner says so; not required).

### 5.4 Task 4: reading a Windows security event (6 points: Windows and Active Directory 4, Detection and DFIR 2)

| Item | Reference answer | Points |
|---|---|---:|
| (a) the account | `lab.user2` in the domain `CORP` (the domain name as the sample writes it) | 0.5 |
| (a) the logon type | 3: a network logon (for example access to a share or a service over the network) | 0.5 |
| (a) the authentication | NTLM through the `NtLmSsp` logon process, not Kerberos | 0.5 |
| (a) the origin | the workstation `WS01`, address `10.77.30.20`, source port 51944 | 0.5 |
| (a) status and sub status | status `0xC000006D` is the generic logon failure (0.5); sub status `0xC000006A` means the password was wrong (0.5) for a user name that is valid, so the account exists (0.5). A value read from memory is ASSUMED knowledge (Microsoft documentation of event 4625; not in the registry) | 1.5 |
| (a) the empty subject | the Subject fields (`S-1-0-0`, `-`) name the account that asked the local system to log on; for a failed network logon there is none, so they are empty | 0.5 |
| (b) not evidence of an attack | one failed network logon has benign causes (a typing error, a stale saved credential, a service or scheduled task with an old password, a mapped drive). At least two further data points, each with where it lives: the count and the rate of failures by source and account over time (the Security log of the machines that log it, collected in the SIEM); a success for the same account from the same source after failures (event 4624); failures of other accounts from the same source; an account lockout (event 4740, where a lockout threshold is set); the credential-validation event on the domain controller for the same attempt (event 4776, with the same error code; ASSUMED); on the source host, the process that made the connection (endpoint telemetry) | 1 |
| (c) what must be in place | the audit policy for logon events with failure auditing on the machine (Advanced Audit Policy Configuration, Logon/Logoff, Audit Logon; `w03-ms-advanced-audit-policy`, SEARCH_CHECKED), a Security log large enough to keep it, and collection to the SIEM if it is to be searched there. The absence of an event shows only that none was recorded and collected: it does not show that no attempt happened | 1 |

Overclaims to reject (the item's points are lost): "this event proves a password-guessing attack"; "Kerberos was used"; "the account does not exist" (the sub status says it does).

### 5.5 Task 5: classifying three web requests (7 points: Web and API 5, Reverse engineering and reporting 2)

| Request | Class (0.5) | Outcome (0.5) | Evidence to cite (0.5) |
|---|---|---|---|
| R1 `GET /files?name=..%2f..%2f..%2fetc%2fhostname` | path traversal (reading a file outside the intended directory; the `..` sequences are URL-encoded) | succeeded | status 200 and a 10-byte body, `lab-web01`, which is a host name and not a file of the application; the body is consistent with the content of the host's hostname file (INFERRED until compared with that file in the lab) |
| R2 `GET /search?q=%27%20OR%20%271%27%3D%271` | SQL injection candidate (a quote and a boolean fragment in a search parameter) | inconclusive | status 500 and a body that starts with a database syntax error: the input reached the SQL parser and altered the statement's syntax; no data, no change of behaviour that only a changed query explains, and no impact is shown |
| R3 `GET /fetch?url=http%3A%2F%2F10.77.30.10%2F` | server-side request forgery (the server is asked to fetch an internal address, here the CORP domain controller) | blocked | status 403 and the body `destination not permitted`: the application refused this value. It does not show that the control is complete (other encodings, redirects and names are not tested). The lab policy denies and logs DMZ to CORP connections, so the router log would show whether any connection was attempted |

Scoring of (a): 4.5 points, 1.5 per request. For R2 an answer of "succeeded" or "blocked" is wrong; "inconclusive" with the evidence is right. For R3 an answer of "the application is not vulnerable to request forgery" is an overclaim and loses the evidence point.

| Item | Reference answer | Points |
|---|---|---:|
| (b) a further observation for R2 | any one: a pair of controlled requests in the authorized lab, one with a benign value and one with a modified value, whose responses differ in a way only a changed query explains; the source of the query construction; the database's own log of the executed statement | 0.5 |
| (c) a report sentence | an example: "On 2026-11-16 at 16:20:05Z a request for `/files?name=..%2f..%2f..%2fetc%2fhostname` from 10.77.10.10 to the lab application on 10.77.20.20 (authorized lab target) returned HTTP 200 with a 10-byte body, `lab-web01` (OBSERVED). The body is consistent with the content of a file outside the application's directory (INFERRED); which other files can be read, and the impact, were not tested." Full points need the observed facts with target, time and scope context (1) and the inference labelled with its limit (1). An answer that claims arbitrary file read, server takeover or an impact, or states the inference as observed, scores 0 | 2 |

### 5.6 Task 6: trust boundaries of a described agent (6 points, Cloud and AI)

A reference diagram (plain text; the learner's own layout may differ). Trust labels: T trusted, I internal, U untrusted.

```
customer --(1 ticket text: U)--> [web form] --(2: U)--> [agent runtime]
customer uploads --(3: U)--> [knowledge index] <--(4: I)-- company documents
[agent runtime] --(5 query)--> [knowledge index] --(6 chunks: U or I)--> [agent runtime]
[agent runtime] --(7 prompt: mixed)--> [language model]
[language model] --(8 text and tool-call requests: U)--> [agent runtime]
[agent runtime] --(9 tool-call request)--> [tool executor] --(10 service credential)--> [ticket store]
[tool executor] --(11 message, any address)--> external recipient
[agent runtime] --(12 reply: U)--> [operator dashboard] --> operator

Boundaries: B1 customer | application; B2 uploaded content | index (ingestion);
B3 index content | model context; B4 model | runtime (model output proposes, never decides);
B5 runtime | tool executor and credential (authority); B6 application | external recipients (egress);
B7 application | operator's browser (output rendering)
```

| Item | Reference answer | Points |
|---|---|---:|
| (a) components and flows | at least seven of the eight components (customer, web form, agent runtime, knowledge index with company documents and uploads, language model, tool executor with its three tools, service credential and ticket store, operator dashboard) and the flows between them | 1 |
| (b) boundaries and trust labels | at least three boundaries, including model output to tool executor (B4/B5) and uploaded content to the model's context (B2/B3), each flow labelled with a trust level; the content of tickets, uploads, retrieved chunks and model output is untrusted | 1 |
| (c) what crosses and who decides | for each crossing, what crosses (0.5 in all). For the tool-call crossing: a mechanism outside the model decides, from the authenticated user's authority and a list of the tools and arguments allowed for this agent, not from the model's text or from any document (1.0); the model's own output does not authorize itself (0.5) | 2 |
| (d) the three riskiest crossings | any three with reasons from: model output to tool executor with a credential that reaches every customer's tickets (excessive agency, a confused deputy); uploaded documents into the model's context (an indirect prompt-injection entry that is not the customer's own authority); `send_email` to any address (an exfiltration channel); the service credential that spans every customer (blast radius); the reply rendered for the operator (script injection or deception) | 2 (about 0.7 for each valid crossing with its reason; round to the half point) |

Overclaims to reject (the item's points are lost): "the model decides which tools are allowed"; "the system prompt is the boundary"; "trusted because it comes from our knowledge base" for the uploaded files.

### 5.7 Part 3 integrity

A claim in an answer that a command was run, or a result observed, is not credit: nothing may be run in Part 3. An answer that states a sample's content differently from the learner file is read against the file.

## 6. Interpretation by domain

### 6.1 The three tracks

The rule is that of plan section 1.3 and `docs/curriculum/v2/STUDENT_GUIDE.md` (section 9); this table restates it with the arithmetic.

| Share of the domain's question points | Practical pass in the domain | Track |
|---|---|---|
| below 70% | any | Guided: every guided lesson of the domain's weeks |
| 70% to below 90% | any | Independent labs |
| 90% or more | yes | Expert extension |
| 90% or more | no | Independent labs |

The practical pass is PROPOSED: at least 75% of the domain's practical points, rounded up to a half point (Linux and virtualization 5.5 of 7; Networking 6 of 8; Python 2.5 of 3; Windows and Active Directory 3 of 4; Web and API 4 of 5; Detection and DFIR 4 of 5; Reverse engineering and reporting 1.5 of 2; Cloud and AI 4.5 of 6; whole of Part 3 30 of 40).

### 6.2 What a low result in each domain suggests

Day numbers are those of the plan. A signature is a pattern among the missed questions; it is a prompt for reading the answers, not a diagnosis.

| Domain (questions; practical points) | Signature of a miss | Likely gap | First action on the Guided track | A high result does not show |
|---|---|---|---|---|
| Linux and virtualization (E01 to E04, E21, E29; 7) | E01 right, E02 or E04 wrong | knows the mode bits, not the privilege model (effective identity, DAC against MAC) | day 1 block A, day 3 (sudo, AppArmor) | the ability to troubleshoot an unfamiliar host: the Friday of week 1 tests it |
| | E03 wrong | confuses boot configuration with present state | day 3 (systemd status and logging) | |
| | E21 or E29 wrong with E01 to E04 right | recites facts but does not weigh evidence (scanner, listener, firewall; a missing socket as an investigation, not a fix) | day 1 block B (host diagnostics), day 8 | |
| | Task 1 low with the questions right | misses the directory chain and deletion by directory permission | day 2 (permissions and ownership) | |
| Networking (E05 to E09, E22; 8) | E05 or E06 wrong | handshake or addressing basics | day 6 | the ability to read an unfamiliar capture: the Friday of week 2 tests it |
| | E07 or E21 wrong | treats a scan state as a statement about the service | day 8 (port states) | |
| | E08 wrong | misjudges what TLS hides | day 7 (DNS, HTTP, TLS metadata) | |
| | E09 or E22 wrong | a single hypothesis; trusts a route table or a tool setting | day 9 (statefulness, firewalls, routing), day 1 block B | |
| | Task 2 (a) to (c) or Task 3 low | subnet arithmetic, or reading states from packets | days 6, 8 and 10 | |
| Python (E10, E11, E38, E39; 3) | E10 wrong | does not see data becoming command syntax | day 4 (subprocess safety, shell quoting) | |
| | E11 or E39 wrong | error and malformed-input handling is missing | day 4 (exceptions, negative tests), day 9 (sockets and exceptions) | the ability to write and test a utility: the week 1 and week 2 gates test it |
| | E38 or Task 2 (d) to (f) wrong | string tests where a parser or a resolved path is needed | day 4, day 18 (traversal), day 9 | |
| Windows and Active Directory (E12 to E15, E23; 4) | E12 or E13 wrong | principals, policy scope and processing | days 11 and 12 | the ability to assess an unseen directory configuration: the Friday of week 3 tests it |
| | E14 wrong | ticket roles in Kerberos | day 13 | |
| | E15 or E23 wrong | nested groups, inheritance, and service identity | days 13 and 46 (effective access), day 28 | |
| | Task 4 low | reading event fields and the audit prerequisite | day 14 (audit policy, event correlation) | |
| Web and API (E16 to E18, E24; 5) | E16 wrong | instruction and data are not separated in the mental model | day 17 | the ability to find a defect on an unseen application: the Friday of week 4 tests it |
| | E17 or E24 wrong | authentication is treated as authorization | day 23 (IDOR, BOLA) | |
| | E18 wrong | execution contexts of XSS | day 21 | |
| | Task 5 low | confuses an attempt with a result, or a symptom with proof | days 16 to 19, day 20 (classification) | |
| Detection and DFIR (E19, E20, E25, E31, E32, E37; 5) | E19, E20 or E37 wrong | detection tests, telemetry prerequisites and rule limits | days 31 to 33 | the ability to detect on an unseen trace: the Friday of week 7 tests it |
| | E25, E31 or E32 wrong | time, integrity and volatility in evidence handling | day 36 (acquisition, hashes, time zones), day 39 (Linux incident response) | |
| | Task 3 (b), (c) and Task 4 (b), (c) low | does not separate observed from inferred, or does not name the coverage gap | day 34, day 36 | |
| Reverse engineering and reporting (E26, E30, E33, E34, E36; 2) | E26, E33 or E34 wrong | crash, vulnerability and exploitability run together; static properties read as behaviour | days 41 and 44 | any reverse-engineering practical ability: Part 3 has no such item |
| | E30, E36 or Task 5 (c) wrong | claims run ahead of evidence | day 26 (false positives), the reports of days 30 and 60 | |
| Cloud and AI (E27, E28, E35, E40; 6) | E27 wrong | trust against permission policy | day 51 | the ability to run a policy test on an agent: weeks 11 and 12 test it |
| | E35 wrong | container isolation overestimated | day 52 | |
| | E28, E40 or Task 6 low | authority placed in the model or in its text | days 54, 55 and 56 | |

### 6.3 Reading share and practical together

| Pattern | Reading | Action |
|---|---|---|
| share below 70%, practical pass | the paper tasks were done with time and care while the recall is thin, or the guesses were lucky | Guided still applies (the rule uses the share); read the missed questions; the first independent lab is the check |
| share 70% to 89%, any practical | the ordinary case | Independent labs |
| share 90% or more, practical not passed | knowledge that did not transfer to a task on data | Independent labs, never Expert extension; the first independent lab acts as the test |
| share 90% or more, practical pass | both measures agree | Expert extension for the domain; it stays optional |
| a domain marked provisional (Part 2 or Part 3 not done, or many `?`) | thin evidence | the track stands; re-place at the first Friday assessment of that domain's weeks |
| total of Part 3 below 30 of 40 | a general weakness in applying knowledge to data on paper | note it; it changes no hour and no gate |

### 6.4 Retest

The midpoint (day 30) and the final (day 60) fill the same sheet (plan section 1.3). A track moves on evidence: `docs/curriculum/v2/STUDENT_GUIDE.md` (section 9) proposes that two consecutive Fridays with the matching category below its diagnostic target move a domain back to Guided until a Friday meets it. A retest on this same form measures memory as well as skill (open point O-D-02).

## 7. Departures from the plan

| What | Why |
|---|---|
| The plan's 30 questions are kept with their ids, order and meaning; 14 of E01 to E20 become multiple choice (E01, E03 to E08, E12, E14, E16 to E20) | plan section 7.1 allows "carefully designed MCQs with plausible distractors" for E01 to E20; the task asks for at least 12 |
| E21 to E30 and some of E01 to E20 carry synthetic samples, and a few add a sub-question (E21 asks for the reported state of two ports, E24 for a negative test, E25(b) for a conversion, E27 for a correction, E29 for a described diagnostic) | plan section 7.1 asks for applied reasoning that cites packet fields, event sources or software properties; section 8 asks that the key reflect actual fixture data. The extra sub-questions test the same concept and do not contradict a plan key |
| E22: "A SYN reaches a host" is read as "a SYN is sent toward a host", with the observer on the sender | the plan's key lists target down, packet loss, no route and a capture-point blind spot as causes. They cannot explain silence if the SYN is known to have reached a live host. The ambiguity is the plan's |
| E29's sample is the error text of the plan's own observation (the socket path), with invented values | the question is the plan's; the sample states no host fact |
| Part 2 (E31 to E40): ten questions that the plan does not contain | the plan sets 30 as a minimum (section 0.3); the choice is justified in section 4.1. They do not alter the plan's 30 or their key |
| Eight scoring domains instead of the plan's seven; the Student Guide's counts (6, 6, 2, 5, 4, 3, 2, 2) refer to the 30 and are extended to 6, 6, 4, 5, 4, 6, 5, 4 by Part 2 | the plan names Linux, networking, Python, Windows and AD, web, detection, cloud and AI and does not say where E26 and E30 belong; the Student Guide groups them as an eighth domain, and this diagnostic follows it so that the documents agree. The Student Guide's table should be read together with this file (open point O-D-03) |
| Half points | the plan scores no question; `docs/curriculum/v2/STUDENT_GUIDE.md` proposes 0, 0.5 and 1. PROPOSED, like the equivalent change of the Friday exam in `docs/curriculum/v2/INSTRUCTOR_GUIDE.md` |
| "Practical pass" at 75% of a domain's practical points | the plan gives no baseline pass; 75% matches the practical minimum of 30 of 40 (plan section 9.1). PROPOSED |
| Part 3 is scored out of 40 and each point belongs to a domain | the plan asks for a non-invasive practical baseline and the Student Guide for a practical pass per domain; neither gives a point split. PROPOSED |
| Time estimate of about 225 minutes against the 120 minutes of block A of day 1 | the plan allots the diagnostic no time of its own and puts a lesson in the same block. The estimate is not measured; hours are not extended (plan section 1.3) |
| Day 1 is 2026-11-16 in the plan and the courseware is written on 2026-11-18 | the plan lets the learner shift dates and keep the sequence; nothing in the diagnostic depends on the date, and every sample date is invented |
| E34 and E36 use the labels OBSERVED, INFERRED and UNSUPPORTED | UNSUPPORTED is a classification of a statement against given evidence, not a status label of a document; it corresponds to the plan's UNVERIFIED (plan section 0.4) in the sense that no evidence is given. NOT RUN is accepted for E36 S5 |
| No competency id for E13, E26, E27, E30, E33, E35 and E36 | no competency of the packs fits; ADR-0023 records the gap for reverse engineering, containers and cloud IAM and proposes a ninth pack |

## 8. Sources, checks, commands and open points

### 8.1 Sources

Registry ids are the entries of the week registries as read on 2026-11-18 (`SEARCH_CHECKED`: a search result was seen; the page was not opened). Addresses are those of plan section 4 and are ASSUMED here; the generated `docs/curriculum/v2/RESOURCE_REGISTRY.md` wins.

| Used for | Source |
|---|---|
| E07, E21, Task 3 (port states) | `https://nmap.org/book/man-port-scanning-basics.html` (`w02-nmap-port-scanning-basics`) |
| E08, E05 | RFC 8446 (`w02-rfc8446`); RFC 9293 is named in its evidence text; otherwise ASSUMED standards knowledge |
| E10 | `https://docs.python.org/3/library/subprocess.html` (`w01-python-subprocess`) |
| E13 | `https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/manage/group-policy/group-policy-overview` (`w03-ms-group-policy-overview`) |
| E12, E14, E15 | the Microsoft AD DS documentation named in plan section 4 (ASSUMED) |
| E16, E24, Task 5 | `https://wstg.owasp.org/` (`w04-owasp-wstg`, `w04-owasp-wstg-sql-injection`) |
| E20 | `https://learn.microsoft.com/en-us/windows/security/operating-system-security/sysmon/sysmon-events` (`w07-ms-sysmon-events`, `w03-ms-sysmon-events`) |
| E37 | `https://sigmahq.io/docs/basics/rules.html` (`w07-sigma-rules-basics`); `https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/manage/component-updates/command-line-process-auditing` (`w07-ms-event-4688-cmdline`); `https://learn.microsoft.com/en-us/sysinternals/downloads/sysmon` (`w07-ms-sysmon`) |
| Task 4 | `https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/plan/security-best-practices/advanced-audit-policy-configuration` (`w03-ms-advanced-audit-policy`); `https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/plan/appendix-l--events-to-monitor` (`w07-ms-appendix-l-events`) |
| E29 | `https://libvirt.org/daemons.html` (`w01-libvirt-daemons`) |
| E32 | RFC 3227, `https://datatracker.ietf.org/doc/html/rfc3227` (`w08-rfc3227`); `https://csrc.nist.gov/pubs/sp/800/61/r3/final` (`w08-nist-sp800-61r3`) |
| E27 | `https://docs.aws.amazon.com/wellarchitected/latest/security-pillar/` (`w11-aws-security-pillar`, ASSUMED) |
| E28, E40, Task 6 | `https://genai.owasp.org/resource/owasp-genai-llm-top-10-2026/` (`w11-owasp-genai-llm-top10-2026`, ASSUMED) |
| Task 3 | `https://www.wireshark.org/docs/wsug_html/` (`w02-wireshark-users-guide`) |
| Standards knowledge not in the registry (ASSUMED) | RFC 1918 and RFC 3021 (E06); RFC 5737 (documentation addresses of the samples); TCP initial retransmission timeout (E22); GPO background refresh intervals (E13); Sysmon `Initiated` field (E20); Docker socket and `:ro` behaviour (E35); AWS trust-policy semantics (E27); PowerShell parameter abbreviations (E37); Event 4625 status values and event 4776 (Task 4); typical entropy of compiled code (E34) |

### 8.2 Checks made by the author

| Check | Result |
|---|---|
| Every question of the learner file has a tag line; ids E01 to E40 are present once and in order; 15 multiple choice with options A to D | done by parsing the learner file on 2026-11-18 |
| Every difficulty lies in the band of its competency (E01 to E40 and the six tasks) | done by comparison with the `difficulty_band` of each competency of the packs |
| The learner file holds no non-ASCII character, no control character and no markup | done by scanning the file |
| Subnet table, membership tests, `os.path.join` and containment behaviour (including a link and a `-old` sibling directory), the trace and the fix of the log parser, the Central Time conversion, the retransmission gaps of E22 and Task 3, the byte counts of the Task 5 bodies | recomputed with the Python 3.13 standard library; a temporary directory was used for the file checks; the script is not committed |
| Product behaviour (Sysmon, Windows events, Docker, AWS, PowerShell, nftables, Nmap's mapping of ICMP codes to `filtered`) | not run; ASSUMED knowledge, with the registry entries named where they exist |

### 8.3 Commands named in this key

Proposed text for the owner's review. Nothing here is run by the key, and nothing here says that running it is authorized (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, class B).

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| 1 | `systemctl list-unit-files 'libvirtd*' 'virtqemud*' 'virtproxyd*' --no-pager` (plan section 2.2, Stage 00) | the G9 host's systemd unit-file list | answers E29 on the real host: which libvirt units are installed and whether each is enabled | a table of matching unit files with their state, or a line saying that no unit files were listed | none: it reads the list of installed unit files | none beyond an ordinary user | the G9 host only; one read-only listing; the owner approves each Stage 00 row individually | none needed (read-only) |

### 8.4 Open points

| Id | Point | Owner of the decision |
|---|---|---|
| O-D-01 | The estimate of 225 minutes is not measured. Where Parts 1 to 3 sit in day 1 is for the day 1 guide; the plan gives the diagnostic no time of its own | the day 1 author, the owner |
| O-D-02 | A retest on this form measures memory as well as skill, because the questions are known. PROPOSED: write a parallel form (same tags and difficulty, other samples and numbers) for the midpoint and a third for the final; this task wrote one form only | the owner |
| O-D-03 | `docs/curriculum/v2/STUDENT_GUIDE.md` (section 9) counts the domains over the 30 questions only; the counts of this diagnostic are 6, 6, 4, 5, 4, 6, 5, 4 | the Student Guide's author |
| O-D-04 | The fixture set FX-C01 of `docs/curriculum/v2/FIXTURES/README.md` is not registered: the samples are embedded in the learner file and the map is this file | the owner, the assessments task |
| O-D-05 | A ninth competency pack for reverse engineering, containers and cloud IAM (ADR-0023) would let E26, E27, E33 and E35 carry competency ids | the owner |
| O-D-06 | Statements marked ASSUMED in this key (section 8.1) should be checked against the primary documentation before the retests | the owner, with the registry |
| O-D-07 | Whether the window records a placement per domain and a hash of the answers is not established; the day 1 record holds them meanwhile | the owner |
