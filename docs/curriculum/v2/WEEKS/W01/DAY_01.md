# G9 W01 / DAY_01 — Linux privilege model, host diagnostics, libvirt failure modes and isolation evidence

- Status: PROPOSED
- Day id: D01
- Original calendar date: 2026-11-16 (Monday)
- Duration: 8 instructional hours (R 0.5 h; A 2 h; B 2 h; C 2.5 h; D 1 h)
- Lab authorization: PROPOSED, unsigned, void without a signed RoE. W01-GUIDED-A (class B, `HOST:G9`, approved Stage 00 read-only rows, block B); W01-GUIDED-B1 and B2 (classes D, E, `G9-ATK-01` `10.77.10.10`, `G9-TGT-01` `10.77.20.20`, tests T8, T6, T7, T5, block C, only if separately approved and Stage 00 resolved: not today); W01-GUIDED-H (class F, `HOSTED:overthewire:bandit`, block C, rules read and dated first). Denied: host as target, household LAN, Internet, other addresses; STOP-01 to STOP-16.
- Dependencies: none (day 1); host terminal for read-only rows, `LABS/fixtures/d01/`, an SSH client; `w01-otw-bandit` FREE; no local lab exists while Stage 00 is blocked.
- Primary provider: libvirt (documentation) | Secondary: OverTheWire

## Objectives
1. Entry diagnostic; Linux privilege model; UEFI, kernel/process concepts — Parts 1 and 3 of the entry diagnostic locked by hash before the key opens and scored as placement; the four user IDs, capability sets, firmware-to-PID-1 path and guest-as-host-process explained in your words. [linux.privilege-boundaries]
2. Inspect actual host diagnostics; identify libvirt failure mode; plan two-guest lab — failed `virsh` command recorded verbatim, each Stage 00 row run as approved or NOT RUN, a decision-tree leaf named from evidence, a fit table for stacks A and B with sources. [lab.authorization, linux.processes-services]
3. Create only if separately approved and verify RED/DMZ virtual isolation; otherwise use hosted labs and write configuration plan — T8, T6, T7, T5 each PASS, FAIL, INCONCLUSIVE or NOT RUN with the deciding line; or "isolation: BLOCKER, no intrusive local testing" with a paper topology. [lab.authorization, linux.evidence-completeness]

## Hand-in
Evidence manifest, baseline measurements, network diagram, unresolved blockers — one record a stranger can check: each artifact with SHA-256 and time; each figure with source row and status word; the plan's zones, addresses, MACs and test states; each blocker with what it blocks and its substitute.

## Resources
| Id | Use in this day | Block |
|---|---|---|
| `w01-man7-credentials` | the four user IDs and group list, one sentence each | A |
| `w01-man7-capabilities` | which capability set is checked | A |
| `w01-libvirt-daemons` | `libvirtd` against `virtqemud`, socket names | B |
| `w01-ubuntu-libvirt` | system and session mode, guest confinement (ASSUMED here) | B |
| `w01-libvirt-formatnetwork` | what an isolated network still allows | C |
| `w01-otw-bandit` | hosted substitute from level 0, estimate 30 minutes; levels reached, never a password | C |
| `w01-htb-platform-rules` | read once, record the date; governs every Hack The Box item | D |

## R — Active recall (30 min)
No queue exists on day 1. Closed book, in writing, no search; block A checks it against the manuals.
- 10 min: two sentences each on what a process, a file mode and a network route are; one on what `root` can and cannot do.
- 10 min: one failed experiment of your own, in your words: what you did, expected, saw, what you now think explains it, and one check that would test that. Look nothing up.
- 10 min: write the plan for finding out why `Failed to connect socket to '/var/run/libvirt/libvirt-sock': No such file or directory` appeared: questions in order, and which would change the machine. Block B compares it with the decision tree.

## A — Entry diagnostic, privilege model and boot chain (120 min)
Time plan (PROPOSED, estimates): the plan gives the diagnostic no hours and its file estimates about 225 minutes. So 55 minutes for Part 1 (30 questions) of `docs/curriculum/v2/ASSESSMENTS/ENTRY_DIAGNOSTIC.md`, 5 to lock it, 40 lesson, 15 guided lab step 1. Part 3 sits in block C; Part 2 is skipped and the placement is provisional; an unanswered question scores 0. Closed book, no search, no AI, no commands; mark guesses `?`; hash the answers before opening the key. A placement, not a grade.

Lesson (`w01-man7-credentials`, `w01-man7-capabilities`).
- Identity: a process holds real, effective, saved and filesystem user IDs (the `Uid:` line of `/proc/PID/status`, in that order), the same for groups, and a supplementary group list fixed at creation and inherited. File access uses the filesystem ID, which follows the effective one. To drop privilege for good a program sets all three IDs; a saved 0 lets it regain root.
- Capabilities: root's powers are split (override file checks, read-search, bind below 1024). The kernel checks the effective set (`CapEff`), not the UID; execve gives UID 0 a full set unless the bounding set (`CapBnd`) removes bits. `NoNewPrivs` blocks gains through setuid and file capabilities. AppArmor acts after these checks and only refuses more.
- Firmware to PID 1: UEFI starts an EFI program from the EFI System Partition (shim and GRUB under Secure Boot), which loads kernel and initramfs; the kernel mounts root and executes init (systemd) as PID 1, ancestor of every user process. No init, or an exiting PID 1, panics the kernel. Secure Boot checks what firmware loads, not the running system.
- Guest as host process: `kvm` and `kvm_amd` expose `/dev/kvm`; QEMU, a user-space process, opens it; each vCPU is a QEMU thread run in guest mode by the hardware; virtual devices are QEMU code, so a flaw there runs with QEMU's host privileges and libvirt confines the process. `Virtualization: AMD-V` is a CPU capability, not a working `/dev/kvm`, and that is not an accelerated guest.

Wrong models and tests: "UID 0 does anything": `CapEff` of a root service with an empty `CapBnd` (extension E2.3, day 3) predicts EACCES where the model predicts success. "A new group applies at once": `id` in an old shell against `id` by name (day 2). "AMD-V means KVM works": rows S-06, F-06. "No `libvirt-sock` means no libvirt": S-05 contacts no daemon; modular sockets are `/run/libvirt/virtqemud-sock`. "A switch named `g9-lab-dmz` is isolated": definition, NICs, routes, a probe inside a capture window (block C).

Guided lab step 1 (15 min, `LABS/GUIDED_LAB.md`): scope lines, RoE draft, stack record, stop rule before any command; the first command-log line is later than its scope line.

## B — Stage 00 diagnostics and the two-guest plan (120 min)
Prerequisites: a terminal on the trusted host, never a target; every command read-only, approved in writing, expected output written first (`docs/curriculum/v2/HOST_READINESS.md` section 4). No guest exists; nothing installs, enables, starts or restarts. Read `w01-libvirt-daemons`, `w01-ubuntu-libvirt`. Estimates: 10, 25, 15, 55, 15 minutes for steps 1 to 5.
1. Evidence gap G1: the failed `virsh` command as typed, its user, the full error; compare with your block R plan.
2. Four synthetic hosts, `d01/host_diag_sample_01.txt` to `_04.txt`: one line each: decision-tree leaf (HOST_READINESS section 7), deciding line, next read-only row, what is not established. They are independent paper data.
3. Fit arithmetic on `d01/host_budget_sample.txt` for stack A (6 vCPU, 9 GiB, 120 GiB thin) and B (8, 11, 140), rule stated: threads are a ceiling, not a reservation; P-06 (PROPOSED) wants available memory of stack RAM plus 8 GiB; thin disks grow, and baselines multiply what is consumed.
4. Real rows S-01 to S-09 one at a time, then the tree: from your evidence choose F-08 to F-11, F-17 (layout, socket, group) or F-24 to F-27 (rules, routes); F-28 needs `sudo`. Save each output verbatim with its time; interpretation separate, INFERRED. Any connection to a libvirt daemon is class C: it can start a socket-activated daemon.
5. Paper plan of stack A: `G9-ATK-01` RED `10.77.10.10/24`, MAC `52:54:00:77:10:0a`, `g9-lab-red`, `virbr91`; `G9-TGT-01` DMZ `10.77.20.20/24`, MAC `52:54:00:77:20:14`, `g9-lab-dmz`, `virbr92`; one NIC each, no default route, IPv6 off, no host address in `10.77.0.0/16`. They cannot reach each other: the expected state.

Ground truth: unit files (S-05) with the socket directory (F-11); route table (S-07) with rules (F-24). Three unexplained outputs in a row is STOP-15.

## C — Isolation evidence, paper topology and hosted Bandit (150 min)
Task, modified and unseen: two evidence packages from two stacks. Scope: the blocked branch, class A paper work and, once the owner has read and dated the rules, class F `HOSTED:overthewire:bandit`. The local branch (guided lab steps 4 and 5) needs separate approval and P-01 to P-14; assume nothing local exists. Estimates: 45 min Part 3 of the diagnostic (cap; its own estimate is about 75), 40 packages, 35 topology, 30 Bandit.
1. Part 3 of the diagnostic; lock and hash.
2. Judge `d01/isolation_evidence_set_01.txt` and `_02.txt`: for T8, T6, T7, T5 (LAB_ARCHITECTURE section 11) give PASS, FAIL, INCONCLUSIVE or NOT RUN, the deciding line and what it does not prove. Success: each verdict cites a line and names its stack and policy; an empty capture counts only if the package shows the probe, its time inside the window; a name or an absent route is never proof of enforcement. Negative control: before set 2, write what a T8 failure would look like; a PASS on all four tests must be defended line by line or withdrawn.
3. Paper topology and test plan: zones, guests, addresses, bridges, MACs, routing matrix with default deny, the nine tests NOT RUN, their order (T8, T6, T7, T5 before any vulnerable service) and the evidence each needs.
4. Hosted: read and date the game's rules, then `w01-otw-bandit` from level 0 under W01-GUIDED-H; no automation the rules forbid; passwords in a private note; record levels reached, they count toward tomorrow. Rules unread means class F is unavailable.

## D — Evidence manifest, baseline, diagram and blockers (60 min)
Telemetry today is evidence text: saved outputs with times, the package lines and, if a test ran, router, firewall and capture records. Name the gap: any test without a recorded stimulus. Control: containment designed, then measured: no `forward` or `ip` element on lab networks, one NIC per guest, no default route, IPv6 off, host rules dropping input and forward on lab bridges, no host address in `10.77.0.0/16`; on a blocked host it is NOT APPLIED, design only. Positive retest: the permitted flow works; negative: the forbidden flow fails, stimulus and time recorded. Neither ran unless approved: write NOT RUN, never PASS.
Estimates: 20 min open the diagnostic key, score, fill the placement sheet, one mechanism line per miss; 15 manifest; 10 baseline table; 10 diagram and blockers; 5 three flashcards of mechanisms and limits. Read `w01-htb-platform-rules`, write the date.

## Commands proposed for review
| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| 1 | `date -u +%Y-%m-%dT%H:%M:%SZ` | host (G1.1) | session start in UTC | one line ending in `Z` | none | user | host | none needed (read-only) |
| 2 | `lscpu` then `free -h` then `df -h /` | host (S-01 to S-03) | threads, virtualization line, memory, free space | CPU lines, available memory, free space | none | user | host | none needed (read-only) |
| 3 | `systemctl list-unit-files 'libvirtd*' 'virtqemud*' 'virtproxyd*' --no-pager` | host (S-05) | which libvirt units exist, no daemon contacted | unit table, or `0 unit files listed.` | none | user | host | none needed (read-only) |
| 4 | `ls -l /dev/kvm` | host (S-06) | KVM device node | root, group `kvm`, or `No such file or directory`; necessary, not sufficient | none | user | host | none needed (read-only) |
| 5 | `ip -4 route` then `ip -6 route` then `ip rule show` | host (S-07, S-08, F-24) | main-table routes and policy rules | default routes with metrics; empty IPv6 output is not proof IPv6 is off | none | user | host | none needed (read-only) |
| 6 | `ls -la /run/libvirt/` then `id` | host (F-11, F-17), only if the tree selects them | socket files, your groups | sockets with owner and mode, or `No such file or directory`, unlike `Permission denied` | none | user | host | none needed (read-only) |
| 7 | `virsh -c qemu:///system domiflist g9-tgt-01`, then in the guest `ip route get 192.0.2.1` | host libvirt (X-01), guest (X-05); local branch, separate approval | NIC inventory; any route off the lab | one NIC on `g9-lab-dmz`; `Network is unreachable` | the connection can start a socket-activated daemon | libvirt connection; user | the one guest | none needed (read-only); a started daemon idles out |
| 8 | `ssh -p PORT USER@HOST`, values from the Bandit level 0 page | `HOSTED:overthewire:bandit` (class F) | log in to level 0 | login prompt, then a shell on the game host | a session on a shared host | the game's account | that exercise; rules read | log out; delete files you made |

## Offense, detection, remediation, retest
| Offensive mechanism | Defensive observation | Remediation | Retest |
|---|---|---|---|
| a second NIC on a NAT network, or a mistaken default route, lets a guest reach the LAN or Internet through the host and past a VPN's default-route rules | NIC lists live against persistent (T8); guest `ip route get`; tap capture filtered on the stimulus destination, since NAT rewrites the source; router or firewall log | remove the NIC and any `forward`; no default route in guests; drop rules on lab bridges and taps; fail-closed VPN rule in the firewall, not in metrics | T8, T6, T5 with a stimulus inside the window: connect fails, no packet leaves, a log line |
| a guest reaches host services over the bridge (ARP, a service on all addresses, IPv6 link-local) | host addresses on `virbr*`, sysctl values, host ruleset | drop input and forward from lab bridges; `disable_ipv6`; no host lab address | host-address probe with capture (T5 variant 8, T7) fails |
| membership of the group owning a libvirt socket is root-equivalent (ASSUMED) | `ls -la /run/libvirt/`, `id`, `getent group` | minimal membership, a recorded privilege decision (O-HR-03) | F-17, F-18 again; a non-member is refused |

## Quiz
### Recall (closed book)
1. [W01D01-R1] A client on an Ubuntu host fails with `Failed to connect socket to '/var/run/libvirt/libvirt-sock': No such file or directory`. List causes from at least four layers; say what `No such file or directory`, `Permission denied` and `Connection refused` each tell you about the socket path; name the first diagnostic and why it does not change the machine.
2. [W01D01-R2] A process shows `Uid: 1000 0 0 0` in `/proc/PID/status`. Name the four fields in order, say which identity decides whether it may open a root-owned 0600 file and the decision, and what the program must do to drop root for good and why one ID is not enough.
3. [W01D01-R3] Which attribute does the kernel check for a privileged operation, and which per-process set limits what execve can add? A service starts as UID 0 with an empty bounding set: name one thing it can still do and two it cannot.
4. [W01D01-R4] A machine prints `Kernel panic - not syncing: No working init found` after mounting its root filesystem. Which boot stages had succeeded, which step failed, and what does this show about the first user-space process?
5. [W01D01-R5] In a KVM guest, what runs as host threads or host code and where does device emulation run? Why does a virtual-device flaw expose the host while no network isolation test addresses it? Name two controls and the risk that remains.
### Applied
1. [W01D01-A1] Sample (synthetic): a reviewer writes "isolated: NIC on `g9-lab-dmz`, planned MAC, capture empty". The package: `virsh domiflist g9-tgt-01` shows one NIC on `g9-lab-dmz`, MAC `52:54:00:77:20:14`; `virsh net-dumpxml g9-lab-dmz` holds `forward` with `mode='nat'` and an `ip` element `10.77.20.1` with DHCP; host `virbr92` has `10.77.20.1/24`; guest `ip route` begins `default via 10.77.20.1`; an uplink capture, filter `net 10.77.0.0/16`, ran 15:20:00Z to 15:35:00Z with 0 packets; the guest ran `timeout 5 bash -c ': < /dev/tcp/203.0.113.10/22'` at 15:41:09Z and printed `exit=0` (203.0.113.10 stands for the owner's household device). Give T6 and T5 as PASS, FAIL, INCONCLUSIVE or NOT RUN with the deciding line, name the lines that contradict the claim, say why the empty capture does not support it, and list kinds of evidence, other than a name, that do support isolation.
2. [W01D01-A2] Sample (synthetic) host: `ip rule show` prints `32764: from all lookup main suppress_prefixlength 0`, `32765: not from all fwmark 0xca6c lookup 51820`, `32766: from all lookup main`; table 51820 holds `default dev wg0 scope link`; main holds `default via 198.51.100.1 dev enp7s0 metric 100`, `198.51.100.0/24 dev enp7s0 scope link src 198.51.100.20` and `192.0.2.0/24 dev virbr0 scope link`; `net.ipv4.ip_forward` is `1`. Guest `g9-tgt-01` has its planned NIC, 10.77.20.20/24, and a second NIC on a NAT libvirt network `default`, leased from 192.0.2.0/24. From these lines only: by which interface does a connection from the second NIC to the household router 198.51.100.1 leave the host, and one to a public address; what happens to the public one when `wg0` goes down; why a capture filtered on `net 10.77.0.0/16` shows neither; name two controls and how you would prove them; what does the correct lab address tell you?
3. [W01D01-A3] Sample (synthetic): `lscpu` prints `Virtualization: AMD-V`; `grep -c -w svm /proc/cpuinfo` prints `12` on 12 logical processors; `grep -e '^kvm ' -e '^kvm_amd ' /proc/modules` prints nothing; `ls -l /dev/kvm` prints `No such file or directory`; `journalctl -k -b -g kvm --no-pager` shows that the AMD module was refused because SVM is disabled by the firmware. Say which layer each line speaks for, the cause the evidence supports and whose decision follows, two natural actions that are out of bounds, and what stays unproven once `/dev/kvm` appears.

## Hint ladder
1. No hint: for 45 minutes alone, write for each of T8, T6, T7, T5 what a package must contain for a PASS and look for it; give a verdict on set 1 before opening set 2, with the one line that would change each.
2. Conceptual hint: ask of each capture whether the stimulus is inside it and whether the source could have been rewritten; of each network name which definition stands behind it; of each missing line whether the test that makes it was run.
3. Scoped technical hint: for each guest of set 2 compare live and persistent NIC lists with the MAC plan, its routes and `disable_ipv6` value with the design, and each probe time with each capture window.
4. Instructor explanation: in the answer key, after the attempt is submitted.

## Troubleshooting and rollback
- Expected: each row prints what you predicted or something you can explain; the four synthetic hosts reach different leaves; the record says NOT RUN wherever nothing ran.
- Missing dependency: a tool or column is absent (`docker`, `MOUNTPOINTS`): record the message, install nothing; a substitute is a new command needing review.
- No connectivity: the hosted game is unreachable: change no host network setting, finish the paper tasks, mark the hosted part NOT RUN. Two stack A guests not reaching each other is expected.
- Unexpected result: save it, write what it contradicts and a test separating the explanations; three in a row is STOP-15.
- Safety breach: stop, do not fix forward, keep evidence; a guest that seems to reach the LAN, host or Internet is STOP-03; open the incident record (SAFETY_AUTHORIZATION section 10).
- Restoration: today changes nothing on the host; delete files made on the hosted game; no guest exists to shut down.
- Time exceeded: stop at the block boundary, record blocker and substitute (HOST_READINESS section 9.3); unrun means NOT RUN; never claim the original exercise passed.

## Evidence and acceptance
- Evidence: diagnostic answer files with hash and time; scope lines and RoE draft; one Stage 00 record per row; fit table; verdict table for both packages; paper topology and test plan; Bandit record; blockers list; manifest with SHA-256 per file.
- Grading: scope lines precede commands; every figure has a source row and status word; every verdict cites its deciding line and stack; nothing says a guest or test was run without saved output.
- Negative test: a package with an empty capture is INCONCLUSIVE or worse unless a stimulus inside the window is shown; a claim resting on a name, a missing route or a configuration file alone is rejected.

## Expert extension (optional)
Read the libvirt connection-URI page (not in the registry, UNVERIFIED) and write one page on how a client with no URI picks the system or session instance. Or design on paper the host firewall table dropping input and forward from the lab bridges plus a fail-closed VPN rule, with the test that proves each rule fires. Not counted in the 480 hours.

## Spaced recall
- 1 day: W01D01-R1, W01D01-R2
- 3 days: W01D01-R3
- 7 days: W01D01-R4
- 14 days: W01D01-R5

## Reflection
- What was observed and what was inferred?
- What was not proven?
- Which test would falsify the leading hypothesis?
