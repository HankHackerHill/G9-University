# G9 Cybersecurity Mastery v2.0: Stage 00 host readiness

- Status: PROPOSED. Stage 00 is BLOCKED (plan section 14.2, item 1).
- Written: 2026-11-18
- Source: `docs/curriculum/v2/MASTER_PLAN_V2.0.md`, section 2 (host readiness), section 3.2 (isolation), section 5 week 1 (precondition), section 14.2 item 1, section 16 items 9 and 10; `docs/adr/ADR-0023-a-second-curriculum-shape-the-v2-plan-of-60-weekdays.md`
- Rules of approval: `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` (classes B and C of section 2.2)
- Pointer for version-sensitive facts (product behaviour, packaging, documentation pages): `docs/curriculum/v2/RESOURCE_REGISTRY.md`, generated later from the week registries. Facts about libvirt packaging and behaviour below are ASSUMED unless a registry entry is named
- Labels: PROPOSED, ASSUMED, INFERRED, VERIFIED (only with evidence in the repository), NOT RUN. SEARCH_CHECKED means a search result of 2026-11-18 recorded in a week registry; the page was not opened.

> **Nothing here was executed.** No command in this file was run by the author of the file, on the G9 host or anywhere else. The host observations of 2026-11-14 are the owner's own, taken from the plan; their raw output is not in this repository, so no host fact below is VERIFIED here. Every command is proposed text. This file approves none, and writing it authorizes no command in it.

## 1. State of Stage 00 on 2026-11-18

| Item | State |
|---|---|
| Stage 00 | BLOCKED: the cause of the failed libvirt connection is not established by recorded evidence |
| Diagnostics run by the author | none (NOT RUN) |
| Diagnostics approved by the owner | none recorded in the repository |
| Age of the observations | taken 2026-11-14, four days before this file; the host may have changed since (a reboot, an update, a new Docker network) |
| Local lab | does not exist; no guest, virtual network, router, domain or SIEM has been built; isolation is not proven |

Three Stage 00 status values are proposed, so that progress can be stated without implying a lab:

| Status | Meaning | Does not mean |
|---|---|---|
| BLOCKED | the root cause of the libvirt blocker is not established by recorded evidence | that libvirt is broken |
| EVIDENCE-COMPLETE | every row the decision tree (section 7) selected was approved, run and recorded, and the cause of the blocker is named from that evidence | that anything works |
| READY-FOR-APPROVAL | every precondition of section 8 is met on evidence; the owner may then approve building the lab, one command at a time, under class C records | LAB TESTED. That state needs the isolation acceptance tests of `docs/curriculum/v2/LAB_ARCHITECTURE.md` (section 11) to pass and be recorded |

## 2. Owner-supplied observations of 2026-11-14

The status column is the plan's own label, unchanged (plan section 2.1). In the plan, "Observed" means the owner pasted the output of that command; it does not mean this repository holds the output.

| Category | Observation | Status (plan's label) | What it does not establish |
|---|---|---|---|
| Mainboard | Gigabyte X870E AORUS Master X3D ICE | User supplied | any firmware setting (SVM mode, IOMMU) |
| CPU | AMD Ryzen 7 9800X3D, 8 cores / 16 logical CPUs | Observed `lscpu` | that 16 guest vCPUs are 16 cores: they are 16 SMT threads on 8 cores |
| Virtualization support | `Virtualization: AMD-V` | CPU capability; KVM acceleration **not yet verified** | a usable `/dev/kvm`, loaded KVM modules, or a firmware setting that leaves SVM usable |
| RAM | `60 GiB total`, `55 GiB available` during diagnostic | Adequate for lab if current | memory under later load; the plan's "nominal 64 GiB" is a different figure (section 10) |
| Swap | 8 GiB | Observed | swap is not a lab resource; no guest is planned to use it |
| NVMe/root filesystem | 1.9 TiB filesystem, 108 GiB used, ~1.7 TiB available | Adequate at observation time | the filesystem type, where VM images will live, encryption, drive health |
| Docker | Docker Engine Community client/server v29.9.3, functioning daemon | Observed; version may change | the firewall rules and forwarding Docker installed; the owner's group membership |
| GPU | NVIDIA RTX 5080 | User supplied; no passthrough required | not relevant to the lab |
| `virsh` | Failed to connect `/var/run/libvirt/libvirt-sock`; socket absent | **BLOCKER: libvirt not operational from tested client connection** | which command line, user or connection URI produced the message; whether a daemon is installed |
| Default IPv4 routes | Proton-associated interface preferred by metric; physical gateway also shown | **Not proof of kill switch or leak** | policy routes, firewall rules, behaviour when the VPN drops |
| Docker bridges | `172.17.0.0/16`, `172.18.0.0/16`, `172.19.0.0/16` | Observed; check full routing state before new lab | other bridges, libvirt networks, VPN ranges, policy routes |
| Host LAN | `192.168.1.0/24` during diagnostic | Avoid routing vulnerable guests onto it | the host's other addresses |
| IPv6 route display | No lines returned by `ip -6 route` | Not sufficient to establish IPv6 disabled everywhere | per-interface state, policy rules, firewall |

Not in the plan's table: the host's operating system. `README.md` (target environment) names an Ubuntu 26.04.1 LTS based G9 OS and labels it ASSUMED until inspected. The repository holds no record of the libvirt or QEMU package set.

## 3. The blocker and the no-action rule

The plan's rule, in its meaning: the missing libvirt socket is an unresolved root-cause investigation, not permission to install or restart service packages. First determine whether the host uses `virtqemud`, `libvirtd`, socket activation, or another connection URI.

### 3.1 What is not done until evidence and an approval exist

| Not done | Includes |
|---|---|
| installing, removing or upgrading any package | `apt`, `dpkg -i`, snap, any download |
| changing service state | `systemctl start`, `stop`, `restart`, `enable`, `disable`, `mask`, `unmask`, `daemon-reload` |
| changing kernel or firmware state | `modprobe`, `rmmod`, editing module options, changing UEFI settings |
| editing configuration | anything under `/etc`, the user's `~/.config`, unit drop-ins |
| changing privilege | `usermod`, `gpasswd`, `visudo`, ACL changes on `/dev/kvm` |
| changing the network | routes, rules, firewall, sysctl, the VPN client, Docker networks |
| connecting to a libvirt daemon | class C probes (section 5.3): a connection can start a socket-activated daemon |

### 3.2 What the observation says and what it does not

The observation is the client error that `/var/run/libvirt/libvirt-sock` was not found.

| Reading | Basis |
|---|---|
| The client tried the legacy (monolithic) system socket path, and nothing existed there at that moment | the message itself |
| `/var/run` is normally a link to `/run`, so the path is `/run/libvirt/libvirt-sock` | ASSUMED; row F-12 checks it |
| `virsh` exists on the host, so some libvirt client software is installed | the observation; the package set is not known |
| The modular daemons use per-driver sockets, for example `/run/libvirt/virtqemud-sock`, and are socket-activated by systemd; an absent legacy socket is not proof that QEMU cannot work | libvirt daemons guide; registry `w01-libvirt-daemons`, SEARCH_CHECKED 2026-11-18 (`docs/curriculum/v2/WEEKS/W01/RESOURCES.json`, `plan_fact_checks`); plan section 16 item 9 |
| It does not say which user, which command line (with or without `sudo`, with or without `-c`) or which URI produced the message | the plan quotes only the path |

Evidence gap G1: the exact command line, the user, the full text of the error and any environment override at the time are not recorded in the repository. The first step is not a command: the owner writes them down. They decide which hypothesis in the next table is likely.

### 3.3 Hypotheses for the missing socket

| ID | Hypothesis | Evidence that supports it | Evidence that excludes it | Rows |
|---|---|---|---|---|
| H1 | libvirt daemon packages are not installed; only client tools are | S-05 and F-08 list no unit files; F-14 shows no daemon package | any libvirt unit file exists | S-05, F-08, F-14 |
| H2 | the monolithic `libvirtd` is installed but not running: its socket or service is disabled, inactive, failed or masked | unit files exist for `libvirtd*`; F-10 shows `inactive`, `failed` or `masked`; no socket in `/run/libvirt` | the socket unit is active and its path exists | S-05, F-08, F-09, F-10, F-11 |
| H3 | the modular layout (`virtqemud` and the other per-driver daemons) is installed; the legacy path is not provided; the client or the URI used the legacy path | unit files for `virtqemud*`; `/run/libvirt/virtqemud-sock` present or its socket unit loaded | no modular unit exists | S-05, F-08, F-09, F-11, F-15 |
| H4 | the daemon listens in a non-default directory or with other permissions, set in its configuration | a socket unit active but its path differs; `unix_sock*` settings in F-21 | the default path is in use | F-09, F-11, F-21 |
| H5 | the intended connection is the per-user session (`qemu:///session`); no system socket is expected | an override in F-19 or F-20 names a session URI | the owner ran a system connection | F-19, F-20 |
| H6 | the command ran where `/run` is not the host's: a container, toolbox, confined package or chroot | F-02 shows a container or hypervisor; `/run` differs inside the tool | the command ran in a plain host shell (stated by the owner) | F-02, F-12 |
| H7 | the state changed after 2026-11-14 (reboot, service stop, update); the observation is stale | F-31 shows a boot time after 2026-11-14; the same rows now give a different picture | nothing changed | F-31, all |

## 4. Diagnostic classes, approval protocol and the evidence record

| Class | Definition | Approval |
|---|---|---|
| A | reads local state and writes nothing outside its own output; no elevated privilege | the owner reviews the exact command (SAFETY_AUTHORIZATION class B) |
| B | reads local state; the system asks for elevated privilege (`sudo`) or a privileged group | same; the privilege is part of what the owner approves |
| C | a connection to a libvirt daemon. Not read-only: connecting to a socket-activated daemon makes systemd start it, and a per-user connection can start a per-user daemon and create files in the owner's home. A read-only connection option still activates the daemon | separate approval, after rows F-08 to F-11 show which unit would start (SAFETY_AUTHORIZATION class C, because a service starts) |

These diagnostic classes are local to this file. They map to the action classes of `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` as the Approval column says.

Protocol:
1. The decision tree (section 7) names the next row from the evidence returned so far.
2. The owner reviews the exact text, its target, privilege and side effects, and decides.
3. The approval is written before the run (`docs/curriculum/v2/TEMPLATES/CHANGE_APPROVAL.md` fields; for class A and B the Stage 00 record below is enough).
4. The owner runs the command, or states the approval for it, one command at a time.
5. The output is saved verbatim. The interpretation is written separately and labelled INFERRED.
6. A result is OBSERVED once saved with its time. Nothing is edited afterwards; a correction is a new entry.
7. The tree stops at a leaf. A leaf never leads to an installation or a change; it leads to a decision of the owner outside this file.

Stage 00 evidence record (one per run):

```text
Row id:            <S-nn / F-nn / C-nn>
Command as typed:  <exact text>
Run by / privilege: <user name> / <user or sudo>
Time:              <UTC, ISO 8601>   <local with offset and zone>
Exit status:       <number>
Output (stdout):   <verbatim, saved as a file; its SHA-256>
Output (stderr):   <verbatim>
Interpretation:    INFERRED: <one or two sentences>
Not established:   <what this result does not show>
Next row and why:  <row id, reason from the tree>
```

Redaction: output of the routing, firewall and Docker rows can contain VPN endpoint addresses, user names and host names. Keep the records private; redact before any record leaves the private evidence folder (`docs/curriculum/v2/PORTFOLIO/README.md`).

## 5. Proposed diagnostics

All rows are proposed text. Rollback "none needed (read-only)" means the command changes nothing; the exceptions are stated in the Side effects column. "Safe scope" is the G9 host unless stated.

### 5.1 The plan's nine read-only rows (plan section 2.2)

| ID | Command | Target and what it reads | Expected output and meaning (the plan's expectation) | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| S-01 | `lscpu` | the CPU description (from `/proc/cpuinfo` and sysfs) | an x86-64 AMD processor, 16 logical CPUs, 2 threads per core, a `Virtualization:` line; a `Hypervisor vendor:` line appears only if the host is itself virtualized | none | user | the host | none needed (read-only) |
| S-02 | `free -h` | memory and swap accounting (`/proc/meminfo`) | total, used, `available` (plan with this figure) and swap, in binary units | none | user | the host | none needed (read-only) |
| S-03 | `df -h /` | capacity and free space of the root filesystem | size, used, available, mount point. It is the lab's figure only if VM images will live on `/` | none | user | the host | none needed (read-only) |
| S-04 | `lsblk -o NAME,SIZE,FSTYPE,MOUNTPOINTS` | block devices, sizes, filesystem types, mount points | the partition layout and filesystem types; not proof of drive health. Older util-linux rejects the column `MOUNTPOINTS`; the substitute `MOUNTPOINT` is a new command needing its own review | none | user | the host | none needed (read-only) |
| S-05 | `systemctl list-unit-files 'libvirtd*' 'virtqemud*' 'virtproxyd*' --no-pager` | the unit files on disk whose names match the three globs | a table of unit names with their enablement state, or `0 unit files listed.` | none | user | the host | none needed (read-only) |
| S-06 | `ls -l /dev/kvm` | the KVM device node | a character device, normally owned by root with group `kvm`; or `No such file or directory`. Check ownership and permissions | none | user | the host | none needed (read-only) |
| S-07 | `ip -4 route` | the IPv4 main routing table | default routes with metrics and the connected subnets, including Docker bridges. It does not show policy routes | none | user | the host | none needed (read-only) |
| S-08 | `ip -6 route` | the IPv6 main routing table | possibly no lines | none | user | the host | none needed (read-only) |
| S-09 | (removed) | removed 2026-11-20 on the owner's instruction: the container engine is not used and no command here calls it; the id stays so that references remain valid | | | | | |

### 5.2 Follow-up rows, chosen from returned evidence only

The plan requires follow-up diagnostics to be selected from the returned evidence, not shot in bulk. Each row below is named by the decision tree. All are class A unless the Privilege column says otherwise.

| ID | Command | Target and what it reads | Expected output and meaning | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| F-01 | `cat /etc/os-release` | the distribution and release | name and version; decides package and unit naming (Debian and Ubuntu naming is ASSUMED throughout) | none | user | the host | none needed (read-only) |
| F-02 | `systemd-detect-virt` | whether the host runs under a hypervisor or in a container | prints `none` on bare metal (exit status 1); otherwise a type | none | user | the host | none needed (read-only) |
| F-03 | `grep -c -w svm /proc/cpuinfo` | the count of logical CPUs reporting the `svm` flag | a count; 16 if every thread reports it; 0 if the flag is not exposed | none | user | the host | none needed (read-only) |
| F-04 | `grep -e '^kvm ' -e '^kvm_amd ' -e '^vhost_net ' -e '^tun ' -e '^bridge ' /proc/modules` | the loaded kernel modules of interest | one line per loaded module; no output means none is loaded (a module may be built in or loadable) | none | user | the host | none needed (read-only) |
| F-05 | `getfacl /dev/kvm` | the access list of the KVM node (the tool is in the `acl` package, ASSUMED) | owner, group and any named user entries; an error if the node is absent | none | user | the host | none needed (read-only) |
| F-06 | `journalctl -k -b -g svm --no-pager` and `journalctl -k -b -g kvm --no-pager` | kernel messages of this boot that mention `svm` or `kvm` | a message that SVM or KVM was disabled by firmware, or lines showing KVM initialized; reading the kernel journal may need group `adm` or `systemd-journal`, or `sudo` (then class B) | none | user, or sudo (class B) | the host | none needed (read-only) |
| F-07 | `virt-host-validate qemu` | libvirt's host check: hardware virtualization, `/dev/kvm` and its access for the invoking user, vhost-net, tun, control groups, IOMMU. Present only if the tool is installed (it ships with the libvirt client tools, ASSUMED). The tool is not documented here; read its manual first | one PASS, WARN or FAIL line per check. An IOMMU warning is irrelevant: no passthrough is planned | none expected (ASSUMED to read `/proc`, `/sys` and `/dev` only and not to contact a daemon) | user | the host | none needed (read-only) |
| F-08 | `systemctl list-unit-files 'virt*' 'libvirt*' --no-pager` | every libvirt-related unit file: daemons, sockets, `virtlogd`, `virtlockd`, `libvirt-guests` | shows which layout exists; a modular set includes `virtnetworkd`, `virtstoraged`, `virtnodedevd` and `virtqemud` | none | user | the host | none needed (read-only) |
| F-09 | `systemctl list-sockets --all --no-pager 'libvirt*' 'virt*'` | loaded socket units, their listen paths, and the service each activates | `LISTEN` paths and unit names; an empty list means no such socket unit is loaded | none | user | the host | none needed (read-only) |
| F-10 | `systemctl status libvirtd.service libvirtd.socket virtqemud.service virtqemud.socket virtproxyd.socket --no-pager` | the runtime state of these units, with the last journal lines if readable | per unit: `could not be found`, or loaded with `active`, `inactive`, `failed` or `masked`. A non-zero exit status is normal when a socket-activated service is idle | none | user; journal lines may need group `adm` or `systemd-journal` | the host | none needed (read-only) |
| F-11 | `ls -la /run/libvirt/` | the libvirt runtime directory and the socket files in it | the sockets present, with owner, group and mode; or `No such file or directory` | none | user | the host | none needed (read-only) |
| F-12 | `readlink -f /var/run` | where `/var/run` points | `/run` is expected | none | user | the host | none needed (read-only) |
| F-13 | `command -v virsh virt-host-validate qemu-system-x86_64 libvirtd virtqemud virtproxyd` | which of these programs the shell finds | a path per program found, nothing for the others. The daemons are normally in `/usr/sbin`, which may not be on a user's search path, so absence here is weak evidence; F-14 decides | none | user | the host | none needed (read-only) |
| F-14 | `dpkg-query -W -f='${Package} ${Version} ${db:Status-Abbrev}\n' 'libvirt*' 'qemu*' 'virtinst' 'ovmf' 'swtpm*' 'dnsmasq*'` | the package database (Debian and Ubuntu, ASSUMED) | `ii` lines for installed packages; a "no packages found matching" message per pattern without one | none | user | the host | none needed (read-only) |
| F-15 | `virsh --version` | the client's version | a version string; this option is client-only and does not connect (ASSUMED) | none | user | the host | none needed (read-only) |
| F-16 | `virtqemud --version` or `libvirtd --version` (the owner may skip: F-14 gives package versions) | the daemon binary's version | prints a version and exits (ASSUMED); the binary is run, though only to print | none expected | user | the host | none needed (read-only) |
| F-17 | `id` | the owner's user and groups | groups such as `kvm`, `libvirt` (Debian and Ubuntu names, ASSUMED) | none | user | the host | none needed (read-only) |
| F-18 | `getent group kvm libvirt libvirt-qemu` | the group database entries | which groups exist and who belongs; a missing group suggests the package that creates it is absent (ASSUMED) | none | user | the host | none needed (read-only) |
| F-19 | `printenv LIBVIRT_DEFAULT_URI` | the environment override of the default connection URI | the URI, or nothing with exit status 1 | none | user | the owner's shell | none needed (read-only) |
| F-20 | `ls -l ~/.config/libvirt/` and `grep -n uri_default ~/.config/libvirt/libvirt.conf` | the per-user client configuration | a file list and any `uri_default` line; `No such file or directory` is a normal answer | none | user | the owner's home | none needed (read-only) |
| F-21 | `grep -n -e unix_sock -e auth_unix -e listen_ /etc/libvirt/libvirtd.conf /etc/libvirt/virtqemud.conf` | the daemons' socket and authentication settings (commented defaults match too) | lines naming the socket directory, group, permissions and listening options; an error for a file that does not exist | none | user (the files are normally world-readable, ASSUMED) | the host | none needed (read-only) |
| F-22 | `df -hT /var/lib` | the filesystem type, size and free space where the system libvirt images would be | type (ext4, xfs, btrfs), free space. If the owner wants images elsewhere, that path is the one to check (open decision O-HR-02) | none | user | the host | none needed (read-only) |
| F-23 | `sysctl net.ipv4.ip_forward net.ipv6.conf.all.forwarding net.ipv6.conf.all.disable_ipv6 net.ipv6.conf.default.disable_ipv6 net.ipv4.conf.all.rp_filter net.ipv4.conf.all.arp_ignore` | kernel networking parameters (read, no `-w`) | the values. `net.ipv4.ip_forward = 1` is normal when Docker runs (ASSUMED) | none | user | the host | none needed (read-only) |
| F-24 | `ip rule show` and `ip -6 rule show` | the IPv4 and IPv6 policy routing rules | the rule list; WireGuard tooling often adds a fwmark rule and its own table (ASSUMED) | none | user | the host | none needed (read-only) |
| F-25 | `ip route show table all` and `ip -6 route show table all` | every routing table, IPv4 and IPv6 | all routes, including those outside the main table | none | user | the host | none needed (read-only) |
| F-26 | `ip -br addr` and `ip -br link` | every interface with state, addresses and links | brief lines; existing `virbr*` interfaces, `docker0`, `br-*`, the VPN interface and their subnets | none | user | the host | none needed (read-only) |
| F-27 | `ip route get 10.77.10.10`, `ip route get 10.77.20.20`, `ip route get 10.77.30.10` | the kernel's route choice for three lab addresses; no packet is sent | the chosen route per address; a result through the default route or a physical or VPN interface means no lab route exists now | none | user | the host | none needed (read-only) |
| F-28 | `sudo nft list ruleset` | the firewall rules in force | the tables and chains of Docker, libvirt (if present) and any VPN kill-switch rules. It may reveal VPN endpoint addresses: redact. If the firewall backend was never used the command may load a kernel module (ASSUMED; harmless, but a state change) | possible module load | root (sudo; class B) | the host | none needed (read-only); an unloaded module is not rolled back |
| F-29 | (removed) | removed 2026-11-20 on the owner's instruction: the container engine is not used and no command here calls it; the id stays so that references remain valid | | | | | |
| F-30 | (removed) | removed 2026-11-20 on the owner's instruction: the container engine is not used and no command here calls it; the id stays so that references remain valid | | | | | |
| F-31 | `uptime -s` | the time the system booted | a date and time; compare with 2026-11-14 to see whether the host restarted after the observation (H7) | none | user | the host | none needed (read-only) |

### 5.3 Class C rows: connection probes (not read-only)

Used only when F-08 to F-11 show a loaded, listening socket (so the connection will reach a daemon) or the owner explicitly accepts that a daemon will start. Each needs its own approval and a class C record.

| ID | Command | Target | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| C-01 | `virsh -c qemu:///system version` | the system libvirt instance (URI ASSUMED until Stage 00) | prints the library and hypervisor versions if the daemon answers; otherwise the error text, which is evidence | may start a socket-activated daemon; the daemon then keeps running until it idles out or is stopped | libvirt system connection: root or the group that owns the socket (ASSUMED) | one connection, no definitions | none for the files the daemon creates; the service stops by its idle timeout (ASSUMED) or by a stop that is itself a class C decision |
| C-02 | `virsh -c qemu:///session list --all` | the per-user session instance | lists domains (expected none) | may start a per-user daemon and create files under the owner's home and runtime directory | user | the owner's session only | remove the created files only after review; the daemon idles out (ASSUMED) |
| C-03 | `virsh -c qemu:///system net-list --all` | the system instance's network driver | lists defined networks (expected only any that exist; none for the lab); shows whether the network driver works | as C-01 | as C-01 | listing only | as C-01 |

## 6. Reading the results

Every reading is INFERRED. A row answers its own question only: no row proves that the lab will work.

### 6.1 CPU, KVM and `/dev/kvm`

| Result | Meaning | Does not establish | Next |
|---|---|---|---|
| S-06: `No such file or directory`, and F-04 shows neither `kvm` nor `kvm_amd` | the KVM modules are not loaded. Candidate causes: SVM disabled in firmware, a blacklisted module, a kernel without KVM, virtualization hidden by an outer hypervisor | which cause | F-03, F-06, F-02, F-07 |
| S-06: `No such file or directory`, F-04 shows `kvm` and `kvm_amd` | modules loaded but no device node: unusual; a udev or namespace issue | the cause | F-06, F-02 |
| S-06: `crw-rw----` root and group `kvm` | the device exists; users in group `kvm` can open it | that the owner is in the group | F-17, F-05 |
| S-06: a trailing `+` | an ACL exists; the user at the seat commonly gets access through it (ASSUMED) | who is in it | F-05 |
| S-06: `crw-rw-rw-` | any user can open it | acceptable use; record it | none needed |
| S-06 present, `Permission denied` when a tool opens it | the user lacks the group or ACL | | F-17, F-05 |
| F-03: 16 | the `svm` flag is exposed on all threads | that firmware leaves SVM usable: kernel messages (F-06) say | F-06 |
| F-03: 0 | not exposed: firmware has SVM off, or an outer hypervisor hides it | which | F-02; the firmware setting is the owner's decision, made offline, outside this file |
| F-06: a line that SVM or KVM was disabled by firmware | the firmware setting is the cause | | leaf L3 |
| F-07: hardware virtualization PASS and `/dev/kvm` PASS | KVM is usable at kernel level for the invoking user | that libvirt works; that a guest runs accelerated | |
| S-01: a `Hypervisor vendor:` line, or F-02 not `none` | the host runs under another hypervisor or in a container: nested virtualization is a separate question | | leaf L2 or H6 |

KVM acceleration stays "not yet verified" (the plan's label) until a guest actually runs under KVM, which needs a later, separately approved build step. `/dev/kvm` presence is necessary, not sufficient. For guests under the system libvirt instance, the guest process normally runs as a service user, so the owner's own access to `/dev/kvm` is not what matters for those guests (ASSUMED).

### 6.2 Daemon layout and unit state

Sample (synthetic): S-05 on a modular host that is only socket activated. Not an observation.

```text
UNIT FILE                 STATE    PRESET
virtproxyd.socket         disabled enabled
virtqemud-admin.socket    disabled enabled
virtqemud-ro.socket       disabled enabled
virtqemud.service         disabled enabled
virtqemud.socket          enabled  enabled
```

Sample (synthetic): S-05 on a host with no libvirt daemon units.

```text
0 unit files listed.
```

| Result | Meaning | Does not establish | Next |
|---|---|---|---|
| S-05 and F-08 both list nothing | no libvirt unit files in the systemd search path; INFERRED: daemon packages absent (H1) | that QEMU or the client tools are absent: they can be installed without a daemon | F-14, F-13 |
| `libvirtd.service` with `libvirtd.socket`, `libvirtd-ro.socket`, `libvirtd-admin.socket` (and perhaps `-tcp` and `-tls` sockets) | monolithic layout with socket activation available (naming per the libvirt daemons guide, ASSUMED) | that it is running | F-10 |
| `virtqemud.service` with `virtqemud.socket`, `-ro` and `-admin` sockets, other per-driver daemons, usually `virtproxyd*` | modular layout (H3). The qemu driver socket is `/run/libvirt/virtqemud-sock` (registry `w01-libvirt-daemons`, SEARCH_CHECKED). A legacy `libvirt-sock` is not expected unless a proxy unit provides it (ASSUMED) | that the client chooses the modular socket by itself: this depends on the libvirt version installed | F-09, F-10, F-15 |
| both families present | two layouts installed. The libvirt documentation is believed to treat running both for the same driver as unsupported (ASSUMED; read `w01-libvirt-daemons`) | which one is in use | F-10; record; change nothing |
| a `.socket` unit `enabled` and its `.service` `disabled` or `static` | the normal socket-activation pattern: the service starts on the first connection | that the socket is active now | F-09 |
| both `.socket` and `.service` `disabled` | nothing listens after boot (H2) | whether it was started by hand this boot | F-10 |
| `masked` | a deliberate block; someone chose it (H2) | why | record; unmasking is a class C decision |
| `static` | the unit has no install section and is pulled in by another unit; not the same as disabled | | F-09 |
| `indirect`, `alias`, `generated`, `enabled-runtime` | enabling it enables another unit; it is a name for another unit; systemd generated it; it is enabled only until reboot | | F-10 |

### 6.3 Sockets, connection URI and permissions

| Result | Meaning | Does not establish | Next |
|---|---|---|---|
| F-09 shows a libvirt socket with a `LISTEN` path and the unit it activates | the socket unit is loaded and listening; a connection to that path would start that service (the class C consequence) | access for the owner | F-11 |
| F-09 empty while unit files exist | the socket units exist but are not active this boot, or the daemon creates its own socket | | F-10 |
| F-10: `Unit ... could not be found.` | no such unit | | F-08 |
| F-10: `failed` | the unit started and exited; the status block holds the last journal lines if readable; the cause is in them | | owner reviews; journal access may need class B |
| F-11: `/run/libvirt` absent | no libvirt daemon created its runtime directory since boot | that nothing is installed | F-08, F-14 |
| F-11: socket files with mode `srwxrwx---`, owner root, group `libvirt` | the socket exists; access needs that group (the Debian and Ubuntu default, ASSUMED). The message `Permission denied` would be this case | | F-17, F-18 |
| F-11: a socket file exists but a connection is refused | a stale socket file with no listener, or a service that exited | | F-10 |
| F-11: a socket under another name or directory | the daemon layout or path differs from the client's assumption (H3, H4) | | F-21, F-19 |
| F-19 or F-20 returns a URI | the client does not probe; it uses that URI. A wrong or stale value explains a failure (H5) | whether it is the intended URI | |
| F-19 and F-20 return nothing | the client's own default applies. How it chooses (probing the drivers, and for an unprivileged user possibly the per-user session) is in the libvirt connection URI documentation, which is not in the registry: UNVERIFIED. Read `libvirt.org/uri.html` before relying on a default | | |
| error text "No such file or directory" for a socket path | the path does not exist (the observed case). It differs from `Permission denied` (exists, no access) and `Connection refused` (exists, no listener) | | |
| `qemu:///system` against `qemu:///session` | the system instance is privileged and shared; the lab's three host bridges need it (ASSUMED; confirm in `w01-libvirt-formatnetwork` and `w01-libvirt-daemons`). The session instance is per user and is not designed to create host bridges (ASSUMED) | | |
| the owner is not in the group that owns the socket | the system socket is closed to the owner. A member of the libvirt group can define guests with arbitrary host paths and is root-equivalent in effect (ASSUMED); granting membership is a privilege decision (open decision O-HR-03) | | leaf L11 |
| a connection works only through `sudo` | the access path is root; whether that is acceptable for the lab is the owner's decision | | |

### 6.4 Memory, disk and filesystem

| Result | Meaning | Does not establish | Next |
|---|---|---|---|
| S-02 `available` | the planning figure at that moment, to compare with the week's stack plus headroom (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 13). At the observation 55 GiB of 60 GiB | memory under later load: the browser, the application, any local model | |
| S-02 swap 8 GiB | a safety margin, not capacity. Sustained swapping during a lab is a stop signal (STOP-06) | | |
| S-03 or F-22 free space | to compare with the plan's target of about 600 GB free (plan section 3.1) and the 390 GiB thin allocation. Thin disks grow with use; the configured size is not the consumed size | the real consumption | |
| S-04 `FSTYPE` | `ext4`, `xfs` or `btrfs`. Copy-on-write copies (reflink) work on btrfs and on xfs with reflink enabled, not on ext4 (ASSUMED); this decides how baselines are copied (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 12) | | |
| S-04 shows an encryption layer (`crypto_LUKS`) | images are encrypted at rest; record it for recovery and performance notes | | |
| the images' directory is on a different mount than `/` | S-03 is not the relevant figure; F-22 is | | |

### 6.5 Routes, VPN, IPv6, Docker and address collisions

| Result | Meaning | Does not establish | Next |
|---|---|---|---|
| S-07: two default routes, one through the VPN interface with the lower metric | route preference by metric; the plan states this is not proof of a kill switch or leak | policy rules, tables, firewall, behaviour when the VPN drops | F-24, F-25, F-28 |
| F-24: fwmark rules and a separate table | policy routing, common with WireGuard tooling (ASSUMED); the effective default depends on these rules, so S-07 alone is incomplete | | F-25 |
| S-07 or F-26: a route or interface address inside `10.77.0.0/16` | an address collision with the lab plan; blocks precondition P-04 | | F-27; leaf L13 |
| F-27: the lab addresses leave through the default route | no lab route exists now (expected). The host has no lab address by design; nothing on the host should ever address a lab guest | | |
| S-08 empty | no IPv6 routes in the main table; not proof that IPv6 is off | per-interface state, link-local addresses, policy rules, firewall | F-23, F-25, F-26, F-28 |
| F-23: `disable_ipv6 = 1` for `all` | disabled on existing interfaces; new interfaces (`virbr`, `vnet`) take the `default` value, so check it too | | |
| F-23: `ip_forward = 1` | the host forwards IPv4 (Docker normally sets it, ASSUMED); relevant to the bridge-exposure design (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 7) | | |
| F-23: `rp_filter` and `arp_ignore` values | they decide whether a guest on a bridge can reach host addresses (section 7 of the architecture file) | | |
| a vulnerable application is found on the host by any row | a vulnerable application on the host, which plan section 3.2 forbids; record and stop (SAFETY_AUTHORIZATION, STOP-06) | | |

## 7. Decision tree: from returned evidence to the next diagnostic

The tree names the next read-only row; it never names an installation. Its leaves are decisions for the owner.

```text
START   S-01 to S-08 returned and recorded; owner has written down the failed virsh command (G1)
|
+-- N1  S-06 /dev/kvm
|     +-- exists and accessible ----------------------> F-07 -> N5
|     +-- exists, not accessible ---------------------> F-17, F-18, F-05 -> leaf L1
|     +-- absent -------------------------------------> F-04, F-03, F-02, F-06, F-07 -> leaf L2 or L3
|
+-- N2  S-05 and F-08 unit files
|     +-- none ---------------------------------------> F-14, F-13
|     |     +-- daemon packages absent ---------------> leaf L4
|     |     +-- packages present, no units -----------> F-21, F-10 -> leaf L5
|     +-- libvirtd units only, or virtqemud units ----> F-10, F-09, F-11 -> N3
|     +-- both families ------------------------------> F-10 -> leaf L6
|
+-- N3  state of the socket and service units
|     +-- masked -------------------------------------> leaf L7
|     +-- disabled or inactive -----------------------> leaf L8
|     +-- failed -------------------------------------> F-10 journal lines (class B) -> leaf L9
|     +-- active and listening: compare the path in F-11 with the path the failed command used
|           +-- paths differ ---------------------------> F-19, F-20, F-21 -> leaf L10
|           +-- same path, access blocked --------------> F-17, F-18 -> leaf L11
|           +-- same path, access allowed --------------> owner decides on class C row C-01 -> N4
|
+-- N4  class C probes, each separately approved
|     +-- C-01 answers -------------------------------> C-03 -> leaf L12
|     +-- C-01 fails ---------------------------------> record the exact text -> back to N3
|
+-- N5  network picture (independent of N1 to N4)
|     +-- S-07, F-26 or F-27 show a use of 10.77.0.0/16 -> leaf L13
|     +-- none ---------------------------------------> F-23 to F-28 recorded -> leaf L14
|
+-- N6  resources: S-02, S-03, S-04, F-22 recorded ---> leaf L15
```

| Leaf | Meaning (INFERRED) | What the owner may decide next, outside this document |
|---|---|---|
| L1 | `/dev/kvm` exists but the owner cannot open it | how access is granted (group, ACL, or system-instance guests only); a privilege decision |
| L2 | `/dev/kvm` absent, `svm` exposed, modules not loaded | whether to investigate module loading or blacklisting; any module change is class C |
| L3 | `/dev/kvm` absent and `svm` not exposed, or the kernel names firmware | the firmware setting is the owner's offline decision; record it |
| L4 | libvirt daemon packages absent (H1) | whether to install, and what. This document proposes no installation. A change approval record would need the exact package set, its source and checksum, side effects and rollback. Until then the status is BLOCKED and section 9 applies |
| L5 | packages present, unit files missing | an anomaly; the owner decides how to investigate package integrity (a further read-only row may be proposed) |
| L6 | both layouts installed | which layout the lab uses; no change is proposed |
| L7 | a masked unit | why it was masked; unmasking is class C |
| L8 | units present, inactive or disabled (H2) | whether to start or enable; class C |
| L9 | a unit failed | the cause from the journal; the fix is class C |
| L10 | the client's URI or path differs from the daemon's (H3, H4, H5) | which URI to use; a corrected attempt is C-01 under approval |
| L11 | the socket exists and the owner lacks access | the privilege model (O-HR-03) |
| L12 | a daemon answers on the intended URI | the earlier failure is explained; update preconditions P-01 and P-03 |
| L13 | an address collision with `10.77.0.0/16` | which side changes. The plan's addresses are fixed; another range would be a departure the owner decides |
| L14 | network baseline recorded | the before-and-after comparison base for P-05 |
| L15 | resource fit recorded | the base for P-06 |

Stage 00 becomes EVIDENCE-COMPLETE when N1, N2 and N3 reached a leaf with the cause named, and L14 and L15 are recorded.

## 8. What must be true before any local KVM lab is approved

Every row is NOT MET on 2026-11-18: no evidence exists for any of them.

| ID | Condition | Evidence required | Source | State |
|---|---|---|---|---|
| P-01 | the root cause of the libvirt blocker is identified, and the daemon layout, connection URI and privilege path are written down | recorded rows named by the tree; a leaf reached | plan section 2.1 | NOT MET |
| P-02 | KVM acceleration is usable: `/dev/kvm` exists and QEMU can use it under the intended connection | F-07 PASS lines, then a guest observed running accelerated after a separately approved first build step | plan section 2.1 ("not yet verified") | NOT MET |
| P-03 | the libvirt network, storage and QEMU drivers answer on the intended URI | C-01 and C-03 outputs | plan section 3.2 | NOT MET |
| P-04 | no route, interface, Docker network, VPN allowed range or libvirt network overlaps `10.77.0.0/16` in any table | S-07, F-24 to F-27 | plan section 3.2 ("check collisions") | NOT MET |
| P-05 | a baseline of the host's network state exists before any change: routes in all tables, rules, interfaces, firewall ruleset, forwarding sysctls, Docker networks, with a hash, so that a later comparison can show the VPN and Docker behaviour unchanged | the outputs of F-23 to F-28 (F-29 was removed) | plan section 2.2 ("preserve Proton/WireGuard and Docker behavior") | NOT MET |
| P-06 | resource fit: `MemAvailable` at the start of a session is at least the week's stack RAM plus 8 GiB headroom (PROPOSED heuristic, not measured), and free space at the image location is at least about 600 GB (the plan's target) | S-02, S-03 or F-22 at approval time | plan section 3.1 | NOT MET |
| P-07 | the owner has accepted the standing rules and signed the RoE for the exercise, and approved each mutating command | `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` (sections 2 and 12) | plan section 0.4 | NOT MET |
| P-08 | the nine isolation acceptance tests are written as reviewed commands, and the order is fixed: containment tests before any vulnerable service starts, positive tests last | `docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 11 | plan section 3.3 | NOT MET |
| P-09 | the recovery plan exists: baseline images, an AD build record, a system-state plan | `docs/curriculum/v2/LAB_ARCHITECTURE.md`, sections 10 and 12 | plan section 3.2 | NOT MET |
| P-10 | installation media provenance is recorded: sources, checksums, licences (Windows evaluation availability is ASSUMED: the week 3 registry did not confirm it, `docs/curriculum/v2/WEEKS/W03/RESOURCES.json`), router software choice and licence read | the owner's media record | plan sections 3.1 and 5 (week 3) | NOT MET |
| P-11 | a package-staging procedure is chosen (offline media, or a temporary staging network on a separate network that is removed and re-verified) | an approved procedure | plan section 3.2 ("separate approved procedure") | NOT MET |
| P-12 | QEMU and libvirt versions are recorded and updated under the owner's policy before a vulnerable guest runs | F-14 and F-15 outputs, an update decision | plan section 14.1 (residual hypervisor risk) | NOT MET |
| P-13 | a rollback exists for every host-level change that the build will make | the class C records | plan section 0.4 | NOT MET |
| P-14 | the criterion for "VPN, firewall and Docker unchanged" is written, and the comparison method is chosen | the owner's criterion, applied to P-05 | plan section 0.4 | NOT MET |

## 9. Fallback when the host stays blocked

Plan section 5, week 1 precondition: Stage 00 must be resolved before authorizing any local KVM deployment. If blocked, complete the first-day systems theory, hosted Bandit, and a paper topology, and defer only the setup-dependent exercises; document the schedule substitution transparently. The gate of week 1 accepts isolation as either verified or explicitly marked as a blocker with no intrusive local testing.

### 9.1 Rules while blocked

1. No intrusive local testing of any kind. No local vulnerable service is started.
2. Hosted exercises continue under class F scope lines (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`).
3. The scheduled hours do not change. A deferred exercise is replaced by the hosted or paper alternative the day guide names; nothing is added to the 480 hours and nothing is deleted (plan section 1.3).
4. Every substitution is recorded (section 9.3) and shown on the weekly scorecard.
5. The blocker is re-assessed at the start of each week by re-running only the rows the tree currently selects (PROPOSED cadence), never a full set.

### 9.2 What continues and what is substituted

The day and week guides name the exact hosted exercise; this table states the plan's own dependency and the kind of substitute.

| Week | Setup-dependent work | Substitute while blocked | Not demonstrated by the substitute |
|---|---|---|---|
| 1 | two-guest lab, static addresses, isolation check, restore (plan guided lab steps 4, 5, 10; day 1 block C) | first-day systems theory; hosted OverTheWire Bandit (registry `w01-otw-bandit`); a paper topology: the addresses, zones, routing matrix and test plan written, not tested | local guest isolation; the restore |
| 2 | scan of the lab target `10.77.20.20`; comparison of packet, router and service logs | a platform-designated target or room with its own ground truth; analysis of supplied captures | router logs and guest ground truth under the owner's control |
| 3 | local `corp.g9.test` build | a hosted directory-service exercise (the plan's gate accepts "hosted equivalent") | the build record and the local recovery |
| 4 and 5 | Juice Shop and crAPI inside the target guest; editable-source retest | PortSwigger Academy labs and hosted vulnerable-application rooms | the source-level fix and retest on code the owner controls |
| 6 | local privilege-escalation and segmentation exercises; the cross-segment test of day 29 | authorized hosted machines (the plan names THM and HTB machines); the midpoint on a hosted unknown target | removal of temporary routes and the restored-segmentation evidence |
| 7 | Wazuh deployment, agent enrollment, local detection tests | hosted SIEM and detection exercises; Sigma logic tested on supplied fixtures | collection of the owner's own events and the ingestion check |
| 8 | analysis of supplied datasets | the same datasets analysed in a hosted or disposable environment. Platform evidence is not redistributed | live generation of Windows artifacts |
| 9 | the malware workspace; compile and sanitizer run | static exercises on a hosted platform; no execution of samples | the owner's controlled crash, patch and regression |
| 10 | local advanced-AD graph and ACL correction | hosted directory exercises; graph analysis of supplied data | the local before-and-after permission test |
| 11 | cloud sandbox (also blocked by the absence of an approved sandbox); container lab | hosted or no-cloud lab, marked as a substitution (plan section 5). The toy agent may run without a guest if the owner approves a place for it (open decision O-HR-04); otherwise the exercise is on paper | cloud teardown evidence; agent boundary tests that were not run |
| 12 | the instrumented local lab for attack-to-detection correlation | a hosted or recorded-data substitute, labelled as not the instrumented lab. The unknown-target half is unaffected | the plan's separate instrumented lab; scored on what was demonstrated (plan section 9.4, acceptable partial performance) |

### 9.3 Substitution record

```text
Original exercise:   <day id, block, lab id>
Why blocked:         <Stage 00 leaf or condition, with the evidence record ids>
Substitute:          <platform, exercise id from the registry, scope line id>
What it teaches the same:   <...>
What it cannot show:        <...>
Evidence that replaces the original:  <artifact names>
Hours:               unchanged (the block's minutes are spent on the substitute)
Gate effect:         <for example: isolation = BLOCKER, no intrusive local testing>
Status:              SUBSTITUTED / DEFERRED / PAPER ONLY
Recorded by / date:  <owner, UTC and local>
```

### 9.4 What a blocked host cannot claim

LAB TESTED stays NO; a local lab outcome is never claimed from a substitute; a week whose gate needs local evidence is scored on what was demonstrated, and the gap is named in the weekly scorecard and in the portfolio's unresolved-risk register. The unknown-target work (day 30 and day 57) does not need the local lab; the instrumented capstone work (days 58 and 59) does.

## 10. Departures from the plan

| What | Why |
|---|---|
| Follow-up rows F-01 to F-31 are added | the plan requires follow-ups chosen from evidence and gives none; these are the rows needed to tell packages, units, sockets, URI, permissions and `/dev/kvm` apart |
| Connection probes (C-01 to C-03) are classed as not read-only | connecting to a socket-activated daemon can start it, and a per-user connection can create files; the plan's blanket statement that diagnostics write nothing holds for rows S and F only |
| The plan's host RAM is 60 GiB total (section 2.1) and "nominal 64 GiB" (section 3.1) | the observed figure is used for capacity: 60 GiB total, 55 GiB available at the observation; the plan's "about 33 GiB not committed" becomes about 29 GiB against 60 GiB (`docs/curriculum/v2/LAB_ARCHITECTURE.md`, section 2) |
| Stage 00 status values BLOCKED, EVIDENCE-COMPLETE and READY-FOR-APPROVAL are added | the plan has "not green" and no intermediate state; none of the three claims a lab exists |
| The plan says `sudo` may be requested "where specified" | none of the nine rows needs it; it appears only in F-06 (possibly) and F-28 |
| The host operating system is taken from `README.md` as ASSUMED | the plan's host description does not name it |

## 11. Open decisions for the owner

| ID | Decision | Default until decided |
|---|---|---|
| O-HR-01 | Approve, row by row, which diagnostics to run, and in which order | none recorded |
| O-HR-02 | Where VM images will live (path, filesystem, encryption) | `/var/lib` is checked by F-22 as the usual default (ASSUMED) |
| O-HR-03 | The privilege model for libvirt: membership of the socket's group, or `sudo` for each command | not decided; no group change is proposed |
| O-HR-04 | Where the toy agent of weeks 11 and 12 runs while the local lab is blocked | on paper only |
| O-HR-05 | Write down the failed `virsh` command, its user and its full text (gap G1) | the first step of Stage 00 |
