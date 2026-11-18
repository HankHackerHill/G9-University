# Guided lab W01-GUIDED: from an unsafe permission state to a verified least-privilege state, on a toy service

- Status: PROPOSED. Nothing in this file was run. Every command is proposed text for the owner's review; no output is presented as observed unless it is marked `Sample (synthetic):`.
- Lab id: W01-GUIDED. Week 1, days D01 to D03. Plan source: section 5 week 1 (guided lab specification, steps 1 to 10) and section 6 row 1 of `docs/curriculum/v2/MASTER_PLAN_V2.0.md`.
- Rules above this file: `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` (classes, scope lines, stop conditions), `docs/curriculum/v2/LAB_ARCHITECTURE.md` (guest ids, isolation tests T1 to T9, recovery), `docs/curriculum/v2/HOST_READINESS.md` (Stage 00). Where this file says "test T8" or "row S-05", the exact commands are in those files and are not repeated here.
- Stage 00 is BLOCKED (the libvirt socket). Parts A to D need that blocker resolved and the owner's approvals. Without them, run the variants in the Environment section: nothing local is assumed to exist.
- Fixtures: `docs/curriculum/v2/WEEKS/W01/LABS/fixtures/` (listed with hashes in `MANIFEST.md`). All are synthetic. The guided lab uses the folders `guided/`, `d01/` and `d02/`.

## Why it matters

A file mode is the last element of an access decision, not the whole of it. The kernel picks one class (owner, group or other) from the identity of the process, requires search permission on every directory of the path, and then applies the class bits; ACLs, mount options and mandatory access control can still refuse after that. Three habits recur in permission faults: widening a group or a mode to make a ticket go away, checking only the file and not its parents, and checking the configured state instead of the state a running process actually has.

This lab turns those habits into measurements on a toy service. You build a least-privilege baseline for two synthetic accounts and one service account, widen it the way a support workaround would, record what an account that should have no access can now do, correct it, and prove two things together: the permitted operations still work, and the forbidden ones fail in a near-miss form of the same test. A correction that only passes the second proof is a broken service; one that only passes the first is no correction.

The lab also measures the isolation that the rest of the course relies on. A network name proves nothing. The evidence is the guest's interface inventory, its routes, the host's view of the bridge, and a probe that leaves a trace in a capture. The steps follow the plan's ten, in order, so that the plan's guided specification can be checked against this file line by line.

What it does not show: a toy service with a made-up protocol is not a real service, synthetic accounts are not a real directory, and one guest is not a production host. The mechanisms are real; the exposure is not.

## Scope and authorization

Every scope line below is PROPOSED and unsigned. A line without a signed rules-of-engagement record (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 12) is void. Copy one line per window and change the WINDOW when a block moves.

```text
SCOPE W01-GUIDED-A | TARGET HOST:G9 | CLASS B | ADDR none (local read-only commands; no network address is contacted) | FROM host terminal | WINDOW 2026-11-16T10:45/2026-11-16T12:45 America/Chicago | ALLOW the Stage 00 read-only rows named in the approval record | DENY any command not approved, any state change, any libvirt connection (class C), any scan | STOP STOP-01..STOP-16 | ROE <record id> | TERMS none
SCOPE W01-GUIDED-B1 | TARGET G9-ATK-01,G9-TGT-01 | CLASS D | ADDR 10.77.10.10,10.77.20.20 | FROM host libvirt connection and guest consoles | WINDOW 2026-11-16T13:45/2026-11-16T16:15 America/Chicago | ALLOW starting the two guests, the static address file and the sysctl file inside each guest, the fixture medium | DENY any host change beyond the approved commands, the router and CORP guests, package installation, any network staging | STOP STOP-01..STOP-16 | ROE <record id> | TERMS none
SCOPE W01-GUIDED-B2 | TARGET G9-ATK-01,G9-TGT-01 | CLASS E | ADDR 10.77.10.10,10.77.20.20 and the one address the RoE designates for test T5 | FROM G9-ATK-01 and G9-TGT-01 consoles, host captures | WINDOW 2026-11-16T13:45/2026-11-16T16:15 America/Chicago | ALLOW tests T8, T6, T7 and T5 of LAB_ARCHITECTURE section 11, one probe per variant | DENY every address except the two guest addresses, the documentation addresses the tests name and the designated T5 address; any vulnerable service before T8, T6, T7, T5 pass | STOP STOP-01..STOP-16 | ROE <record id> | TERMS none
SCOPE W01-GUIDED-C | TARGET G9-TGT-01 | CLASS E | ADDR 10.77.20.20 (the toy service listens on 127.0.0.1 only) | FROM G9-TGT-01 console | WINDOW 2026-11-17T08:30/2026-11-17T10:30 America/Chicago | ALLOW creation of the synthetic accounts and files of steps 6 to 9, permission changes under /srv/g9toy, requests to the toy service on 127.0.0.1, reading its journal | DENY any path outside /srv/g9toy, /opt/g9toy, /etc/g9toy, /var/lib/g9toy and the one unit file, any other address, any change to sshd, the firewall or the router | STOP STOP-01..STOP-16 | ROE <record id> | TERMS none
SCOPE W01-GUIDED-H | TARGET HOSTED:overthewire:bandit | CLASS F | ADDR the host and port the platform shows for the game | FROM host terminal (SSH client) | WINDOW 2026-11-17T10:45/2026-11-17T12:45 America/Chicago | ALLOW logging in with the game's own accounts and reading what the game exposes | DENY every other host, other players, any scan, any automation the platform's rules forbid | STOP STOP-01..STOP-16 | ROE <record id> | TERMS OverTheWire <date read>
```

Rules that hold in every part: the trusted host is never a target; guests have no route to the household LAN or the Internet; no real credential exists anywhere (accounts are `labadmin`, `svclab`, `labuser1`, `labuser2`, and passwords are never needed in this lab); evidence leaves a guest as text only. Linux account names here carry no dot, because the guest's `useradd` decides which names are valid (ASSUMED; the rule depends on the installed shadow-utils), and `lab.user1` of the fixture rules would risk a refusal.

A stop condition ends the exercise at once (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 9). The ones most likely here: STOP-03 (a guest appears able to reach the LAN, the host or the Internet, or an unexpected interface or route appears), STOP-04 (an isolation test fails or its evidence is stale), STOP-11 (a cleanup step cannot be shown), STOP-15 (three commands in a row did not match their expected output and are not understood).

## Environment

### Stacks and ids

| Item | Value |
|---|---|
| Stack A (the first sub-step of the plan: two guests, no router) | `G9-ATK-01` RED `10.77.10.10/24`, `G9-TGT-01` DMZ `10.77.20.20/24`, on separate isolated networks. They cannot reach each other: that is the expected state |
| Stack B (adds the router; needed only for extension step 9c and for test T1) | stack A plus `G9-RTR-01` with `10.77.10.1`, `10.77.20.1` and `10.77.30.1`; exception E1 (RED to DMZ, TCP 8080) opened for one window |
| Networks and bridges | `g9-lab-red` (`virbr91`), `g9-lab-dmz` (`virbr92`); no `<forward>`, no `<ip>` (design in LAB_ARCHITECTURE section 5; NOT APPLIED) |
| Synthetic MACs | `G9-ATK-01` `52:54:00:77:10:0a`; `G9-TGT-01` `52:54:00:77:20:14` (LAB_ARCHITECTURE section 3.1) |
| Accounts in `G9-TGT-01` | `labadmin` (administrator; the installer's first account, with sudo rights, ASSUMED), `svclab` (service account, no login shell), `labuser1` (authorized reader), `labuser2` (unauthorized synthetic account) |
| Groups | `labusers` (broad: `labuser1`, `labuser2`), `labreaders` (narrow: `labuser1`) |
| Paths in `G9-TGT-01` | `/opt/g9toy/g9toy_reportd.py`, `/etc/g9toy/g9toy.conf`, `/etc/systemd/system/g9toy.service`, `/srv/g9toy/reports/`, `/srv/g9toy/spool/`, `/srv/g9toy/private/`, `/var/lib/g9toy/` |
| Toy service | `g9toy.service`, TCP `127.0.0.1:8080`, protocol of the header of `guided/g9toy_reportd.py` |
| Fixture medium in the guest | read-only, mounted at `/mnt/g9fx`, holding only the folder `guided/` for this lab (recipe W01-R1 in `MANIFEST.md`) |
| Evidence folder | `<evidence-folder>/w01/` on the host (the owner's private folder); in the guest, outputs are appended to files under `~/w01-evidence/` and leave the guest as text through the channel the owner chose (open point O-W01-01 of `DESIGN_NOTES.md`: LAB_ARCHITECTURE defines an evidence-out channel only for `G9-MAL-01`) |

### Resources and setup time

| Item | Stack A | Stack B |
|---|---|---|
| Configured vCPU, RAM, thin disk | 6 vCPU, 9 GiB, 120 GiB | 8 vCPU, 11 GiB, 140 GiB (35% of the 31 GiB budget) |
| Host memory rule at session start (P-06, PROPOSED heuristic) | available memory of at least 17 GiB | at least 19 GiB |
| Setup time, ESTIMATE, not measured | part A 60 to 90 min; part B 90 to 120 min with guests already at baseline B0; part C steps 6 and 7 about 60 min, steps 8 and 9 about 60 min; step 10 about 30 min | add 45 min for the router rule, test T1 and its removal |

The guests themselves are built beforehand under separate approvals (class D, `docs/curriculum/v2/LAB_ARCHITECTURE.md` section 12, baseline B0). Building them is not part of this lab and its time is not counted here.

### Hosted alternative and variants when the local lab is blocked

Nothing local may be assumed. These variants keep the hours, replace only the setup-dependent work, are recorded on a substitution record (`docs/curriculum/v2/HOST_READINESS.md`, section 9.3) and never produce a statement that a guest was tested.

| Step | Variant | What to practise there | Evidence to keep | Estimate (ESTIMATE) | What it cannot show |
|---|---|---|---|---|---|
| 1 | none needed: it is writing | scope lines, RoE draft | the written lines | 15 min | nothing is lost |
| 2 | the synthetic sample `d01/host_budget_sample.txt`, then the real rows once approved | the arithmetic of vCPU, memory and thin-disk fit; the P-06 rule | your table with each source | 20 min | the real fit if the rows were not run |
| 3 | the four synthetic hosts `d01/host_diag_sample_01.txt` to `_04.txt` | reading unit states, socket paths and error texts; naming the leaf of the decision tree and the next read-only row | one line per host: leaf, evidence line, next row, what is not established | 40 min | the owner's real host; only a real row shows it |
| 4 and 5 | none exists on a hosted platform (registry gap of `RESOURCES.json`): a paper topology plus the two synthetic evidence packages `d01/isolation_evidence_set_01.txt` and `_02.txt` | judging an isolation claim from evidence: which line proves what, what an empty capture does not prove | per test T8, T6, T7, T5: PASS, FAIL, INCONCLUSIVE or NOT RUN with the line that decides it; the paper topology with the written test plan | 60 min | any property of a real guest |
| 6 to 9, host-only | variant H0 below: a toy directory inside `<evidence-folder>` as the owner's own user | owner-class selection, search permission on a directory | modes and outputs of H0 | 45 min | a second account, a service, group semantics |
| 6 to 9, hosted | `w01-pwncollege-permissions` (Perceiving Permissions) and `w01-pwncollege-users` (Untangling Users), class F, free account; `w01-otw-bandit-levels-0-8` on day 2 | reading modes, changing owner and group, switching users and the delegation mechanism; reading files by access semantics | per challenge class: the permission fact that mattered (mode, owner, group), your reasoning in your words; no flag, no credential, no screenshot | 45 to 75 min for the pwn.college modules; 120 min for the Bandit range as the plan's block B | your own service, your own logs, a before and after of a fault you induced |
| 6 to 9, paper | the drills `d02/perm_drill_01.txt` and `d02/perm_drill_02.txt` | deciding twelve and nine access requests from a listing, naming the deciding element | your decision per request with the deciding element | 45 min | any observation |
| 10 | a written restore plan: baseline names, method, the marker check of test T9 as text | what a restore must prove | the plan, labelled NOT RUN | 15 min | that a restore works |

Platform terms must be read and dated by the owner before the first hosted use: until they are, class F is unavailable (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 8.2). Never publish a Bandit password, a flag or a solution step.

### Variant H0: a toy directory on the host, inside the evidence folder, as the owner's own user

No guest, no second account, no privilege, no network. Run it as your ordinary account: the superuser bypasses the checks it demonstrates. Every command writes only inside `<evidence-folder>/w01-toy/`. Each still needs the owner's individual approval, because the plan asks that the real host not be changed outside individually approved commands. Classification is PROPOSED: SAFETY section 2.2 names "any file outside the evidence folder" as class C, so a write inside it is not class C; the owner may classify it otherwise.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| H0.1 | `mkdir -p <evidence-folder>/w01-toy/data` | host, evidence folder | creates the toy tree | no output | one directory | user | `<evidence-folder>/w01-toy` only | `rm -r <evidence-folder>/w01-toy` after evidence is copied |
| H0.2 | `printf 'synthetic toy file\n' > <evidence-folder>/w01-toy/data/a.txt` | host, evidence folder | creates the toy file | no output | one file | user | the toy tree | delete the file |
| H0.3 | `stat -c '%A %U:%G %n' <evidence-folder>/w01-toy <evidence-folder>/w01-toy/data <evidence-folder>/w01-toy/data/a.txt` | host, toy tree | baseline modes and owners | three lines; your user and your primary group; modes such as `drwxr-xr-x` and `-rw-r--r--` (they follow your umask) | none | user | the toy tree | none needed (read-only) |
| H0.4 | `chmod 0044 <evidence-folder>/w01-toy/data/a.txt` then `cat <evidence-folder>/w01-toy/data/a.txt` | host, toy file | tests owner-class selection: write the prediction first | `cat` fails with `Permission denied` although group and other have read: the owner class was chosen and it has no bits | changes one mode | user | the toy file | `chmod 0644 <evidence-folder>/w01-toy/data/a.txt` |
| H0.5 | `chmod 0600 <evidence-folder>/w01-toy/data` then `ls <evidence-folder>/w01-toy/data` then `cat <evidence-folder>/w01-toy/data/a.txt` | host, toy directory | directory read without search: names list, the file cannot be reached | `ls` prints `a.txt`; `cat` fails with `Permission denied` | changes one mode | user | the toy directory | `chmod 0755 <evidence-folder>/w01-toy/data` |
| H0.6 | `chmod 0100 <evidence-folder>/w01-toy/data` then `ls <evidence-folder>/w01-toy/data` then `cat <evidence-folder>/w01-toy/data/a.txt` | host, toy directory | search without read: a known name can be reached, the directory cannot be listed | `ls` fails with `Permission denied`; `cat` prints the file | changes one mode | user | the toy directory | `chmod 0755 <evidence-folder>/w01-toy/data` |
| H0.7 | `stat -c '%A %U:%G %n' <evidence-folder>/w01-toy/data <evidence-folder>/w01-toy/data/a.txt` | host, toy tree | proves the restore | modes equal those of H0.3 | none | user | the toy tree | none needed (read-only) |

Restore the file mode (the rollback of H0.4) before H0.5, or H0.6 cannot show the file being reached. H0 shows that a mode can lock out its own owner and that a directory bit decides before a file bit. It does not show an unauthorized account being denied: that claim needs a second identity and stays NOT DEMONSTRATED until a guest or the hosted modules supply one.

## Preconditions

| Id | Condition | How it is checked | If not met |
|---|---|---|---|
| L1 | the plan's Stage 00 blocker is resolved to the point that the owner has approved building and starting guests: P-01 to P-14 of `docs/curriculum/v2/HOST_READINESS.md` section 8 | the owner's records; a leaf named in the decision tree | parts B to D are BLOCKED: use the variants above; part A is still possible |
| L2 | a signed RoE per scope line above, with the approved command list | the RoE records | no command of that part is run |
| L3 | guests `G9-ATK-01` and `G9-TGT-01` exist as powered-off baseline B0, on the two networks, with exactly one NIC each | the owner's build record; test T8 | stop; do not improvise a guest |
| L4 | isolation tests T8, T6, T7 and T5 are current and PASS for this stack before any vulnerable state is created | step 5 | step 7 is not started (STOP-04) |
| L5 | the fixture medium for `guided/` exists and its file hashes equal `MANIFEST.md` | `sha256sum` on the host before building the medium | rebuild the medium; never edit a fixture |
| L6 | the guest has Python 3, `systemd`, `sudo`, `namei` and `ss`; `getfacl` is optional (package `acl`, ASSUMED) | `command -v` per tool, written in the record | a missing tool is a missing dependency: install nothing; use `stat` and `namei`, record the gap |
| L7 | a baseline of the host's network state exists (P-05) | the Stage 00 records | test T5 stays NOT RUN |
| L8 | you wrote, before step 6, a prediction table of every test of steps 8 and 9 | the dated record | write it now; a prediction written after the test earns nothing in the marking |

## Steps

Each step lists what to write before, the commands, what to observe and the checkpoint. A command is proposed text: review it against `docs/curriculum/v2/STUDENT_GUIDE.md` section 4, write its expected output first, and run it only when it is approved. Commands that change a guest are class D or E and need the approval record of section 2.3 of SAFETY. "Guest console" means the local graphical or serial console of the guest; no SSH from the host to a guest exists in this design. Outputs you keep are appended to a file with `tee -a ~/w01-evidence/<name>.txt` and leave the guest as text.

### Part A: the host side (plan steps 1 to 3), day D01 block B

#### Step 1: write the authorized scope

Write the scope lines of the previous section, the RoE (template in SAFETY section 12), the stack record (guest ids, addresses, networks) and the stop rule in your words. Write a time record. Nothing in this step runs a lab command.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| G1.1 | `date -u +%Y-%m-%dT%H:%M:%SZ` | host | the session start in UTC | one line ending in `Z` | none | user | host | none needed (read-only) |
| G1.2 | `date +%Y-%m-%dT%H:%M:%S%z` | host | the same instant in local time with its offset | one line; in November an offset of `-0600` if the zone is America/Chicago (ASSUMED setting) | none | user | host | none needed (read-only) |

Checkpoint: scope lines, RoE and time record exist and are dated before the first approved command.

#### Step 2: compare the host's hardware with the allocation

First do the arithmetic on the synthetic host of `d01/host_budget_sample.txt`: threads against vCPU (a ceiling per guest, not a reservation), `available` memory against stack RAM plus 8 GiB (P-06), free space against the configured thin disks and against the plan's target of about 600 GB (thin disks grow; baselines multiply them). Then run the real rows, each with its expected output written first.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| G2.1 | `lscpu` | host (row S-01) | CPUs, threads per core, virtualization line | an x86-64 processor, a thread count, a `Virtualization:` line; a `Hypervisor vendor:` line only if the host is itself virtualized | none | user | host | none needed (read-only) |
| G2.2 | `free -h` | host (row S-02) | total and available memory | total, used, available and swap in binary units | none | user | host | none needed (read-only) |
| G2.3 | `df -h /` | host (row S-03) | free space of the root filesystem | size, used, available | none | user | host | none needed (read-only) |
| G2.4 | `df -hT /var/lib` | host (row F-22) | filesystem type and free space where system images would live | a type such as ext4, xfs or btrfs; it decides how baselines are copied | none | user | host | none needed (read-only) |
| G2.5 | `lsblk -o NAME,SIZE,FSTYPE,MOUNTPOINTS` | host (row S-04) | block devices and mounts | the layout; an older util-linux rejects the column `MOUNTPOINTS`, and the substitute is a new command needing its own review | none | user | host | none needed (read-only) |
| G2.6 | `ip route get 10.77.10.10` | host (row F-27) | the kernel's route choice for a lab address; no packet is sent | a route through the default route or a physical or VPN interface: no lab route exists on the host, which is the design | none | user | host | none needed (read-only) |

Checkpoint: a table with the plan's allocation (week 1 stack A: 6 vCPU, 9 GiB, 120 GiB thin; stack B: 8, 11, 140) beside each measured figure and its source row, the P-06 verdict for each stack, and the statement of what a thin disk does and does not promise.

#### Step 3: inspect the libvirt service and socket state

Before any command, write down the failed `virsh` command exactly as typed, the user, and the full error text (evidence gap G1 of HOST_READINESS). The plan's observation is only the path `/var/run/libvirt/libvirt-sock`. Practise first on the four synthetic hosts, naming for each the leaf of the decision tree, the evidence line that decides it and the next read-only row. Then run only the rows the tree selects from your returned evidence. The first row is S-05 because it separates "no daemon installed" from "daemon inactive" without contacting any daemon; a connection can start a socket-activated daemon and is class C.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| G3.1 | `systemctl list-unit-files 'libvirtd*' 'virtqemud*' 'virtproxyd*' --no-pager` | host (row S-05) | which libvirt unit files exist and their enablement | a table of units, or `0 unit files listed.` | none | user | host | none needed (read-only) |
| G3.2 | `ls -l /dev/kvm` | host (row S-06) | the KVM device node | a character device normally owned by root with group `kvm`, or `No such file or directory`; necessary for acceleration, not sufficient | none | user | host | none needed (read-only) |
| G3.3 | `systemctl list-sockets --all --no-pager 'libvirt*' 'virt*'` | host (row F-09) | loaded socket units, their listen paths, the service each activates | `LISTEN` paths, or an empty list | none | user | host | none needed (read-only) |
| G3.4 | `ls -la /run/libvirt/` | host (row F-11) | the socket files that exist, with owner, group, mode | sockets with modes, or `No such file or directory`; this differs from `Permission denied` (exists, no access) and from `Connection refused` (exists, no listener) | none | user | host | none needed (read-only) |
| G3.5 | `id` | host (row F-17) | your groups, to compare with the socket's group | a line listing groups | none | user | host | none needed (read-only) |
| G3.6 | `virsh -c qemu:///system version` | host, system libvirt instance (row C-01, CLASS C) | only if the tree reaches it and the owner approves it separately: asks the daemon its version | version lines if the daemon answers, otherwise the error text, which is evidence | may start a socket-activated daemon, which then stays up until it idles out or is stopped | libvirt system connection (root or the socket's group, ASSUMED) | one connection, no definition | none for files the daemon creates; the service stops by its idle timeout (ASSUMED) or by a stop that is itself a class C decision |

Checkpoint: a recorded leaf (L1 to L15) with the rows that led to it, or the statement that the tree is not finished, plus the list of what is not established. Stage 00 status is BLOCKED until the cause is named from recorded evidence; it is never EVIDENCE-COMPLETE because this lab says so.

### Part B: two guests and isolation (plan steps 4 and 5), day D01 block C

Requires L1 to L5. If any is unmet, run the variant of steps 4 and 5 and stop this part.

#### Step 4: bring up the two Linux guests

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| G4.1 | `virsh -c qemu:///system net-list --all` | host libvirt (row LA-05) | the lab networks exist and nothing else is attached to them | `g9-lab-red` and `g9-lab-dmz` listed, no autostart; any `default` network has no lab guest | none | libvirt system connection | listing only | none needed (read-only) |
| G4.2 | `virsh -c qemu:///system domiflist g9-atk-01` and the same with `g9-tgt-01` | host libvirt (row X-01) | NIC inventory before power-on | one line each: network `g9-lab-red` with MAC `52:54:00:77:10:0a`; network `g9-lab-dmz` with MAC `52:54:00:77:20:14` | none | libvirt system connection | the two guests | none needed (read-only) |
| G4.3 | `virsh -c qemu:///system start g9-tgt-01` then `virsh -c qemu:///system start g9-atk-01` | host libvirt (class C or D approval) | powers the two guests on | a line that the domain started, for each | boots a guest; volatile state exists from now on; a tap interface appears on the host per guest | libvirt system connection | the two guests | `virsh -c qemu:///system shutdown <domain>` (row R-01) |
| G4.4 | `virsh -c qemu:///system dominfo g9-tgt-01` and the same with `g9-atk-01` | host libvirt | vCPU and memory against the allocation | `CPU(s): 2` and 3 GiB for the target; 4 and 6 GiB for the attacker | none | libvirt system connection | the two guests | none needed (read-only) |
| G4.5 | `ip -br link` | host | the bridges and taps among the existing interfaces | `virbr91`, `virbr92` and one `vnet` tap per running guest (two); nothing unexpected | none | user | host | none needed (read-only) |

Checkpoint: the inventory equals the MAC plan, one NIC per guest, and the resources equal the allocation.

#### Step 5: static addresses and a measured isolation

Write the prediction first: what each guest's route table and `ip route get 192.0.2.1` will show, and what a probe toward the designated LAN address will do. Then, in each guest console:

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| G5.1 | `ip -br link` and `ls -l /etc/netplan/` | each guest console | the NIC's real name and any configuration that already covers it | one interface besides `lo`, name assumed `enp1s0` in the fixtures; a list of files; netplan merges all files by name order | none | user | the guest | none needed (read-only) |
| G5.2 | `sudo install -m 0600 -o root -g root /mnt/g9fx/guided/netplan_atk.yaml /etc/netplan/60-g9-lab.yaml` (the attacker; `netplan_tgt.yaml` for the target) | each guest | installs the static address, with no default route and no link-local address | no output | writes one file | sudo in the guest | the guest's `/etc/netplan` | `sudo rm /etc/netplan/60-g9-lab.yaml` then `sudo netplan apply` |
| G5.3 | `sudo netplan generate` | each guest | validates the file before it is applied | no output on success; a message naming the line on error | writes generated files under `/run` | sudo in the guest | the guest | removed with the file of G5.2 |
| G5.4 | `sudo netplan apply` | each guest | applies the address | no output; the console connection is not affected | changes the live network configuration of the guest | sudo in the guest | the guest | G5.2 rollback, then this command |
| G5.5 | `sudo install -m 0644 -o root -g root /mnt/g9fx/guided/sysctl_lab.conf /etc/sysctl.d/60-g9-lab.conf` then `sudo sysctl -p /etc/sysctl.d/60-g9-lab.conf` | each guest | turns IPv6 off for the lab | the two settings echoed back with value 1 | writes one file; changes two kernel parameters | sudo in the guest | the guest | `sudo rm /etc/sysctl.d/60-g9-lab.conf`; set both values to 0 with `sudo sysctl -w` or reboot |
| G5.6 | `ip -br addr` then `ip route` then `ip -6 route` then `ip route get 192.0.2.1` then `sysctl net.ipv6.conf.all.disable_ipv6` | each guest | what the guest believes | one lab address; a connected route only; no IPv6 route; `RTNETLINK answers: Network is unreachable` for the documentation address; the sysctl value 1 | none | user | the guest | none needed (read-only) |

Sample (synthetic): what G5.6 prints on the attacker in stack A. Not an observation.

```text
$ ip -br addr
lo               UNKNOWN        127.0.0.1/8
enp1s0           UP             10.77.10.10/24
$ ip route
10.77.10.0/24 dev enp1s0 proto kernel scope link src 10.77.10.10
$ ip route get 192.0.2.1
RTNETLINK answers: Network is unreachable
```

Then run, in this order and as written in `docs/curriculum/v2/LAB_ARCHITECTURE.md` section 11, tests T8 (extra NICs), T6 (no public route), T7 (IPv6) and T5 (the household LAN, only if the owner designated the address under open decision O-LA-07; otherwise T5 is NOT RUN and isolation stays INCONCLUSIVE). Practise the judgment first on `d01/isolation_evidence_set_01.txt` and `_02.txt`. For each test keep: test id, run id `T<n>-<yyyymmdd>-<k>`, UTC and local time, the stack and baseline ids, commands as run with outputs, capture files with SHA-256, result and rationale. A capture that holds no packet proves nothing unless the record shows the probe that should have made one, with its time inside the capture window. A route that is merely absent is a property of the configuration; an enforced denial needs the router or firewall log.

Checkpoint: each of T8, T6, T7 and T5 has PASS, FAIL, INCONCLUSIVE or NOT RUN with its evidence. A vulnerable state is created in step 7 only if T8, T6, T7 and T5 are PASS for this stack. Otherwise write "isolation: BLOCKER, no intrusive local testing" and continue with the variants.

### Part C: accounts, an unsafe state, unauthorized test access and its correction (plan steps 6 to 9), day D02

Run in `G9-TGT-01`. Two phases, matching the plan's blocks: steps 6 and 7 in block A, steps 8 and 9 in block D.

#### Step 6: synthetic accounts and a least-privilege baseline

The baseline: the service account owns the data and nobody else may write; only the narrow group may read; the broad group has no access; the service listens on loopback; the unit is started but not enabled.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| G6.1 | `sudo groupadd labusers` then `sudo groupadd labreaders` | G9-TGT-01 | creates the broad and the narrow group | no output | edits `/etc/group` | sudo in the guest | the guest | `sudo groupdel labreaders` and `sudo groupdel labusers` |
| G6.2 | `sudo useradd --system --user-group --no-create-home --shell /usr/sbin/nologin svclab` | G9-TGT-01 | the service account | no output | adds an account and a group | sudo in the guest | the guest | `sudo userdel svclab` |
| G6.3 | `sudo useradd --create-home --shell /bin/bash --groups labusers,labreaders labuser1` then `sudo useradd --create-home --shell /bin/bash --groups labusers labuser2` | G9-TGT-01 | the authorized reader and the unauthorized account; no password is set, so neither can log in | no output | two accounts with home directories | sudo in the guest | the guest | `sudo userdel --remove labuser1` and `sudo userdel --remove labuser2` |
| G6.4 | `sudo install -d -m 0755 -o root -g root /opt/g9toy /etc/g9toy /srv/g9toy` then `sudo install -m 0644 -o root -g root /mnt/g9fx/guided/g9toy_reportd.py /opt/g9toy/g9toy_reportd.py` then `sudo install -m 0644 -o root -g root /mnt/g9fx/guided/g9toy.conf /etc/g9toy/g9toy.conf` then `sudo install -m 0644 -o root -g root /mnt/g9fx/guided/g9toy.service /etc/systemd/system/g9toy.service` | G9-TGT-01 | installs the service code, its configuration and its unit | no output | writes four files and three directories | sudo in the guest | the paths of this lab | `sudo rm -r /opt/g9toy /etc/g9toy /srv/g9toy /etc/systemd/system/g9toy.service` |
| G6.5 | `sudo install -d -m 0750 -o svclab -g labreaders /srv/g9toy/reports` then `sudo chmod 2750 /srv/g9toy/reports` | G9-TGT-01 | the data directory with the setgid bit, set explicitly so that the result does not depend on `install` | no output | one directory | sudo in the guest | the reports directory | `sudo rm -r /srv/g9toy/reports` |
| G6.6 | `sudo install -m 0640 -o svclab -g labreaders /mnt/g9fx/guided/reports/q3_summary.txt /srv/g9toy/reports/q3_summary.txt` then the same for `q3_detail.txt` | G9-TGT-01 | the two synthetic reports | no output | two files | sudo in the guest | the two report files | `sudo rm` of each file |
| G6.7 | `sudo install -m 0640 -o svclab -g labreaders /dev/null /srv/g9toy/reports/q3_canary.txt` then `printf 'synthetic canary\n' > /tmp/w01-canary.txt` then `sudo sh -c 'cat /tmp/w01-canary.txt > /srv/g9toy/reports/q3_canary.txt'` | G9-TGT-01 | a sacrificial file for the delete test of step 8, with its owner, group and mode intact | no output | one file in the data directory and one in `/tmp` | sudo in the guest | the canary and its source file | `sudo rm /srv/g9toy/reports/q3_canary.txt /tmp/w01-canary.txt` |
| G6.8 | `sudo install -d -m 0750 -o svclab -g svclab /srv/g9toy/spool` then `sudo install -d -m 0700 -o root -g root /srv/g9toy/private` then `sudo install -m 0600 -o root -g root /mnt/g9fx/guided/private_ledger.txt /srv/g9toy/private/ledger.txt` | G9-TGT-01 | the service's scratch directory and a root-only synthetic file used by the delegation extension | no output | two directories, one file | sudo in the guest | the two directories | `sudo rm -r /srv/g9toy/spool /srv/g9toy/private` |
| G6.9 | `sudo systemctl daemon-reload` then `sudo systemctl start g9toy.service` | G9-TGT-01 | starts the service without enabling it | no output | a running process as `svclab`; a state directory `/var/lib/g9toy` created by `StateDirectory=` | sudo in the guest | the guest | `sudo systemctl stop g9toy.service` |
| G6.10 | `systemctl is-enabled g9toy.service` then `systemctl is-active g9toy.service` | G9-TGT-01 | enablement and runtime state are two facts | `disabled` (exit status 1) and `active` | none | user | the guest | none needed (read-only) |
| G6.11 | `systemctl show g9toy.service -p User -p MainPID -p ActiveState -p UnitFileState` | G9-TGT-01 | the identity and state the manager holds | `User=svclab`, a main PID, `ActiveState=active`, `UnitFileState=disabled` | none | user | the guest | none needed (read-only) |
| G6.12 | `ss -ltn` then `sudo ss -ltnp` | G9-TGT-01 | the listening socket and its owner | `127.0.0.1:8080` listening; the process column shows `python3` only with `sudo` (the plan: do not assume an unprivileged user can inspect other users' processes; observe it) | none | user, then sudo in the guest | the guest | none needed (read-only) |
| G6.13 | `stat -c '%A %U:%G %n' /srv/g9toy /srv/g9toy/reports /srv/g9toy/reports/q3_summary.txt /srv/g9toy/reports/q3_detail.txt /srv/g9toy/reports/q3_canary.txt` then `namei -l /srv/g9toy/reports/q3_summary.txt` | G9-TGT-01 | the baseline modes, owners and the path walk, saved as the pre-state | the baseline listing below | none | user | the guest | none needed (read-only) |
| G6.14 | `g9q() { bash -c 'exec 3<>/dev/tcp/127.0.0.1/8080; printf "%s\nQUIT\n" "$1" >&3; timeout 3 cat <&3' _ "$1"; }` | the shell of G9-TGT-01 | defines a small client function for this session; it is not saved anywhere | no output | a shell function in this session only | user | the session | close the shell |

Sample (synthetic): the baseline listing of G6.13, one line per path. Not an observation.

```text
drwxr-xr-x root:root /srv/g9toy
drwxr-s--- svclab:labreaders /srv/g9toy/reports
-rw-r----- svclab:labreaders /srv/g9toy/reports/q3_summary.txt
-rw-r----- svclab:labreaders /srv/g9toy/reports/q3_detail.txt
-rw-r----- svclab:labreaders /srv/g9toy/reports/q3_canary.txt
```

Baseline proofs (record each, with its prediction written first). The sudo commands below use the administrator's rights to start a process as the named account; `sudo -u` rebuilds the target's supplementary groups from the group database, so each result reflects the database, not an old session.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| G6.15 | `sudo -u labuser1 -- cat /srv/g9toy/reports/q3_summary.txt` | G9-TGT-01 | positive: the authorized reader reads directly | the four lines of the synthetic report | none | sudo in the guest | the guest | none needed (read-only) |
| G6.16 | `sudo -u labuser2 -- cat /srv/g9toy/reports/q3_summary.txt` | G9-TGT-01 | negative: the unauthorized account is refused | `Permission denied`; the cause is the directory (search bit), not the file | none | sudo in the guest | the guest | none needed (read-only) |
| G6.17 | `g9q 'LIST'` then `g9q 'GET q3_summary.txt'` then `g9q 'COUNT'` then `g9q 'WHOAMI'` | G9-TGT-01 | the service works through its own identity | `OK 3`, three names and `OK bye`; `OK 138`, the report text and `OK bye`; `OK 1` and `OK bye`; `OK Uid=` followed by the service account's numbers and `OK bye` (every request closes with the `QUIT` that the client function appends) | the service logs each request; `COUNT` creates `/var/lib/g9toy/counter` | user | `127.0.0.1:8080` only | `sudo rm /var/lib/g9toy/counter` |
| G6.18 | `sudo journalctl -u g9toy.service --no-pager -o short-iso` | G9-TGT-01 | the service's own record of those requests | one line per request: peer, command, first reply line | none | sudo in the guest | the guest | none needed (read-only) |

Checkpoint: labuser1 reads, labuser2 is refused, the service answers, and the listener is on loopback. Save all outputs. If any result differs from your prediction, stop and explain it before step 7.

#### Step 7: induce one controlled unsafe state

The unsafe state is a support workaround: "labuser2 needs to see the quarterly summary, so widen the group". Write two predictions before running anything: after changing only the file, can labuser2 read it; after also changing the directory, what can labuser2 do (read, append, create, delete)?

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| G7.1 | `sudo chgrp labusers /srv/g9toy/reports/q3_summary.txt` then `sudo chmod 0660 /srv/g9toy/reports/q3_summary.txt` | G9-TGT-01 | step 7a: widen the file only | no output | changes the file's group and mode; its ctime changes | sudo in the guest | the one file | `sudo chgrp labreaders` and `sudo chmod 0640` on the file |
| G7.2 | `sudo -u labuser2 -- cat /srv/g9toy/reports/q3_summary.txt` | G9-TGT-01 | tests whether step 7a exposed anything | still `Permission denied`: the parent directory refuses search to this account, so a wider file mode does not matter | none | sudo in the guest | the guest | none needed (read-only) |
| G7.3 | `sudo chgrp labusers /srv/g9toy/reports` then `sudo chmod 2770 /srv/g9toy/reports` | G9-TGT-01 | step 7b: widen the directory too | no output | changes the directory's group and mode | sudo in the guest | the one directory | `sudo chgrp labreaders /srv/g9toy/reports` and `sudo chmod 2750 /srv/g9toy/reports` |
| G7.4 | `stat -c '%A %U:%G %y %z %n' /srv/g9toy/reports /srv/g9toy/reports/q3_summary.txt /srv/g9toy/reports/q3_detail.txt` | G9-TGT-01 | the post-state, with modification and status-change times | modes and groups as in the sample below; the status-change time of the two widened paths is the moment of G7.1 and G7.3 | none | user | the guest | none needed (read-only) |

Sample (synthetic): the post-state. Not an observation.

```text
drwxrws--- svclab:labusers /srv/g9toy/reports
-rw-rw---- svclab:labusers /srv/g9toy/reports/q3_summary.txt
-rw-r----- svclab:labreaders /srv/g9toy/reports/q3_detail.txt
```

Checkpoint: pre-state and post-state saved side by side; your two predictions marked right or wrong with the reason.

#### Step 8: record the unauthorized test access

All four tests use only the lab's synthetic data. Predict each outcome first. The point is to see which operation needs which bit, and what the logs do and do not record.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| G8.1 | `sudo -u labuser2 -- cat /srv/g9toy/reports/q3_summary.txt` | G9-TGT-01 | read test | the report text: read now succeeds | none | sudo in the guest | the guest | none needed (read-only) |
| G8.2 | `sudo -u labuser2 -- sh -c 'printf "synthetic-marker-labuser2\n" >> /srv/g9toy/reports/q3_summary.txt'` | G9-TGT-01 | write test with a marker line | no output: the append succeeds | the report now holds one extra line; its mtime changes | sudo in the guest | the one report | step 9 reinstalls the report from the medium |
| G8.3 | `g9q 'GET q3_summary.txt'` | G9-TGT-01 | what the service now serves to everyone | `OK 164` and the report with the marker line: an integrity violation reaches the authorized consumer | one logged request | user | `127.0.0.1:8080` only | none needed |
| G8.4 | `sudo -u labuser2 -- sh -c 'printf "synthetic\n" > /srv/g9toy/reports/q3_probe_labuser2.txt'` | G9-TGT-01 | create test | no output: creating succeeds | one new file owned by `labuser2`; its group is `labusers` because the directory has the setgid bit | sudo in the guest | the reports directory | step 9 removes the file |
| G8.5 | `sudo -u labuser2 -- rm /srv/g9toy/reports/q3_canary.txt` | G9-TGT-01 | delete test on a file that labuser2 cannot even read | no output: the delete succeeds, because removing a name needs write and search on the directory, not any right on the file | the canary is gone | sudo in the guest | the canary only | step 9 recreates it as in G6.7 |
| G8.6 | `stat -c '%A %U:%G %y %z %n' /srv/g9toy/reports/q3_summary.txt` then `sudo journalctl -u g9toy.service --since '<time of G8.1>' --no-pager -o short-iso` | G9-TGT-01 | what evidence the system holds of those four operations | the file's times and size changed; the journal holds the service's `GET` and `LIST` lines but no line saying who edited, created or deleted a file | none | sudo in the guest | the guest | none needed (read-only) |

Observe and write down: the same group-write bit let labuser2 modify a report that the service then served; delete was possible without any right to the file; and the telemetry gap is real: ordinary logs recorded the service's requests and not the file operations. A control that would close the gap is an audit watch on the directory (PROPOSED, not applied: `auditd` and its rule `-w /srv/g9toy/reports -p wa -k g9toy_reports`; the package is not assumed installed and nothing is installed here). Say what you can and cannot infer about who changed the file from the file's times and group alone: nothing about the actor; only the time of a change of mode or group (ctime) and of content (mtime).

Checkpoint: four test results with outputs, the service's tampered reply, the time evidence, and the stated gap.

#### Step 9: remediate, then confirm that permitted operations still work

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| G9.1 | `sudo chgrp labreaders /srv/g9toy/reports` then `sudo chmod 2750 /srv/g9toy/reports` | G9-TGT-01 | restores the directory to the narrow group with no access for others | no output | changes group and mode back | sudo in the guest | the one directory | the commands of G7.3 (only to repeat the exercise) |
| G9.2 | `sudo install -m 0640 -o svclab -g labreaders /mnt/g9fx/guided/reports/q3_summary.txt /srv/g9toy/reports/q3_summary.txt` | G9-TGT-01 | restores the report's content, owner, group and mode from the read-only medium | no output | overwrites the tampered report | sudo in the guest | the one report | none: the tampered copy was saved as evidence first |
| G9.3 | `sudo rm /srv/g9toy/reports/q3_probe_labuser2.txt` then `sudo install -m 0640 -o svclab -g labreaders /dev/null /srv/g9toy/reports/q3_canary.txt` then `sudo sh -c 'cat /tmp/w01-canary.txt > /srv/g9toy/reports/q3_canary.txt'` | G9-TGT-01 | removes the probe and recreates the canary | no output | one file removed, one created | sudo in the guest | the reports directory | as G6.7 |
| G9.4 | `sha256sum /srv/g9toy/reports/q3_summary.txt /mnt/g9fx/guided/reports/q3_summary.txt` | G9-TGT-01 | proves the restored content equals the fixture | two lines with the same hash; compare it with the SHA-256 of `guided/reports/q3_summary.txt` in `MANIFEST.md` | none | user | the guest | none needed (read-only) |
| G9.5 | `sudo chown root:root /srv/g9toy/reports/q3_detail.txt` then `sudo chmod 0600 /srv/g9toy/reports/q3_detail.txt` then `g9q 'GET q3_detail.txt'` | G9-TGT-01 | an over-tight correction, tried on the second report only: predict first | `ERR 13 EACCES`: the service identity can no longer read it, and neither can labuser1 | breaks one report's service | sudo in the guest | the one report | G9.6 |
| G9.6 | `sudo chown svclab:labreaders /srv/g9toy/reports/q3_detail.txt` then `sudo chmod 0640 /srv/g9toy/reports/q3_detail.txt` | G9-TGT-01 | repairs the over-tight state to the least privilege that works | no output | restores owner, group, mode | sudo in the guest | the one report | none |
| G9.7 | `stat -c '%A %U:%G %n' /srv/g9toy /srv/g9toy/reports /srv/g9toy/reports/q3_summary.txt /srv/g9toy/reports/q3_detail.txt /srv/g9toy/reports/q3_canary.txt` | G9-TGT-01 | the post-fix state | equal to the baseline listing of G6.13 | none | user | the guest | none needed (read-only) |

Positive retests (the permitted operations):

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| G9.8 | `sudo -u labuser1 -- cat /srv/g9toy/reports/q3_summary.txt` | G9-TGT-01 | the authorized reader still reads | the original four lines | none | sudo in the guest | the guest | none needed (read-only) |
| G9.9 | `g9q 'LIST'` then `g9q 'GET q3_summary.txt'` then `g9q 'GET q3_detail.txt'` then `g9q 'COUNT'` | G9-TGT-01 | the service still serves and still writes its state | `OK 3` with three names; `OK 138`; `OK 178`; the counter one higher than before (each reply is followed by `OK bye`) | log lines; the counter file changes | user | `127.0.0.1:8080` only | none needed |

Negative retests: the same four tests as step 8, with one controlled variable changed (the fix) and nothing else.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| G9.10 | `sudo -u labuser2 -- cat /srv/g9toy/reports/q3_summary.txt` | G9-TGT-01 | read is refused again | `Permission denied` | none | sudo in the guest | the guest | none needed (read-only) |
| G9.11 | `sudo -u labuser2 -- sh -c 'printf "synthetic-marker-labuser2\n" >> /srv/g9toy/reports/q3_summary.txt'` | G9-TGT-01 | write is refused again | a shell message with `Permission denied`; the report's hash is unchanged | none if the fix holds; if it does not, the report is tampered again | sudo in the guest | the one report | G9.2 |
| G9.12 | `sudo -u labuser2 -- sh -c 'printf "synthetic\n" > /srv/g9toy/reports/q3_probe_labuser2.txt'` | G9-TGT-01 | create is refused again | `Permission denied`; no file appears | none if the fix holds | sudo in the guest | the reports directory | G9.3 |
| G9.13 | `sudo -u labuser2 -- rm /srv/g9toy/reports/q3_canary.txt` | G9-TGT-01 | delete is refused again | `Permission denied`; the canary still exists | none if the fix holds | sudo in the guest | the canary | G9.3 |
| G9.14 | `namei -l /srv/g9toy/reports/q3_summary.txt` | G9-TGT-01 | a second, independent observation: the path walk shows the cause | the directory line shows group `labreaders` and no bits for others | none | user | the guest | none needed (read-only) |

Checkpoint: a table with, for each of the four tests, the result before and after the fix, the two retest classes, and the over-tight detour with the reason it passes the negative tests and fails the positive one. State the least privilege in one sentence: who may do what, through which group, and why no wider setting is needed.

#### Step 9c (optional, stack B only): the same service across a permitted zone boundary

Only with the router, exception E1 open for one window, tests T8, T6, T7, T5 and T1 PASS for the stack. Run test T1 of `docs/curriculum/v2/LAB_ARCHITECTURE.md` section 11.2 using the toy service as the permitted listener on `10.77.20.20` TCP 8080 instead of its `http.server` step: set `bind = 10.77.20.20` in `/etc/g9toy/g9toy.conf`, restart the unit, request `LIST` from `G9-ATK-01` with `bash -c 'exec 3<>/dev/tcp/10.77.20.20/8080; printf "LIST\nQUIT\n" >&3; timeout 3 cat <&3'`, confirm port 8081 is refused, then restore `bind = 127.0.0.1`, restart, close E1, repeat T1's negative check and test T2. Record the router's pass and block lines and both captures. The service must not stay bound to a lab address after the window (N-14, STOP-11).

### Part D: restore the baseline and preserve the evidence (plan step 10), day D03 block B, after the extensions

Before anything is reverted, the evidence leaves the guest as text and is hashed. Then run test T9 of `docs/curriculum/v2/LAB_ARCHITECTURE.md` section 11.10 on `G9-TGT-01` (marker hash before, dirty state, restore by the method of section 12, marker hash after, tests T8 and T5 to T7 again). The restore commands are rows R-01 to R-06 of section 12.4 there; the guest is disposable and is restored by discarding its overlay or by copying the cold baseline back, never by deleting and rebuilding by hand.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| G10.1 | `sha256sum <evidence-folder>/w01/*` | host, evidence folder | the hash list of everything you kept, before any revert | one line per file | none | user | the evidence folder | none needed (read-only) |
| G10.2 | `date -u +%Y-%m-%dT%H:%M:%SZ` | host | writes the hash list's time next to it | one UTC line | none | user | host | none needed (read-only) |
| G10.3 | `virsh -c qemu:///system shutdown g9-tgt-01` then `virsh -c qemu:///system domstate g9-tgt-01` | host libvirt (row R-01) | graceful power-off before the restore | the state reads `shut off` | volatile state is lost | libvirt system connection | the one guest | `virsh -c qemu:///system start g9-tgt-01` |
| G10.4 | `cp --reflink=auto --sparse=always <baseline path> <active image>` (row R-06 of LAB_ARCHITECTURE section 12.4, for the powered-off `g9-tgt-01`; or discard its session overlay instead) | host, image directory | returns the guest to its baseline | the marker file hash equals the baseline's after restart | overwrites the active image: its dirty state is lost | file access | one image, after the evidence is copied | none: copy evidence first |

Checkpoint: hashes written before the restore, marker hash before and after equal, isolation re-checked, and a statement of the time the restore took. If any isolation test fails after the restore, the baseline was captured with a faulty configuration: stop (STOP-04).

### Optional extensions, day D03 blocks A and B (on the guest as step 9 left it; if the guest was reverted, repeat G6.1 to G6.14 first)

Each extension has its own prediction, evidence and cleanup. None is needed for the gate.

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| E1.1 | `sudo visudo -cf /mnt/g9fx/guided/sudoers_labreaders` | G9-TGT-01 | checks the syntax of a delegation file before it is installed | `parsed OK` for the file | none | sudo in the guest | the file | none needed (read-only) |
| E1.2 | `sudo install -m 0440 -o root -g root /mnt/g9fx/guided/sudoers_labreaders /etc/sudoers.d/40-labreaders` then `sudo -l -U labuser1` | G9-TGT-01 | installs the rule; lists what the rule allows | a listing: `(root) NOPASSWD:` and the command with its argument pattern | adds a sudo rule | sudo in the guest | the guest | `sudo rm /etc/sudoers.d/40-labreaders` |
| E1.3 | `sudo -u labuser1 -- sudo -n /usr/bin/cat /srv/g9toy/reports/q3_summary.txt` then `sudo -u labuser1 -- sudo -n /usr/bin/cat /srv/g9toy/reports/q3_summary.txt /srv/g9toy/private/ledger.txt` | G9-TGT-01 | the intended use, then an argument string that the pattern also matches | the report; then the report followed by the root-only synthetic ledger: the wildcard matched an extra path | none (reads two synthetic files) | sudo in the guest | the two files | none needed (read-only) |
| E1.4 | `sudo rm /etc/sudoers.d/40-labreaders` then `sudo -u labuser1 -- sudo -n /usr/bin/cat /srv/g9toy/private/ledger.txt; echo "exit=$?"` | G9-TGT-01 | removes the rule; labuser1 already reads the reports through group membership, so the delegation was never needed | a non-zero exit status; the message wording depends on the sudo version: record it | removes the rule | sudo in the guest | the guest | repeat E1.2 |
| E2.1 | `systemd-analyze security g9toy.service` | G9-TGT-01 | the exposure score of the unit as installed | a table of settings and an overall exposure value; the value varies with the systemd version, so it is only compared with the later run | none | user | the guest | none needed (read-only) |
| E2.2 | `sudo install -d -m 0755 /etc/systemd/system/g9toy.service.d` then `sudo install -m 0644 /mnt/g9fx/guided/dropin_sandbox.conf /etc/systemd/system/g9toy.service.d/90-sandbox.conf` then `sudo systemctl daemon-reload` then `sudo systemctl restart g9toy.service` | G9-TGT-01 | applies sandbox directives as a drop-in | no output; the service restarts | the service runs with a read-only file system view except `/var/lib/g9toy` | sudo in the guest | the unit | `sudo rm -r /etc/systemd/system/g9toy.service.d` then `sudo systemctl daemon-reload` and restart |
| E2.3 | `g9q 'WHOAMI'` then `g9q 'COUNT'` then `g9q 'SELFTEST'` then `systemd-analyze security g9toy.service` | G9-TGT-01 | the sandbox's effect | `NoNewPrivs=1` and `CapEff=0000000000000000`; the counter still increments; `SELFTEST` returns `ERR 30 EROFS`, a refusal that comes from the mount namespace and not from a mode (the spool directory is owned by the service account, so the mode would have allowed it); a lower exposure value | log lines | user | `127.0.0.1:8080` | none needed |
| E2.4 | `sudo install -m 0644 /mnt/g9fx/guided/dropin_run_as_root.conf /etc/systemd/system/g9toy.service.d/10-run-as-root.conf` then `sudo systemctl daemon-reload` then `sudo systemctl restart g9toy.service` then `g9q 'WHOAMI'` | G9-TGT-01 | the excess-privilege variant: the same service as root | `Uid=0,0,0,0`; the effective capability set is the sandbox's (empty bounding set) or full without it: compare both runs | systemd may change the owner of the state directory when `User=` changes (ASSUMED; check with `ls -ld /var/lib/g9toy`) | sudo in the guest | the unit | `sudo rm /etc/systemd/system/g9toy.service.d/10-run-as-root.conf`, reload, restart, and restore the state directory's owner if it changed |
| E3.1 | edit `/etc/systemd/system/g9toy.service.d/90-sandbox.conf` so that `ReadWritePaths=` names `/var/lib/g9toy-state`, then `sudo systemctl daemon-reload` then `sudo systemctl restart g9toy.service` then `systemctl status g9toy.service --no-pager` then `sudo journalctl -u g9toy.service -n 20 --no-pager -o short-iso` | G9-TGT-01 | a controlled start failure: read the cause in the journal before repairing it; compare with `d03/journal_unit_sample.log` | the unit fails; the journal names a mount-namespace failure and an exit status of the form `226/NAMESPACE` | the service is down until repaired | sudo in the guest | the unit | restore the path to `/var/lib/g9toy`, reload, restart |
| E4.1 | `cat /sys/module/apparmor/parameters/enabled` then `cat /proc/self/attr/current` then `sudo aa-status` | G9-TGT-01 | a read-only view of mandatory access control on the guest | `Y` or `N`; `unconfined` for an unconfined shell; a count of loaded profiles and their modes | none | user, then sudo in the guest | the guest | none needed (read-only) |
| E5.1 | `grep -e '^Uid:' -e '^Gid:' -e '^Groups:' -e '^CapEff:' -e '^NoNewPrivs:' /proc/self/status` | G9-TGT-01 | the credentials and capability mask your own shell holds, as the kernel records them | the real, effective, saved and filesystem IDs; an empty or zero effective mask for an ordinary account | none | user | the guest | none needed (read-only) |

## Positive validation

The lab's positive claim holds only if every item below is observed and recorded with its time, not predicted.

| Id | What must be observed | Where it comes from |
|---|---|---|
| PV1 | the authorized reader `labuser1` reads the reports directly after the fix | G6.15 before, G9.8 after |
| PV2 | the service lists and serves both reports and increments its counter after the fix | G6.17 before, G9.9 after |
| PV3 | the restored report's hash equals the fixture's hash in `MANIFEST.md` | G9.4 |
| PV4 | the service still runs as `svclab`, listens on `127.0.0.1:8080` only, is active and not enabled | G6.10 to G6.12 repeated after step 9 |
| PV5 | the post-fix listing equals the baseline listing line for line | G9.7 against G6.13 |
| PV6 | for the isolation part: each of T8, T6, T7 and T5 has a recorded result with evidence that includes a stimulus, not an empty capture | step 5 |

## Negative and safety validation

A success that shows only the positive items is apparent success. The negative items separate it from verified success.

| Id | What must fail or stay absent | Where it comes from |
|---|---|---|
| NV1 | `labuser2` is refused on read, write, create and delete after the fix, each in the same form as in step 8 | G9.10 to G9.13 |
| NV2 | the over-tight correction fails the positive test: a fix that passes only NV1 is rejected | G9.5, G9.9 |
| NV3 | the file-only change of step 7a exposed nothing: you explained why from the directory's search permission | G7.2 |
| NV4 | no guest has a route to the household LAN or the Internet, no extra NIC exists, no global IPv6 address exists | tests T8, T6, T7, T5 |
| NV5 | no listener other than `127.0.0.1:8080` exists in the guest after any step, and any temporary exception is removed with its retest | `ss -ltn`; step 9c cleanup |
| NV6 | no command outside the approved text was run, no path outside the lab's tree was changed | the command log against the approval record |
| NV7 | the isolation claim never rests on a network name, a switch name or a missing route alone | the evidence table of step 5 |

## Evidence

The plan's evidence contract for week 1 is: modes, identities, logs, and pre-state and post-state permissions. In files, kept in `<evidence-folder>/w01/` and hashed (`docs/curriculum/v2/WEEKS/W01/EVIDENCE_CHECKLIST.md` gives the pass or fail test of each):

| Artifact | Content |
|---|---|
| scope and RoE | the scope lines, the signed RoE record id, the time record of step 1 |
| Stage 00 record | one evidence record per row run (command as typed, user, time UTC and local, exit status, output, interpretation labelled INFERRED, what is not established), the leaf, the unresolved blocker |
| resource table | step 2's table with sources |
| isolation record | per test: id, run id, times, stack and baseline ids, commands, outputs, capture hashes, result |
| identities | `id` of each account and `systemctl show` of the unit |
| modes | the baseline, post-unsafe and post-fix listings of steps 6, 7 and 9 and the `namei -l` outputs |
| access matrix | each test of steps 8 and 9 with the account, the action, the result before and after |
| logs | the journal excerpts of steps 6 and 8, with the gap you found named |
| lab record | the template `docs/curriculum/v2/TEMPLATES/LAB_RECORD.md` filled, status set last; BLOCKED if the lab was blocked |
| hash list | SHA-256 of every file and of the bundle, with the time, written before any key is opened |

## Rollback and cleanup

Preferred: restore the guest to its baseline (step 10). If the baseline is not used, undo in this order and prove each with a command whose output goes in the record:

| # | Command | Target | Purpose | Expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|---|
| C1 | `sudo systemctl stop g9toy.service` then `sudo rm -r /etc/systemd/system/g9toy.service.d` then `sudo rm /etc/systemd/system/g9toy.service /etc/sudoers.d/40-labreaders` then `sudo systemctl daemon-reload` | G9-TGT-01 | removes the unit, its drop-ins and the delegation rule | no output; `rm` reports a missing file for anything not created | the service is gone | sudo in the guest | this lab's unit only | reinstall from the medium |
| C2 | `sudo rm -r /opt/g9toy /etc/g9toy /srv/g9toy /var/lib/g9toy /tmp/w01-canary.txt` | G9-TGT-01 | removes the lab's files | no output | deletes only this lab's tree | sudo in the guest | the listed paths | reinstall from the medium |
| C3 | `sudo userdel --remove labuser1` then `sudo userdel --remove labuser2` then `sudo userdel svclab` then `sudo groupdel labreaders` then `sudo groupdel labusers` | G9-TGT-01 | removes the synthetic accounts and groups | no output | removes accounts and home directories of the two synthetic users | sudo in the guest | the five names | recreate by G6.1 to G6.3 |
| C4 | `sudo rm /etc/netplan/60-g9-lab.yaml /etc/sysctl.d/60-g9-lab.conf` then `sudo netplan apply` | each guest | removes the static address and the sysctl file only if the baseline is to return to its pre-lab network state | no output | the guest loses its static address until re-applied | sudo in the guest | the two files | G5.2 and G5.5 |
| C5 | `ss -ltn` and `getent passwd labuser1 labuser2 svclab` | G9-TGT-01 | proves the cleanup: no listener on 8080, no synthetic account | no `8080` line; the second command prints nothing and exits with status 2 | none | user | the guest | none needed (read-only) |

Then shut down both guests (`virsh -c qemu:///system shutdown <domain>`, row R-01), confirm with `virsh -c qemu:///system list --all` that they are `shut off`, and re-run the tests that the change could affect plus T8 (retest triggers of LAB_ARCHITECTURE section 11.1). Variant H0: after copying the evidence, `rm -r <evidence-folder>/w01-toy` and prove it with `ls <evidence-folder>/w01-toy`, which must fail with `No such file or directory`. A cleanup step that cannot be shown stops the exercise (STOP-11).
