# Instructor key — G9 W01 labs

Instructor only. Not part of the learner's view.

- Status: PROPOSED. Nothing in this file was observed. Every "reference observation" is a prediction from the mechanism and from the fixtures; it becomes OBSERVED only when the owner's approved run is recorded with provenance. Where an outcome depends on the release of a tool, the label ASSUMED says so and the learner's own observation decides.
- Opening rule (INSTRUCTOR_GUIDE section 6): this file is opened only after the attempt it concerns is submitted and its evidence bundle is hashed; the opening is logged on the scorecard. The learner files `LABS/GUIDED_LAB.md`, `LABS/INDEPENDENT_LAB.md`, `LABS/FRIDAY_PRACTICAL.md` and the fixtures hold no text from this file.
- Fixtures are cited by path under `LABS/fixtures/` and by line number, never quoted at length.
- Marking vocabulary: the bands and sub-criteria P1 to P4, D1 to D4 and E1 to E3 are those of `docs/curriculum/v2/ASSESSMENTS/GRADING_RUBRICS.md` sections 3 and 5 to 7. R items are the observable required items; V is the verification item (a second independent observation).

## Guided lab

### Reference observations by step

| Step | What a correct record shows | What a wrong reading looks like |
|---|---|---|
| 1 | scope lines with target ids, classes, windows, sources; a time record in UTC and local with the offset; the RoE draft; written before the first approved command | a scope written afterwards; a line without a target id; the host listed as a target |
| 2 | the arithmetic of the synthetic host (below) and then the real figures with their rows; the P-06 verdict per stack; the statement that vCPUs are a ceiling per guest and thin disks are not consumption | counting 16 vCPUs as 16 cores; treating configured disk as used or as free; declaring the plan's full allocation fits without the rule |
| 3 | per synthetic host the leaf, the deciding line and the next read-only row (below); for the real host a recorded tree path or the honest statement that it is unfinished; the exact failed command written down; no installation proposed as a next row | a leaf chosen from the error text alone; "libvirt is broken"; `virsh -c` suggested as the first move without the class C caveat |
| 4 | one NIC per guest with the MAC plan, resources equal to the allocation (the target 2 vCPU with 3 GiB, the attacker 4 vCPU with 6 GiB), two taps on the host | a guest with a second NIC accepted as "also fine"; the allocation quoted from the plan instead of read from the domain |
| 5 | per guest one lab address, a connected route, no default route, no IPv6 route, the documentation address unreachable by the kernel's own message; per test T8, T6, T7, T5 a result with the deciding line; a capture counted only with its stimulus inside the window | "isolated" because the network is named `g9-lab-dmz`; an empty capture offered without the probe record; a missing route presented as an enforced denial |
| 6 | baseline listing: root owns `/srv/g9toy`; the reports directory and files belong to the service account with the narrow group; `is-enabled` is disabled and `is-active` is active; `User=svclab`; the service listens on loopback 8080 only; the process column of the socket listing depends on privilege and the learner reports what was observed (ASSUMED: typically blank for another account's socket without administrative rights); labuser1 reads, labuser2 is refused by the directory | explaining labuser2's refusal by the file mode; enabled and active treated as one fact |
| 7 | the file-only change does not expose the file (the directory's search permission refuses first); the directory change does; the status-change time of each widened path records the moment of the change | predicting exposure after step 7a; confusing ctime with mtime |
| 8 | read, append, create and delete all succeed for labuser2; the service then returns the report with the extra marker line (164 bytes where the fixture has 138); delete succeeded without any right to the canary because the directory allowed it; the journal holds the service's own request lines and nothing about the file operations; the proposed audit watch is labelled PROPOSED and not applied | claiming the journal shows who edited the file; calling the delete "impossible without file write" |
| 9 | the directory returns to the narrow group with the setgid bit; the report is reinstalled from the medium and its hash equals the fixture's; the over-tight state yields a refusal for the service identity and for labuser1; the final listing equals the baseline; the four negative tests fail in the same form as the step 8 tests; positives succeed | a fix that is only `chmod 700`; a retest from a stale session; "retest passed" with no output file |
| 9c | the pass line and the block line of the router for the two ports, TTL one lower on the DMZ tap, no listener left on a lab address, T1 negative check and T2 repeated | the service left bound to the DMZ address after the window |
| 10 | hashes written before the revert; marker hash equal before and after; T8 and T5 to T7 repeated; a restore time that was measured | a restore claimed with no marker check; evidence copied after the revert |

Extensions, reference observations (each is a prediction until observed):

| Extension | What a correct record shows |
|---|---|
| E1 delegation | the syntax check passes; the listing of the rule shows the argument pattern with a trailing wildcard; the intended single-file use works; an argument string that adds a second path is matched by the same pattern, because the arguments are compared as one string and a wildcard in arguments also matches spaces and slashes (`w01-sudoers-manual`, SEARCH_CHECKED); the root-only synthetic file is read. The delegation was never needed because group membership already grants the read; the correct fix is to remove it, with exact arguments as the fallback. A refusal after removal is recorded verbatim; its wording is release-dependent |
| E2 sandbox | the exposure value falls (the number varies with the systemd version and is only compared with the earlier run); `WHOAMI` shows the no-new-privileges flag set and an empty effective capability mask; `COUNT` still works because the state directory is writable by `ReadWritePaths=`; `SELFTEST` is refused with the read-only file system error although the account owns the spool directory: the refusal is a mount-namespace effect, not a mode (`w01-systemd-exec`); the exposure value never proves the service safe or vulnerable. Root variant: `WHOAMI` shows uid 0; the owner of the state directory may change (ASSUMED systemd behaviour); the capability mask is empty only if the bounding set drop-in is also applied |
| E3 failure | the status shows an exit code in the 226/NAMESPACE family; the journal names the path that does not exist; the repair is the path, not a mode |
| E4 | the module flag is `Y` or `N` as observed; an ordinary shell is `unconfined`; the profile count and modes come from the administrator listing; AppArmor is not claimed to confine the toy service unless a profile for it is listed (none is created in this lab) |
| E5 | for an ordinary account the effective capability mask is zero; the real, effective, saved and filesystem IDs are equal; supplementary groups are those of the session |

### Reference readings of the synthetic samples

Host budget (`d01/host_budget_sample.txt`): 12 threads on 6 cores. Stack A configured 6 vCPU, stack B 8 vCPU, the plan's full stack 16 vCPU: A and B fit the thread count as ceilings; the full stack exceeds it, and 2 vCPU per physical core is the ratio to state. Memory available 24 GiB: stack A needs 9 plus 8 = 17 (fits), stack B 11 plus 8 = 19 (fits), the full stack 31 plus 8 = 39 (does not fit). Free disk 273 GiB: stack B's 140 GiB configured is below it, the plan's 390 GiB is above it, the plan's 600 GB target is not met; thin disks grow with writes and baselines multiply them, so no consumed figure follows from configured ones. Accept the rule P-06 labelled PROPOSED heuristic.

Stage 00 samples (HOST_READINESS decision tree):

| File | Leaf | Deciding evidence | Next read-only row | Not established |
|---|---|---|---|---|
| `host_diag_sample_01.txt` | L10 (client path or URI differs from the daemon's) | the modular socket units exist and listen under the per-driver path; the legacy path is absent from the directory listing; no environment override | F-19, F-20, F-21 | whether the client chooses the modular path by itself on this version; at the right path access would need the socket's group, and the id line shows the account is not in it (L11 would follow) |
| `host_diag_sample_02.txt` | L4 (daemon packages absent) | no unit file for any of the three families; only the client library and tools in the package query; no QEMU package | F-14 and F-13 are already shown; the owner decides outside the file | that no emulator exists elsewhere; `/dev/kvm` is present and irrelevant to the missing socket |
| `host_diag_sample_03.txt` | L8 (units present, disabled or inactive) | the monolithic units exist, both service and socket are disabled and inactive, the runtime directory is absent | none needed: F-10 is shown; starting is a class C decision | whether the units were ever started on this boot |
| `host_diag_sample_04.txt` | L11 (socket exists, account lacks access) | the socket exists with group `libvirt`, mode 0770; the id line has no such group; the error text is `Permission denied`, not `No such file or directory` | F-17 and F-18 are shown; the privilege model is the owner's decision | whether group membership would be acceptable: it is root-equivalent in effect (ASSUMED) |

Key teaching point across the four: the same client, three different error strings and four different causes. The learner who answers from the plan's error string alone fails sample 04.

Isolation evidence sets:

| Set | T8 | T6 | T7 | T5 | Overall |
|---|---|---|---|---|---|
| `isolation_evidence_set_01.txt` | PASS: one NIC per guest, live equals persistent, two taps for two guests, only the lab bridges and the pre-existing interfaces | PASS: both guests answer with the kernel's unreachable message and have no default route; the capture shows nothing for the lab range on the uplink and the tunnel | PASS: no IPv6 route in either guest, the sysctl is 1, the host bridges carry no address and have the sysctl set, the host rule drops input and forward for the bridges | PASS for the base probe: the probe time lies inside the capture window and the guest reports the unreachable message. Limit to state: the packet never left the guest, so the capture says nothing about the host's answer on the bridge; the mistaken-route variant and the on-link host probe of the test are NOT RUN | PASS for the base case with named limits; a claim of full isolation from the household LAN is an overclaim |
| `isolation_evidence_set_02.txt` | FAIL: the target has a second NIC on the default network in the live definition and not in the persistent one (a live attach), a second address and a default route in the guest, the default network is active and set to autostart | FAIL for the target (a default route through the extra NIC means a route exists) and a deviation for the attacker (a default route toward the router port, where the design says none); whether packets would leave is not shown (no router export) | INCONCLUSIVE: link-local only in the guests, but the target has IPv6 enabled, the host bridge holds a link-local address with the sysctl at 0, and no firewall evidence or neighbour test is given; no global address exists, so FAIL is also wrong | INCONCLUSIVE: no probe record, no stimulus, a capture of 15 minutes with zero packets proves nothing; the extra NIC gives the target a path the probe must also test | STOP-03 and STOP-04 apply; no vulnerable state; contain the extra interface by the pre-approved ladder step (link down) and open a class B2 record; every reading that says PASS is wrong |

### Scoring of the guided lab (formative; the guided lab is not a scored attempt)

A completion record, 100 points. A guided result with a hint or a published solution is marked guided evidence. PROPOSED.

| Block | Points | Items |
|---|---:|---|
| steps 1 to 3 | 15 | scope lines and RoE before commands 5; resource table with sources 5; Stage 00 record with leaf or honest unfinished tree 5 |
| steps 4 and 5 | 25 | NIC and resource inventory 5; addresses, routes, IPv6 observations 5; T8, T6, T7, T5 each with a result and its deciding line, stimulus present 15 (3 for T8, 3 for T6, 3 for T7, 6 for T5 including the stated limit) |
| steps 6 and 7 | 15 | baseline proofs with predictions first 8; both predictions of step 7 marked with their reason 7 |
| steps 8 and 9 | 30 | four tests recorded 8; the tampered service reply 4; the telemetry gap named 4; the fix on the cause 4; positive retests 5; near-miss negative retests 5 |
| step 10 | 10 | hashes before revert 4; marker equality and re-run tests 6 |
| record | 5 | the lab record, whose status is the lowest the evidence supports, with the independence declaration |

Blocked lab: score the variant lines on their own evidence (the sample arithmetic and the four-host table for steps 2 and 3; the two isolation sets for steps 4 and 5; H0 and the drills for steps 6 to 9) and write NOT DEMONSTRATED for the rest. A record of 80 or more with no gate failure completes the guided lab; the week's gate is decided by the Friday and `EVIDENCE_CHECKLIST.md`, not by this score.

### Overclaims to reject

- "The network is isolated" from a name, a switch, a definition, a missing route or an empty capture without a stimulus.
- "The service is secure" from a lower exposure value.
- "labuser2 had no access" after step 7a, as a general statement: it had none through the directory that was still closed.
- "The journal proves who edited the report."
- "chmod 777 would have fixed it", or any correction that widens.
- A restore claimed on the strength of a shutdown, a snapshot name or a running guest.
- Any statement that a command was run when the record says PROPOSED.

## Independent lab

### Fixture map: what is planted where

All paths are under `LABS/fixtures/independent/` unless stated. Provisioning file `provision_independent.sh` installs them with the modes that make the state.

| Id | Deviation | Where it lives | Policy line it breaks | How it is observed | Severity in this lab (reasoned) |
|---|---|---|---|---|---|
| I-F1 | an SSH setting that is written as one value and is effective as another: the image drop-in `10-image.conf` line 2 sets password authentication to yes; the hardening drop-in `60-hardening.conf` line 2 and the main file line 8 say no (the keyboard-interactive method is switched off on line 9, so the password method is the only password path); the first obtained value wins and the drop-ins are read at the position of the `Include` on line 3 of the main file, in name order, so `10-image.conf` wins. The same mechanism makes X11 forwarding effective yes (line 3 of the image drop-in against line 3 of the other drop-in and line 11 of the main file) | the three SSH files; installed by the script into `/etc/ssh` | R5 (the X11 setting is not covered by the policy) | administrator's test-mode dump shows `passwordauthentication yes` and `x11forwarding yes`; a no-credential probe lists the password method among those offered; reading the files alone suggests no | moderate: reachable only over loopback in this lab; in a real host it would be high |
| I-F2 | one cause, two symptoms: the spool directory and its records belong to group `ops` with mode 2770 and 0660 where the policy wants `auditors` with 2750 and 0640 (the script's lines for the spool directory and the two records) | `/srv/g9toy-audit/spool` | R3 and R4 | `labuser3` is refused at the directory; `labuser4` can read, append, create and delete; the service still serves because it owns the files | high: integrity of audit records and availability for the auditor |
| I-F3 | a delegation that matches more than intended: the second clause of `sudoers.d/40-ops` (line 2) ends in a wildcard after the file-name prefix; the arguments are matched as one string, so a second path argument is matched; the command runs as root | `/etc/sudoers.d/40-ops` | R4 (one file only) | `sudo -l -U labuser4` shows the pattern; an argument string with a second path (a probe file inside the root-only restricted directory) is accepted | highest: an arbitrary-path write as root for one account, bounded here to a synthetic marker |
| I-F4 | a secret in the unit's environment: `g9toy-audit.service` line 9 | the unit | R6 | any account can list the unit's environment property through the manager and read the token string (ASSUMED manager behaviour; the learner's own observation as an unprivileged account decides) | moderate: exposure to every local account; the token is unused by the toy |
| I-F5 | a listener wider than the policy: `g9toy-audit.conf` line 2 binds all addresses | the configuration | R2 | `ss -ltn` shows the wildcard address on 8090 | low in this lab (no other host reaches the DMZ guest and no exception is open), moderate on a real host |

Compliant but unusual items (to be reported as compliant): the setgid bit on the spool directory (it is wanted: new files inherit the group); the first clause of the delegation (exact arguments for restart); `PermitRootLogin prohibit-password` (a root login by password is refused, which satisfies the wording of R5); `ListenAddress` in the main SSH file (on a release where the daemon is started by a socket unit the listening address comes from that unit, ASSUMED, release-dependent: the learner must read the socket owner, not infer from the file); the root-only directory with its marker (it exists as the test target for I-F3 and is compliant); `Restart=on-failure`; `X11Forwarding yes` effective is an observation, not a policy deviation.

Requirement table of a correct analysis: R1 MET (the unit runs as the service account: check the manager's user property); R2 NOT MET (I-F5); R3 NOT MET (I-F2); R4 NOT MET (I-F2 and I-F3; the restart clause is MET); R5 NOT MET (I-F1); R6 NOT MET (I-F4).

### What a correct analysis finds, and the corrections

| Id | Cause at the boundary | Correction (least privilege) | Positive retest | Negative retest (near miss) |
|---|---|---|---|---|
| I-F1 | the precedence rule of the daemon: the first obtained value wins, drop-ins come first; a hardening file with a higher number cannot override a lower one | remove the setting from the image drop-in (or give the hardening setting the lowest number), check the configuration, reload; do not add a fourth file | the check passes, reload succeeds, allowed users are still offered a key method | the no-credential probe now offers only the key method; the effective dump shows `passwordauthentication no` |
| I-F2 | the wrong group and a mode with group write on the directory and records | group `auditors` and mode 2750 on the directory, 0640 on the records, keep setgid | labuser3 reads; the service serves and counts | labuser4 refused on read, append, create and delete; a new file created by the service inherits `auditors` |
| I-F3 | the delegation pattern is wider than the one file the policy allows, and it runs as root | replace the clause by the exact file path, or delegate to the service account instead of root; keep the exact restart clause; note that a file created by the delegated write has the delegated identity's umask mode | the operator restarts the service and places the one file | an argument string with a second path is refused; the marker in the restricted directory does not appear |
| I-F4 | a secret placed in a property that the manager exposes to every account | remove it from the unit; supply it by a credential file readable only by the service account, or the manager's credential mechanism (release-dependent, ASSUMED) | the service starts and answers | the unprivileged listing of the unit's properties no longer shows the string |
| I-F5 | the bind address in the configuration | loopback | the service answers on loopback and reports its identity | the socket listing shows no wildcard 8090 |

Ranking: I-F3 first (privileged write primitive), I-F2 second (integrity and availability), I-F1 third (only as exposed as the daemon is reachable), I-F4, I-F5. Accept another ranking if it states the exposure it assumes and a reachability argument; reject an unreasoned one. The task needs three corrections; the three ranked first are the reference set; any three of the five that act on their cause with both retests earn the marks.

Alternatives and discriminating tests the record should hold: for I-F2 "the service is broken" against "the directory refuses the auditor" (the service's own reply succeeds while the auditor's direct read fails); for I-F1 "the daemon was not reloaded" against "another file wins" (the dump after a reload still shows the value; reading the precedence resolves it); for I-F3 "the rule is intended" against "the rule is wider than intended" (the policy's wording and the second-path test).

### Scoring of the independent lab (formative)

100 points, PROPOSED. A hinted result is guided.

| Block | Points | Items |
|---|---:|---|
| process | 12 | scope block and test plan before the first command 4; hypothesis log with predictions 4; hint log and independence declaration 4 |
| requirement table | 12 | six decisions at 2 each, all with deciding evidence |
| findings | 55 | five findings at 11: cause at the boundary 4; preconditions and bounded impact 3; the strongest alternative with its test 2; status label correct 2 |
| corrections and retests | 15 | three corrections at 5: acts on the cause, side effect and rollback stated 2; positive retest 1.5; near-miss negative retest 1.5 |
| record | 6 | status the evidence supports; compliant items reported as compliant 3; baseline notes 3 |

Pass line 80 with no safety or integrity failure. A blocked lab: the paper variant is scored on the same blocks with every claim INFERRED; the correction and retest blocks are scored as proposed text and capped at half. A hosted variant is scored on the process, requirement and record blocks only, and the rest is NOT DEMONSTRATED.

### Overclaims to reject

- "Password logins are disabled" from the main file or the hardening file.
- "The delegation gives a root shell" without a test; the demonstrated effect is a write to a path of the lab's own tree.
- "Remote code execution", "compromised host" or any impact beyond the evidence.
- "The wildcard bind is exposed to the Internet": the guest has no route; reachability is not shown.
- "The token is used by the service": the toy does not use it; the finding is its visibility.
- A correction by making the spool root-only, which breaks the auditor and the service.
- A retest that repeats the exact first test and nothing else.
- A compliant item reported as a finding (the setgid bit, the first sudo clause, root login by key).

## Friday practical

### Catalogue with parameter spaces, application and repair (instructor-held)

For protocols 1 and 3 and for any local mode. The baseline is the clean state that `friday/provision_baseline.sh` leaves behind. Each class has at least five parameter values; a draw picks one to three classes and a value for each; the draw and the applied-fault log stay sealed until the report is submitted. Commands are proposed text, run as root in the guest only, with a rollback by the baseline restore. `<...>` is chosen from the parameter space.

| Class | Parameter space | Apply (proposed) | Observed by | Repair |
|---|---|---|---|---|
| C01 group | group on outbox directory or batch file: `loaders`, `labuser7`, `adm`, `sudo`, `svcspool` | `chgrp <group> <path>` | `stat`; readers lose access or loaders gain it | `chgrp shippers <path>` |
| C02 mode | directory mode `2770`, `2775`, `2755`, `0775`, `2757`; file mode `0666`, `0664`, `0644`, `0660`, `0604` | `chmod <mode> <path>` | `stat`, group and other bits | `chmod 2750` for the directory, `0640` for the file |
| C03 parent | mode of `/srv/g9toy-spool`: `0700`, `0750`, `0710`, `0770`, `0701` (owner root, group root) | `chmod <mode> /srv/g9toy-spool` | `namei -l` shows the component that refuses; the file's own mode is correct | `chmod 0755 /srv/g9toy-spool` |
| C04 ACL | named entry on outbox: `labuser7` with `rwx`, `-wx`, `r-x`, `rw-`, `r--`; or `labanalyst` with the same; default entries or not (needs the ACL tools in the guest, ASSUMED) | `setfacl -m u:<user>:<perm> /srv/g9toy-spool/outbox` | the plus sign after the mode string; `getfacl`; the group bits of the long listing show the mask | `setfacl -x u:<user> <path>`, or `setfacl -b <path>` |
| C05 membership | remove `labuser6` from `shippers`; add `labuser7` to `shippers`; add `labanalyst` to `shippers`; add `labuser6` to `loaders`; add `labuser7` to `sudo`; or the same change made after a session of that account was opened | `gpasswd -d <user> <group>` or `gpasswd -a <user> <group>` | group database against the groups of the running session | `gpasswd` the inverse; a new login |
| C06 identity | `User=` in a drop-in: `root`, `labuser7`, `labuser6`, `labadmin`, `nobody` | a drop-in file with the line, then reload and restart | the manager's user property; the service's own identity reply; the administrator's socket listing | remove the drop-in, reload, restart |
| C07 sandbox | `ReadWritePaths=` naming: a path that does not exist, the outbox, `/srv`, nothing; or `ProtectSystem=full` in place of strict | edit the sandbox drop-in, reload, restart | exit status 226/NAMESPACE, or the self-test result of the service | restore the drop-in |
| C08 listener | bind `0.0.0.0`, `127.0.0.2`, the DMZ address; port `8101`, `8080` | `sed -i` on `/etc/g9toy-spool/g9toy.conf`, restart | `ss -ltn` | restore the two lines, restart |
| C09 SSH effective | a lower-numbered drop-in `05-extra.conf` setting: password authentication yes; root login yes; strict modes no; empty passwords yes; an extra allowed user | create the file | the administrator's test-mode dump; the no-credential probe | remove the file, check, reload |
| C10 SSH file modes | for an account with a key installed by the preparer: `.ssh` `0775`; home `0777`; `authorized_keys` `0664`; owner of `authorized_keys` root; `.ssh` `0757` | `chmod` or `chown` | the daemon's refusal line in the journal | `chmod 0700 .ssh`, `0600 authorized_keys`, `0750 home`, owner restored |
| C11 delegation | `/etc/sudoers.d/40-spool` with: a copy of an argument-wildcard rule for the file-copy program, for the print program, for the edit-in-place program; or a file whose name contains a dot (it does not apply); or mode 0644 | create the file; `visudo -cf` first | the administrator's listing for the account; the diagnostics of the tool | remove the file |
| C12 secret | a token string in a drop-in `Environment=`; in an environment file with mode `0644`, `0640` with group `shippers`, `0600` (compliant), world-readable and referenced by the unit | create the file and the drop-in | the manager's property listing as the analyst; `stat` | remove, or keep root-only with `0600` |
| C13 setgid | clear the setgid bit on outbox or on inbox | `chmod g-s <dir>` | `stat` flags; the group of the next new file | `chmod g+s <dir>` |
| C14 state directory | `StateDirectoryMode=` `0755`, `0775`, `0777`, `0770`, `0701` | drop-in, reload, restart | `stat` of the directory | restore `0750` |
| C15 file owner | owner of `batch_0001.txt`: `labuser7`, `labadmin`, `root`, `labuser6`, `labanalyst` | `chown <owner> <file>` | the service's reply for that file and the journal's refusal code; `stat` | `chown svcspool` |

Decoys the preparer may add (each is compliant): the inbox dropbox mode (group write and search, no read), a second unused drop-in with only comments, a world-readable configuration without secrets.

### Fixture map of the evidence bundle (instance R0, folder `friday/bundle_01/`)

| Id | Item | Where | What it is |
|---|---|---|---|
| R0-A | a named ACL entry gives `labuser7` read, write and search on the outbox directory | `03_filesystem.txt` part 2, the listing and the ACL dump of the outbox | a deviation from P2 and P3 (class C04) |
| R0-B | `batch_0002.txt` is owned by `labuser7`, group `shippers`, mode 0640, created at 16:12:41 after labuser7's login at 16:12:03 | `03_filesystem.txt` part 2; `06_journal_labadmin_extract.txt` | a consequence of R0-A, class C15 as a symptom; the service identity (not labuser6) is refused at 16:15:52 |
| R0-C | labuser6's `.ssh` directory has mode 0775 | `03_filesystem.txt` part 2, last listing; `06` line for 17:03:40 | class C10; the daemon refuses the key (S2); the SSH configuration is not at fault |
| R0-D | compliant items | the setgid bit on outbox; the plus sign as such; the inbox dropbox mode; the service listening on loopback 8100 as the policy says; the sandbox drop-in; `PermitRootLogin no`; the empty effective sudo state of the analyst | none is a finding |
| R0-E | limits of the analyst capture | `02`, `03` part 1, `04`, `05` part 1, `07` | `sudo` needs a password; the outbox, the home directory and the sudoers directory are unreadable; the process column of the socket listing is empty; the test-mode dump needs host keys; the journal is not visible |

### What a correct analysis finds in R0

- S1 is not a permission problem of labuser6. The refused identity is the service account (journal at 16:15:52), which is neither the owner of `batch_0002.txt` nor in its group. Negative control: `batch_0001.txt` is served, and the group `shippers` can read `batch_0002.txt` directly (labuser6 is a member).
- The cause of S1 sits one level up: a named ACL entry lets `labuser7` create files in the outbox, which the policy forbids (P2, P3). The file labuser7 created carries its owner. The long listing misleads: with an ACL, the group bits show the mask, so `rws` on the directory does not mean the owning group may write (the ACL dump shows the owning group entry is read and search only).
- S2 is explained by the daemon's refusal line for labuser6's `.ssh` directory in the journal and the group-writable mode shown by the administrator's listing; labuser7's `.ssh` is 0700 and its login is accepted.
- What the analyst could not determine, and must say so: which process owns the 8100 socket (inferred from the manager's main PID and user, not from the socket listing), the effective SSH configuration beyond the file (the dump is administrator-only), the sudo rules (not readable).
- Corrections: remove the named entry (`setfacl -x`, or `-b` if no other entry is wanted) and put `batch_0002.txt` under the service's ownership and the readers' group, or remove it if it is not a legitimate batch; `chmod 0700` on labuser6's `.ssh`. Positive retests: the service answers `batch_0002.txt`; loaders can still deliver to the inbox; labuser6 logs in with the key. Near-miss negatives: labuser7 creating a file in the outbox is refused; `.ssh` at 0775 is refused again (restored afterwards). In paper mode these are proposed text with predicted outputs labelled NOT RUN.
- Residuals to state: no evidence that `labuser7` changed `batch_0001.txt` (its owner and group are the service's; no modification time is given); the preparer cannot show how the ACL arose.

### Scoring map

| Sub-criterion | Reference R items for R0 (local and hosted instances use the same shape) |
|---|---|
| P1 | R1 scope block before the first action (paper mode: the bundle hash and the note of what is in scope); R2 an ordered test plan; R3 every logged action in scope; V target identity from an independent source (the ticket and the capture notes agree on host and accounts) |
| P2 | R1 inventory of accounts, groups, paths with modes and ACL, unit state, listener, SSH files, sudo state; R2 each fact with file, command and time; R3 limits stated (the five items of R0-E); R4 no claim beyond the artifacts; V a second source for a key fact (the ACL dump and the plus sign; the journal and the listing) |
| P3 | R1 falsifiable hypotheses before tests (for S1: the service identity versus labuser6; for S2: the daemon configuration versus file modes); R2 no hint, no solution; R3 reproduction or its documented impossibility (paper: documented); R4 a negative control (labuser6 can read the second batch directly; labuser7's login works while labuser6's does not); R5 the final claim equals the evidence; V a second method (the permission model from the listing and the journal) |
| P4 | R1 cause at the control (the named ACL entry for the role; the mode of the account's `.ssh`); R2 preconditions (a login of labuser7; the daemon's mode checks); R3 bounded impact (integrity and availability of outbox batches; no execution claim); R4 the alternative explanations (stale group session, SSH configuration, service bug) with the discriminating test; V a discriminating test run or specified |
| D1 | R1 the data sources that show the behaviour (the service log for the refusal, the daemon log for the refusal of the key, the listing and ACL dump for the permission); R2 shown to exist and be collected (the extract with times); R3 the expected event written before the test; R4 no absent-log-as-absent-action; V event seen at the source and again in a second place |
| D2 | R1 positive cases (the service serves the batch; loaders deliver; labuser6 logs in); R2 predicted before; R3 times tie to actions; R4 attached (paper: predicted outputs labelled NOT RUN); V repeated or confirmed in a second place |
| D3 | R1 near-miss negatives (create in outbox refused; the loosened mode refused again); R2 one variable differs from the positive; R3 false-positive and false-negative risks named (the plus sign is not itself a fault; a refusal can be the service's or the user's); V a bypass attempt of the same class (a delete of a file in outbox by labuser7) |
| D4 | R1 mitigation on the cause; R2 side effects and regression risk (removing the entry must not remove a needed grant; mask recomputation); R3 rollback; R4 residual risk; V applied in an approved lab, or analysed on paper and labelled |
| E1 to E3 | per the rubric: environment and versions, exact commands with status words, input hashes (the bundle's), expected against observed, cleanup; every claim labelled, bundle hash before the key was opened; a specific correction, a retest with positive and negative case and its observed result or the reason it could not run |

An instance drawn under protocol 1 or 3 uses the same R-item shape with its own faults; the preparer's sealed record is the map. In mode H the P and D blocks are scored on the analysis and the proposed tests; the platform's own ground truth stands in for the fixture map and no solution is read before submission.

### Preparer's checklist (protocols 1 and 3)

Before the clock: the baseline built and the isolation tests current; the SHA-256 of the fault record given to the learner and written on the scorecard; one to three classes chosen by a physical die or by a seed written outside the learner's reach; any key pair needed for class C10 created inside the guest (the public part installed; no private key leaves the guest); the preparer's pre-flight shows the scenario can be solved in the time and the guest has no route off the lab; a catalogue of at least twelve classes with at least five parameter values each (the table above satisfies it). After the report: open the sealed record, compare it with the learner's findings, and write the comparison on the scorecard. Limits to record: protocol 3 is self-prepared; the author knows the catalogue.

### Overclaims to reject

- Root, remote code execution, or "the service is compromised" from any item of R0.
- "labuser6 lacks permission" for S1.
- "The ACL is the vulnerability": the entry that exceeds the role is.
- "The SSH daemon is misconfigured" for S2: its file is compliant; the account's directory mode is the cause.
- "Setgid on the outbox is a misconfiguration" or "the inbox mode is wrong" (both compliant).
- A claim about what `ss -p` or the journal shows that the capture notes contradict.
- A fix that widens a group, `chmod 777`, or removes the service's ownership.
- "Retest passed" with no output, or from before the change.
- Any missing item of the learner's own list presented as an absent thing.

## Fixture drills

### Drill 01 (`d02/perm_drill_01.txt`)

| Request | Result | Deciding element |
|---|---|---|
| 1 | allowed | owner class of the file has read, and the directory `data` grants search to the group `team` that labuser1 belongs to |
| 2 | denied | the directory `data` has no bits for others, so search fails before the file's mode is considered, although the file is world-readable |
| 3 | denied | labuser1 is the owner, the owner class is selected and it has no bits; group and other permissions are not consulted |
| 4 | allowed | not the owner; the primary group `team` matches the file's group; the group class has read; the directory allows search to the group |
| 5 | allowed | owner class has write, and the directory `shared` grants search |
| 6 | allowed | labuser1's supplementary group `team` matches; the group class has write on the file; the directory grants group search |
| 7 | denied | the directory allows search to others, but the file gives others nothing |
| 8 | denied | to add an entry the caller needs write plus search on the directory; others have read and search only |
| 9 | allowed | the group class of the directory has write and search; the new file belongs to labuser1 and, because the directory has the setgid bit, to group `team`; its mode is 0644 for a creation request of 0666 under umask 022 |
| 10 | create allowed, list denied | the group class of `upload` is write and search without read: a name can be created and reached, the directory cannot be opened for listing |
| 11 | denied | unlinking a name is decided by write and search on the containing directory; the group class of `data` has read and search only, so owning the file does not help |
| 12 | allowed | labuser2 owns the directory `shared` with write and search, and no sticky bit restricts removal |

### Drill 02 (`d02/perm_drill_02.txt`)

| Request | Result | Deciding element |
|---|---|---|
| 1 | allowed | the named entry for labuser5 on the file is read and write, the mask is read, so the effective permission is read; the directory entry gives search; reading needs both |
| 2 | denied | the same entry's effective permission is read only because the mask removes write |
| 3 | allowed | the owner entry is not limited by the mask and has read and write |
| 4 | denied | the old session's credentials do not hold the group added later; the class falls to other, which has nothing, and the directory search fails first |
| 5 | allowed | the new session holds the group; the group entry on the file gives read; the group entry on the directory gives read and search after the mask |
| 6 | denied | the directory's group entry shows write but the mask reduces it to read and search: the effective permission lacks write |
| 7 | denied | no entry and no group; other has nothing; the directory refuses search |
| 8 | denied | the file system is mounted read-only: mode and privilege do not apply to a write to a read-only mount |
| 9 | denied | discretionary access control allows the owner, and the confinement profile of the program refuses the open (the audit line names the profile, the path and the mask); mandatory access control is evaluated in addition to the mode |

### Samples of day 3 (`d03/`)

| File | Reference reading |
|---|---|
| `journal_ssh_sample.log` | line 2: the daemon refuses a key because of the mode or owner of an account's directory, a permission cause and not a configuration cause; lines 10 and 11: an existing account that is not on the allow list is treated as invalid; lines 12 and 13: a key failure followed by an accepted password for the same account proves that the password method is available to that account in the daemon's effective configuration, whatever the files say; lines 7 to 9: an unknown account can still be offered a password prompt. A missing line is never evidence of an absent attempt |
| `journal_unit_sample.log` | lines 2 to 6: a start failure in the namespace set-up caused by a path that does not exist, exit status 226, then a scheduled restart on line 7; line 12: the `EACCES` refusal is the service's own identity; line 13: `EROFS` is the sandbox; line 14: a hostile name is rejected as data; the gap of about twelve minutes between line 7 and line 8 is not explained by the log |
| `sudoers_sample.txt` | the alias rule is exact and compliant; labuser2's rule runs a reading program as root with an argument wildcard, so a second path argument is matched and any root-readable file can be read; the group rule runs a writing program as the service account under the state directory pattern, so any file that account can write can be written through a second argument or a parent component; the administrator's line is intended. The file name has no dot, so it is read (ASSUMED includedir rule: names with a dot or ending in a tilde are skipped; test with the listing on the guest) |

## Inventory fixtures

Expected values for gate test IV3, computed from the fixtures with the reference rules below. The utility's own rules may differ if documented; the facts may not.

| Source | Expected |
|---|---|
| `d04/passwd_sample` | six accounts: root (uid 0, interactive shell), daemon (nologin), svclab (uid 998, nologin), labadmin (1000, interactive), labuser1 (1001, interactive), labuser2 (1002, interactive); four accounts with an interactive shell |
| `d04/group_sample` | seven groups; `sudo` has the member labadmin; `labusers` has labuser1 and labuser2; `labreaders` has labuser1 |
| `d04/passwd_sample_02` | nine lines: three parse as accounts (root and the labuser2 line twice), of which the duplicate must be flagged; five are rejected with a reason (too few fields, too many fields, non-numeric uid, empty name, the name-service marker line); one is blank. The totals add to nine |
| `d04/proc_sample/100`, `101`, `102` | pid 100: name python3, uid and gid 998 (svclab), no groups, empty capability masks, no-new-privileges set, umask 0027; pid 101: bash, uid 1001 (labuser1), groups 1001, 2000 and 2001, parent 900, no new privileges not set; pid 102: python3, uid 0 (root), full effective and bounding capability masks, parent 1, no-new-privileges not set |
| `d04/systemd_show_sample.txt` | four units: `g9toy.service` active, running, enabled, main pid 100, user svclab; `g9toy-audit.service` active, running, enabled, main pid 102, empty user (so root, and the process sample agrees: uid 0); `ssh.service` active, enabled, main pid 900, empty user; `g9toy-old.service` failed, disabled, main pid 0, user svclab. Pid 900 is not in the process sample: the inventory reports it as absent from the input and does not invent its uid |
| `d04/ss_sample_admin.txt` | three listeners with owners: loopback 8080 by pid 100, loopback 22 by pid 900, wildcard 8090 by pid 102 |
| `d04/ss_sample_unprivileged.txt` | the same three listeners with the process unknown for each: the inventory records the owner as unavailable and never copies the owner from the other file |
| findings (reference, others allowed with evidence) | a root-run service holding a wildcard listener (`g9toy-audit.service`, pid 102, wildcard 8090); a failed unit; the cross-check between main pids and the process sample; a listener-owner mismatch is not present |
| `d04/auth_sample_01.log` | 31 lines, none rejected: 10 failed-password events (admin 2, root 4, `$(id)` 1, `a;b` 1, labuser2 1, oracle 1), from four sources: 203.0.113.7 six events, 198.51.100.23 two, 10.77.10.10 one, 2001:db8::7 one; 5 of the 10 are for an invalid user; 2 accepted logins (labuser1 by key, labuser2 by password); 19 other lines (13 other messages of the daemon and 6 of the delegation tool, which has no process id in its prefix). The two hostile user names are output as data |
| `d04/auth_sample_02.log` | 24 lines, the last without a newline. Reference rules: syslog prefix with a valid calendar date, sshd as program, an exact failed-password message, a valid address (IPv4 or IPv6), a port from 1 to 65535, a user name of at most 256 characters. Result: 7 failed-password events (admin twice, the non-ASCII name once, `$(id)` twice, the traversal-like name once, root from the IPv6 source once); 1 accepted; 1 other program; 2 blank; 13 rejected; the line table below gives each outcome |

Line outcomes of `auth_sample_02.log` under the reference rules (line number: outcome). Lines whose outcome depends on documented parser policy are marked policy.

| Line | Outcome |
|---|---|
| 1 | event (admin, 203.0.113.7, invalid user) |
| 2 | rejected: truncated line |
| 3 | rejected: no port |
| 4 | rejected: port not numeric |
| 5 | rejected: address octet out of range |
| 6, 7 | blank |
| 8 | other program, not a failure |
| 9 | policy: extra whitespace; a tolerant parser accepts it as an event, a strict one rejects it with a reason |
| 10 | policy: lower-case message text; the daemon never writes it, a strict parser rejects it |
| 11 | rejected: impossible date (Feb 30) |
| 12 | event with a non-ASCII user name, output as valid JSON |
| 13 | rejected: empty user name |
| 14 | policy: a 2000-character user name; the reference rejects it as over-length; accepting it needs a documented limit and no crash |
| 15, 16 | two events (identical lines are two attempts unless a documented rule says otherwise); the name stays data |
| 17 | event; a path-like name is never used as a path |
| 18 | policy: a tab inside the name; either rejected with a reason or parsed as data |
| 19 | accepted login |
| 20 | policy: a different timestamp format; rejected as unknown format or supported by a documented rule |
| 21 | event from an IPv6 source |
| 22 | rejected: port out of range |
| 23 | event |
| 24 | rejected: truncated line at the end of the file without a newline |

Totals under the strict reference: events 7 (lines 1, 12, 15, 16, 17, 21, 23), accepted 1, other program 1, blank 2, rejected 13 (lines 2, 3, 4, 5, 9, 10, 11, 13, 14, 18, 20, 22, 24). With the tolerant policies of lines 9, 10, 14, 18 and 20 the event count rises and the rejected count falls by the same number; the sum is always 24. Per source for the strict reference: 203.0.113.7 two, 203.0.113.9 one, 198.51.100.23 two, 198.51.100.24 one, 2001:db8::7 one.

Marking for IV3: every fact above that the utility's rules support is present; no fact is invented; any divergence from the strict reference is explained by a documented rule. A parser that crashes, drops a line without counting it, or lets accepted plus rejected plus blank plus other differ from the total fails IV7.
