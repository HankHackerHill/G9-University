# G9 Cybersecurity Mastery v2.0: lab architecture (KVM, zones, router, SIEM, recovery, no WAN)

- Status: PROPOSED. This is a design. Nothing in it is deployed.
- Written: 2026-11-18
- Source: `docs/curriculum/v2/MASTER_PLAN_V2.0.md`, section 3 (sections 3.1 to 3.3), section 2.2 (preserve VPN and Docker), section 5 (weekly lab specifications), section 14.1 (adversarial table); `docs/adr/ADR-0023-a-second-curriculum-shape-the-v2-plan-of-60-weekdays.md`
- Rules: `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` (approval classes, scope lines, stop conditions); `docs/curriculum/v2/HOST_READINESS.md` (Stage 00 and the preconditions for any build)
- Labels: PROPOSED, ASSUMED, INFERRED, VERIFIED (only with evidence in the repository), NOT RUN. SEARCH_CHECKED means a search result of 2026-11-18 recorded in a week registry; the page was not opened.

> **Not applied, not run.** No guest, network, router, domain, SIEM, volume, snapshot or firewall rule described here exists. The libvirt definitions are design examples. Every command is proposed text for the owner's review, with its target, purpose, expected output, side effects, privilege, safe scope and rollback; this file approves none. All nine isolation acceptance tests are NOT RUN. Stage 00 is BLOCKED, so no build step is available (`docs/curriculum/v2/HOST_READINESS.md`, section 8). Example output is marked `Sample (synthetic):`.

## 1. How to read this file

| Term | Meaning here |
|---|---|
| Zone | one isolated layer-2 segment: RED (`10.77.10.0/24`), DMZ (`10.77.20.0/24`), CORP (`10.77.30.0/24`) |
| Guest id | the identifier of a lab guest, for example `G9-ATK-01`; the libvirt domain name is its lowercase form, for example `g9-atk-01` |
| Stack | the set of guests running at one time |
| Policy version | the SHA-256 of the text export of the router ruleset (section 6) |
| Baseline | a recorded, hashed, powered-off state of a guest or a set of guests (section 12) |
| Run id | the identifier of one execution of one test, `T<n>-<yyyymmdd>-<k>` |
| Evidence folder | the owner's private folder outside the guests and outside the course source; written `<evidence-folder>` |

A design statement is PROPOSED. A product behaviour that this file does not source is ASSUMED. Only observation by the owner, recorded under `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` rules, makes a statement OBSERVED, and only evidence stored in this repository would make one VERIFIED; none exists for the lab.

## 2. VM allocation and its caveats

### 2.1 The plan's allocation (plan section 3.1) with guest ids

| Guest id | Role (plan) | Zone and address | vCPU | RAM | Thin disk |
|---|---|---|---:|---:|---:|
| `G9-RTR-01` | OPNsense or pfSense: internal-only inter-zone policy and logs | RED `.1`, DMZ `.1`, CORP `.1` | 2 | 2 GiB | 20 GiB |
| `G9-ATK-01` | Linux attacker: authorized tools and analysis | RED `10.77.10.10` | 4 | 6 GiB | 80 GiB |
| `G9-TGT-01` | Ubuntu target: Docker-hosted intentionally vulnerable applications | DMZ `10.77.20.20` | 2 | 3 GiB | 40 GiB |
| `G9-DC-01` | Windows Server AD: domain controller, DNS, Kerberos, GPO | CORP `10.77.30.10` | 2 | 6 GiB | 80 GiB |
| `G9-WKS-01` | Windows workstation: domain-joined test endpoint | CORP `10.77.30.20` | 2 | 6 GiB | 70 GiB |
| `G9-SIEM-01` | Wazuh all-in-one: SIEM and endpoint telemetry | CORP `10.77.30.30` | 4 | 8 GiB | 100 GiB |
| Configured total | | | 16 | 31 GiB | 390 GiB |

The totals are the plan's. They are configured allocations on thin disks, not measured consumption.

### 2.2 Additions not in the plan's table

| Guest id | Role | Zone | vCPU | RAM | Thin disk | State |
|---|---|---|---:|---:|---:|---|
| `G9-MAL-01` | disposable malware and reverse-engineering workspace (section 8) | none: no network interface | 4 | 8 GiB | 60 GiB | PROPOSED. The plan names the guest but gives no allocation. It never runs with the full stack (section 13). Sized for a decompiler, sanitizer builds and a fuzzer; not measured |

With `G9-MAL-01` the configured thin disk total is 450 GiB. The week stacks (section 13) are what keeps simultaneous RAM at or under 31 GiB.

### 2.3 Caveats (the plan's, made operational)

| Caveat | Consequence |
|---|---|
| **Thin disks.** Configured size is not consumed size. A thin (sparse) image grows as the guest writes; baselines (section 12) multiply the consumption | The host's free space is the constraint. The plan aims at about 600 GB free to hold guests, captures, snapshots and growth; the owner's observation of about 1.7 TiB free (2026-11-14, stale) meets it. If the filesystem fills, guests can be paused or see I/O errors; the domain definition should stop a guest on a write error rather than continue (`error_policy='stop'`, ASSUMED libvirt disk-driver option; check the domain XML documentation) |
| **8 cores and 16 threads.** The 16 allocated vCPUs are a scheduling ceiling per guest, not 16 cores and not a reservation. They match the 16 SMT threads one to one only in the full stack | Heavy work (fuzzing in week 9, Wazuh indexing in weeks 7 and 12, a Windows update) should not overlap with a scored practical. The host keeps the desktop, the G9 MENTOR application, Docker and a browser |
| **Memory not committed.** The plan states about 33 GiB of a nominal 64 GiB is not committed to the guests, ignoring QEMU overhead, memory accounting, the filesystem cache, graphics or AI workloads and other host processes | The observed total is 60 GiB (`free -h`, 2026-11-14), so the margin is about 29 GiB; against the 55 GiB available at that moment it is about 24 GiB. Per-guest QEMU overhead and the page cache come out of that margin. A start-of-session rule (HOST_READINESS, P-06): available memory at least the stack's RAM plus 8 GiB (PROPOSED heuristic, not measured) |
| **Wazuh sizing.** The Wazuh quickstart gives 4 vCPU, 8 GiB and 50 GB for 1 to 25 agents (the plan's figure; the fact check is in `docs/curriculum/v2/WEEKS/W01/RESOURCES.json`, `plan_fact_checks`, SEARCH_CHECKED, and again in `docs/curriculum/v2/WEEKS/W07/RESOURCES.json`) | The 100 GiB disk is a planning reserve, not a verified requirement. The lab has about four agents. Pointer for the current figure: `docs/curriculum/v2/RESOURCE_REGISTRY.md` |
| **Resizing.** A guest may be resized only while the week's simultaneous RAM stays at or under 31 GiB, and the change is recorded in the lab record | the 31 GiB budget is never exceeded; for example `G9-TGT-01` may need more than 3 GiB for the crAPI containers: whether 3 GiB suffices is NOT established (ASSUMED tight) |

## 3. Logical topology and addressing

```text
G9 Ubuntu host (trusted; not itself a target; no address in 10.77.0.0/16)
  `-- KVM / libvirt (system instance, ASSUMED; Stage 00)
      |-- g9-lab-red   [virbr91]  10.77.10.0/24
      |     |-- G9-RTR-01  RED port   10.77.10.1
      |     `-- G9-ATK-01             10.77.10.10
      |-- g9-lab-dmz   [virbr92]  10.77.20.0/24
      |     |-- G9-RTR-01  DMZ port   10.77.20.1
      |     `-- G9-TGT-01             10.77.20.20
      `-- g9-lab-corp  [virbr93]  10.77.30.0/24
            |-- G9-RTR-01  CORP port  10.77.30.1
            |-- G9-DC-01   AD/DC/DNS  10.77.30.10
            |-- G9-WKS-01             10.77.30.20
            `-- G9-SIEM-01 Wazuh      10.77.30.30

G9-RTR-01: three isolated virtual interfaces, no NAT, no bridged WAN, no default gateway.
Domain: corp.g9.test (a test namespace; never a real customer domain).
G9-MAL-01: separate, disposable, no network interface, never joined to AD.
Docker with intentionally vulnerable applications runs inside G9-TGT-01 only.
```

| Zone | libvirt network | Bridge | CIDR | Router port | Members | Addressing |
|---|---|---|---|---|---|---|
| RED | `g9-lab-red` | `virbr91` | `10.77.10.0/24` | `10.77.10.1` | `G9-ATK-01` `.10` | static only |
| DMZ | `g9-lab-dmz` | `virbr92` | `10.77.20.0/24` | `10.77.20.1` | `G9-TGT-01` `.20` | static only |
| CORP | `g9-lab-corp` | `virbr93` | `10.77.30.0/24` | `10.77.30.1` | `G9-DC-01` `.10`, `G9-WKS-01` `.20`, `G9-SIEM-01` `.30` | static only; DNS from `G9-DC-01` |

### 3.1 Conventions (PROPOSED)

| Item | Convention |
|---|---|
| Address use inside a zone | `.1` router; `.10` to `.99` guests; `.100` and above unused. No DHCP, no DNS forwarding to anything outside the lab |
| Routes on guests | no default route anywhere. Each guest has specific static routes to the zones it may address, via the router port of its own zone. `G9-ATK-01` has routes to the DMZ and CORP prefixes so that the router, not a missing route, decides (isolation tests T2, T3). `G9-MAL-01` has none because it has no interface |
| Domain name | `corp.g9.test`. The `.test` top-level name is reserved for testing (RFC 6761, ASSUMED knowledge, not in the registry) and never resolves outside the lab |
| MAC plan | every NIC has a fixed, synthetic MAC with the QEMU prefix, so that the NIC inventory (test T8) can be compared with a list (below) |
| Time | libvirt starts each guest's clock from the host; clocks then drift apart. Windows guests keep local time in the hardware clock by default, so the event times of the Windows guests and of the Linux guests differ by an offset. Policy (PROPOSED): record host and guest `date -u` at the start of each session and at each test; use the router as the lab's only time source if guests need synchronization (open decision O-LA-04); normalize every timeline to UTC with the original offset kept (plan section 5, week 8, and `docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 10.4) |
| Operator access | the host has no address on the lab bridges. The operator reaches each guest through its local graphical or serial console (no TCP listener). The Wazuh dashboard is reached from the browser inside `G9-WKS-01`, which is in CORP (open decision O-LA-03). No SSH from the host to a guest exists in this design |
| Data in and out | read-only media in, text out (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 3.2) |

Synthetic MAC plan (PROPOSED). The fifth group is the zone as written (`10`, `20`, `30`); the sixth is the last octet of the guest's address in hexadecimal (`0a` is 10, `14` is 20, `1e` is 30); the router's ports use `01`:

| NIC | MAC |
|---|---|
| `G9-RTR-01` RED | `52:54:00:77:10:01` |
| `G9-RTR-01` DMZ | `52:54:00:77:20:01` |
| `G9-RTR-01` CORP | `52:54:00:77:30:01` |
| `G9-ATK-01` | `52:54:00:77:10:0a` |
| `G9-TGT-01` | `52:54:00:77:20:14` |
| `G9-DC-01` | `52:54:00:77:30:0a` |
| `G9-WKS-01` | `52:54:00:77:30:14` |
| `G9-SIEM-01` | `52:54:00:77:30:1e` |
| `G9-MAL-01` | none |

## 4. Routing policy between zones

### 4.1 Principles

1. Deny by default. The router starts every interface with no pass rules; each allowed flow is one rule naming source address, destination address, protocol and port, with logging.
2. A deny is a silent drop that is logged. Scans then show `filtered`, which is what week 2 teaches (plan section 5, week 2: do not infer absence from `filtered`). A reject would show `closed` and is not used for policy denies.
3. No address translation anywhere. A service in the DMZ sees the attacker's real lab address, which is evidence (test T1).
4. No default route on the router or on any guest; the router has no WAN.
5. Every exception has an RoE entry, a change approval, a time window, a removal step and a retest of the test that covers it. A temporary rule left in place is a stop condition (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, N-14, STOP-11).
6. The router cannot restrict traffic inside a zone: it is not in that path. Intra-zone restrictions are enforced on the destination guest (4.4).
7. The matrix is the source of truth. The router ruleset is derived from it, exported as text, and compared with it; a difference is a defect.

### 4.2 Matrix

| Source | Destination | Default | Allowed exceptions (each needs RoE and change approval) | Evidence |
|---|---|---|---|---|
| RED | DMZ | deny | E1: the named lab services of one exercise, from `G9-ATK-01` to `G9-TGT-01` only (plan: "explicitly allow necessary lab services") | T1: router decision, captures on both sides, service log |
| RED | CORP | deny and log (plan) | E3: a time-boxed approved cross-segment test (week 6 day 29; attack-simulation traffic in weeks 7, 10 and 12, as those day guides say), listing destination and ports | T2: router log, failed connection, route confirmation; after removal T2 is repeated |
| DMZ | RED | deny | E2: a reverse connection to one named listener on `G9-ATK-01`, one port, one window | rule hit, listener log; removal retest T1 reverse and T2 |
| DMZ | CORP | deny, except approved telemetry (plan) | E4: `G9-TGT-01` to `G9-SIEM-01`, Wazuh agent ports only (see 4.5) | T3 (denied flows), T4b (allowed telemetry) |
| CORP | RED | deny | none | T2 mirror check by the same method |
| CORP | DMZ | deny | E5: a CORP client reaching a DMZ service for one named exercise (client-side or pivot exercises), one destination, ports named | rule hit, service log; removal retest |
| CORP | CORP (inside the zone) | not through the router | on `G9-SIEM-01` the host firewall permits only the ports the SIEM needs (plan: "CORP to Wazuh permit only actual required ports"), see 4.4 | T4a |
| any guest | router's own addresses | deny except what the owner needs for the design; see section 6 | none by default | router export |
| any guest | the G9 host | deny | none. The probe of T5 is a test, not an exception | T5, T7 |
| any guest | household LAN (`192.168.1.0/24` at the observation) | no route; deny (plan) | none | T5 with host captures |
| any guest | Internet | no route during active training (plan) | none. Package staging is a separate procedure on a separate network (4.7), never an exception on a lab zone | T6 with host captures |
| any guest | IPv6 off-link | no route; IPv6 disabled or dropped | none | T7 |
| the host | any guest | not part of the design: the host has no lab address | none | T8, T5 |

### 4.3 Exceptions E1 to E5

| Id | Direction | Typical use | Lifetime | Removal evidence |
|---|---|---|---|---|
| E1 | RED to DMZ | web, SSH or API of the target exercise | one exercise window | rule gone from the export; T1 negative check (the port is denied again) |
| E2 | DMZ to RED | reverse connection or out-of-band confirmation | one exercise window | T1 reverse check; no listener left on the attacker |
| E3 | RED to CORP | cross-segment test; attack-simulation traffic for detections | one exercise window | export shows no RED to CORP pass rule; T2 repeated and PASS |
| E4 | DMZ to CORP | Wazuh agent telemetry from the target | standing for stacks that include the SIEM; reviewed at each baseline | export shows only the agent ports; T3 and T4b |
| E5 | CORP to DMZ | a client exercise | one exercise window | as E1 |

### 4.4 Inside CORP

The three CORP guests share one bridge, so frames between them never cross `G9-RTR-01`. The restriction on what may reach the SIEM is therefore enforced on `G9-SIEM-01`: default deny inbound; allow the agent and enrollment ports and the dashboard port only from the sources the lab needs. Which ports those are is read from the SIEM's own listening sockets (`ss`, test T4a), not assumed.

### 4.5 Wazuh ports (ASSUMED)

The Wazuh components use agent communication on TCP 1514, agent enrollment on TCP 1515, a server API on TCP 55000 and a dashboard on TCP 443, by default (ASSUMED from the product's documentation; not checked, `docs/curriculum/v2/WEEKS/W07/RESOURCES.json` holds the quickstart entry `w07-wazuh-quickstart`). The rules use the ports that test T4a reads from the installed version. Enrollment (1515) is open only during an enrollment window.

### 4.6 Router rule hazards (ASSUMED product behaviour, checked by tests)

| Hazard | Effect | Caught by |
|---|---|---|
| The product ships a default pass-all rule on its first internal interface | RED would reach everything | router export compared with the matrix; T2 |
| An interface given the WAN role blocks private (RFC 1918) sources by default | lab traffic silently dropped | T1 |
| Hardware checksum or segmentation offload on virtual NICs corrupts or stalls TCP through the router | SYN seen on both sides, no completed session | T1 (check offload settings) |
| The product enables IPv6 or router advertisements on an interface | an IPv6 path appears | T7 |
| A "temporary" staging NIC is left attached | a route to the outside | T8 |

### 4.7 Package staging

Package updates for guests and the router need a network. The plan says to use a separate approved procedure and to disconnect and re-verify isolation before tests. Proposed forms: offline media prepared on the host (preferred), or a temporary NAT network attached to one powered-off guest, used while no intentionally vulnerable service has been started on it, then removed with a T8 check and the T5 to T7 checks repeated. The choice is HOST_READINESS precondition P-11.

## 5. Libvirt network definitions: design examples, not applied

The definitions follow the plan: no `<forward>`, no `<ip>`, so libvirt builds a bridge with no address, no DHCP and no DNS service. `stp="on"` and `delay="0"` are the plan's values.

```xml
<network>
  <name>g9-lab-red</name>
  <bridge name="virbr91" stp="on" delay="0"/>
</network>
```

```xml
<network>
  <name>g9-lab-dmz</name>
  <bridge name="virbr92" stp="on" delay="0"/>
</network>
```

```xml
<network>
  <name>g9-lab-corp</name>
  <bridge name="virbr93" stp="on" delay="0"/>
</network>
```

Notes (sources: plan section 3.2; registry `w01-libvirt-formatnetwork`, SEARCH_CHECKED, in `docs/curriculum/v2/WEEKS/W01/RESOURCES.json`):

1. A network without `<forward>` is isolated from other networks, yet its guests can still reach each other and the host operating system. That is why this design adds section 7 and the tests. The network definition is not a complete containment boundary.
2. The definition does not mean the bridge has no host presence. The host kernel still holds the bridge device. See section 7.
3. Before any definition: no existing interface, route, Docker network, VPN range or libvirt network may overlap `10.77.0.0/16` or use the names `virbr91` to `virbr93` (HOST_READINESS rows F-26 and F-27, precondition P-04). Libvirt's own default network, if it exists (commonly `virbr0` on `192.168.122.0/24`, a NAT network), must have no lab guest attached.
4. No autostart is proposed. The lab networks exist only while a lab session runs, which keeps host exposure to those sessions.
5. Starting a libvirt network also adds host firewall rules for it (the libvirt firewall backend, ASSUMED); that is a host firewall change, captured by the before-and-after comparison of P-05.

Interface example for one guest (a fragment, not a definition; the MAC is from the plan above):

```xml
<interface type='network'>
  <mac address='52:54:00:77:10:0a'/>
  <source network='g9-lab-red'/>
  <model type='virtio'/>
</interface>
```

Console example (a fragment). The graphics server listens on a local socket, with clipboard and file transfer disabled (libvirt domain XML options; read the domain XML documentation for the installed version before use, ASSUMED):

```xml
<graphics type='spice'>
  <listen type='none'/>
  <clipboard copypaste='no'/>
  <filetransfer enable='no'/>
</graphics>
```

Optional anti-spoofing (PROPOSED, ASSUMED to need libvirt's network filter driver): a filter reference on each interface that binds the guest to its MAC and address, so that a guest cannot answer ARP for another guest's address. Open decision O-LA-05.

### 5.1 Proposed host commands for the network definitions (class C; no approval recorded)

URI `qemu:///system` is ASSUMED until Stage 00. Each file holds one of the three definitions above; `<file>` is its path in the evidence folder.

| ID | Command | Target | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| LA-01 | `virsh -c qemu:///system net-define <file>` (once per definition) | the system libvirt instance | stores a persistent network definition; prints that the network was defined | writes a definition under libvirt's configuration directory; creates no bridge yet | libvirt system connection | the three lab networks only | `virsh -c qemu:///system net-undefine <name>` |
| LA-02 | `virsh -c qemu:///system net-start g9-lab-red` (and `g9-lab-dmz`, `g9-lab-corp`) | the system instance | creates the bridge; prints that the network started | creates a bridge device; adds libvirt firewall rules; a host-level change | libvirt system connection | one network at a time | `virsh -c qemu:///system net-destroy <name>` |
| LA-03 | `virsh -c qemu:///system net-dumpxml g9-lab-red` | the defined network | prints the stored definition; compare with the file | none | libvirt system connection | one network | none needed (read-only) |
| LA-04 | `ip -br addr show dev virbr91` and `ip -d link show dev virbr91` | the host's bridge device | the bridge exists, is up, has no IPv4 address; its link-local IPv6 state (section 7) | none | user | the lab bridges | none needed (read-only) |
| LA-05 | `virsh -c qemu:///system net-list --all` | the system instance | lists defined networks; only the three lab networks and any pre-existing ones | none beyond the class C connection | libvirt system connection | listing only | none needed (read-only) |
| LA-06 | `virsh -c qemu:///system net-destroy <name>` | one lab network | stops the network and removes its bridge | removes the bridge; removes its libvirt firewall rules; guests on it lose their link | libvirt system connection | one network, guests shut down first | `net-start <name>` |
| LA-07 | `virsh -c qemu:///system net-undefine <name>` | one lab network | removes the persistent definition | deletes the definition | libvirt system connection | one network, stopped first | `net-define <file>` |

## 6. Guest router `G9-RTR-01`

| Item | Design (PROPOSED) | Check |
|---|---|---|
| Product | OPNsense or pfSense, the owner's choice; licence, registration and release availability are read by the owner (open decision O-LA-01). Product behaviours below are ASSUMED | decision recorded in the media record (P-10) |
| Interfaces | three virtual NICs, one per zone, identified by MAC (section 3.1); no fourth interface, ever | T8 |
| Roles | all three are internal interfaces. If the product insists on a WAN role, the interface given it is a lab zone, never an uplink, and its default block-private rule is disabled | T1; the export |
| Addresses | static `.1` in each zone; no DHCP server, no DNS forwarding, no dynamic DNS | export |
| Default route and NAT | none and none | export; T6 |
| Rules | the matrix of section 4, default deny per ingress interface, drop on deny, logging on every deny and on every pass of an exception | export compared with the matrix |
| Services | SSH, UPnP, the resolver, and any package or update service are off. The management GUI is off except during a recorded maintenance window, enabled on one interface from one source (open decision O-LA-02) | export |
| IPv6 | disabled on all interfaces; no router advertisements | T7 |
| Time | NTP serves the lab from the router's own clock if needed; no upstream | export; section 3.1 |
| Logs | firewall logs kept locally and exported as text for each test | test evidence |
| Configuration backup | a text export, hashed, kept in the evidence folder; with the disk baseline (section 12) | policy version |
| Management path | console only by default. A GUI on a lab zone is reachable by whatever sits in that zone, including a compromised guest, so it is opened only for a maintenance window | O-LA-02 |

Residual: the router is a guest on the same hypervisor; a hypervisor flaw is outside what its rules protect (section 7).

## 7. Host exposure from the guest side

The plan lists "libvirt host reachable from guest" as an adversarial failure mode and asks for least host exposure and host and guest interface and firewall tests. The following paths are INFERRED from how Linux and libvirt are commonly built; none is observed here. The tests decide.

| Path | Mechanism (INFERRED) | Control (PROPOSED, each a class C change) | Verified by |
|---|---|---|---|
| Host services on the host's other addresses, reached over a lab bridge | the bridge device is a host interface; Linux by default answers ARP for any local address on any interface and accepts packets for any local address that arrive on any interface (the weak host model), subject to reverse-path filtering and the firewall | a host firewall rule in a dedicated table that drops input and forward on the three lab bridges; `arp_ignore` and `arp_announce` set restrictively on the three bridges, as defence in depth | T5 (host-address probe) |
| IPv6 link-local on the bridge devices | the kernel assigns a link-local IPv6 address to a bridge that is up unless IPv6 is disabled on it; a host service bound to all IPv6 addresses is then reachable from the zone | `disable_ipv6` set on the three bridges; the same drop rule for IPv6 | T7 |
| Forwarding through the host | the host forwards IPv4 when Docker is installed (ASSUMED); with no host address or route on the lab bridges there is no path, but a later mistake would create one | a drop rule for forward traffic in and out of the lab bridges; no host route to `10.77.0.0/16` | T5, T6 |
| Bridge traffic crossing the host firewall | when the bridge netfilter module is loaded, bridged frames are also evaluated by the host's forward chains; Docker's forward policy can then drop lab traffic, and a loose host rule can accidentally permit it (a known interaction, ASSUMED) | explicit rules scoped to the three lab bridges in a dedicated chain, evaluated at build time. Docker's own settings are not changed (plan section 2.2) | T1 (an unexplained drop), T8 |
| Hypervisor or device-model escape | a bug in QEMU or a virtual device lets a guest act on the host | patched QEMU and libvirt under the owner's policy; the guest process runs as an unprivileged service user under mandatory-access confinement (Ubuntu's per-guest AppArmor profiles, ASSUMED); no device a guest does not need (T8); no host paths exposed | the confinement of each guest process is inspected at build time; residual risk is accepted by the owner and is not removed by any test |
| Console channels | a graphics console on a TCP port would be a network service | graphics listen on a local socket only (section 5) | T8 (dumpxml) and a listening-socket check on the host |
| Host-initiated connections | none are designed | no host address on the bridges | T8 |

## 8. Malware workspace `G9-MAL-01`

Plan: a separate disposable VM with disconnected adapters, no shared clipboard or folders, no real secrets, never joined to AD; static educational binaries and recorded evidence by default; no external network in the malware workspace. The rules of use are in `docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 6.

| Item | Design (PROPOSED) | Check |
|---|---|---|
| Network | no `<interface>` element at all (a stricter reading of "disconnected adapters"); no network driver attached | T8: zero NICs in the live and the persistent definition |
| Other devices | no shared folder, no host device passthrough, no USB redirection, no guest-agent or SPICE agent channel, no sound | T8 dumpxml review |
| Console | local socket only, clipboard and file transfer disabled (section 5) | dumpxml |
| Storage | a read-only base image with a per-session overlay; the overlay is discarded at the end of the session; the base's hash is rechecked | section 12 commands |
| Samples in | read-only optical media built on the host from hashed files; the host never opens or executes a sample | media hash list |
| Text out | a serial console log written to a file in the evidence folder; its content is untrusted text (strip control characters before display) | log file |
| Tools | installed from offline media before the base image is frozen, with hashes; no package update afterwards | tool manifest |
| Use | static analysis, toy-binary builds with sanitizers, a controlled fuzzing run with a CPU cap, rule tests; not a network sandbox | week 9 guide |
| Never | joined to `corp.g9.test`; given an address; run together with a full stack | section 13 |

Fragment of a design for the domain (not a definition; elided parts are marked):

```xml
<domain type='kvm'>
  <name>g9-mal-01</name>
  <memory unit='GiB'>8</memory>
  <vcpu>4</vcpu>
  <devices>
    <!-- disk: session overlay over a read-only base ... -->
    <!-- disk: read-only cdrom with the hashed sample media ... -->
    <graphics type='spice'>
      <listen type='none'/>
      <clipboard copypaste='no'/>
      <filetransfer enable='no'/>
    </graphics>
    <serial type='file'>
      <source path='<evidence-folder>/g9-mal-01.console.log'/>
      <target port='0'/>
    </serial>
    <!-- no interface, filesystem, hostdev, redirdev, channel or sound element -->
  </devices>
</domain>
```

## 9. Docker inside the target guest only

Plan: intentionally vulnerable Docker applications live inside the Ubuntu target VM, never on the trusted G9 host; do not mount a Docker socket, production secrets, an SSH agent or sensitive files into deliberately vulnerable containers. "Host" in the plan's rule is the Docker host, which for these applications is `G9-TGT-01`.

| ID | Rule (PROPOSED) | Check |
|---|---|---|
| D-01 | No vulnerable image runs on the trusted host's Docker. The host's Docker, its networks and its behaviour are not changed | HOST_READINESS F-30 |
| D-02 | Images are pulled only during staging (a class C or D approval; the plan lists Docker image pulls as needing approval), pinned by digest, and the digest is recorded. The baseline is taken after the pull | media record |
| D-03 | No container mounts the Docker socket, the guest's home, an SSH agent, a credential file or any path other than a synthetic data directory | `docker inspect` row TD-02 |
| D-04 | No container is privileged or given extra capabilities, unless one exercise's RoE says so and names it (week 11 privileged-container exercise) | TD-02 |
| D-05 | The Docker daemon does not listen on a TCP port | listening-socket check TD-04 |
| D-06 | Applications bind the guest's loopback by default (the plan's registry note for Juice Shop: loopback bind within the isolated VM). Exposure on `10.77.20.20` for a RED exercise is E1, one port, one window | TD-02, TD-04 |
| D-07 | Memory, CPU and process limits are set on each container so that one cannot starve the guest | TD-02 |
| D-08 | Data in containers is synthetic; no credential of the owner exists in the guest | fixture manifest |
| D-09 | The addresses inside the guest's Docker (for example `172.17.0.0/16`) are unrelated to the host's Docker addresses of the same range; every evidence record names the system where an address was seen | evidence record |
| D-10 | Whether the crAPI container set fits in 3 GiB is not established; resize inside the 31 GiB rule if needed | week 5 guide |

Proposed commands inside `G9-TGT-01` (class D; no approval recorded):

| ID | Command | Target | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| TD-01 | `docker run -d --name juice --memory 1g --cpus 1 --pids-limit 256 --cap-drop ALL -p 127.0.0.1:3000:3000 <image reference pinned by digest>` | `G9-TGT-01` | starts one training application bound to the guest's loopback; prints a container id. Whether the application runs without capabilities is ASSUMED | starts a container, opens a loopback port in the guest | a user in the guest's docker group (root-equivalent in the guest) | the guest only | `docker rm -f juice` |
| TD-02 | `docker inspect --format '{{.HostConfig.Privileged}} {{.HostConfig.Binds}} {{.HostConfig.NetworkMode}} {{.HostConfig.PortBindings}}' juice` | the container's configuration | expect `false`, no binds, the default network, the loopback port only | none | as TD-01 | the one container | none needed (read-only) |
| TD-03 | `docker ps --format '{{.Names}} {{.Image}} {{.Ports}}'` | the guest's Docker | lists containers, images and published ports | none | as TD-01 | the guest | none needed (read-only) |
| TD-04 | `ss -tlnp` | the guest | listening sockets; the Docker daemon is not on TCP; the application is on the loopback address (process names need root) | none | user, or root for process names | the guest | none needed (read-only) |
| TD-05 | `docker rm -f juice` | the container | removes it | deletes the container and its writable layer | as TD-01 | the one container | recreate from TD-01 |

## 10. AD backup and restore

Plan: maintain reproducible domain build documentation and test restoration; do not equate arbitrary DC VM snapshots with reliable AD backups; respect Microsoft VM-GenerationID and system-state backup guidance. The Microsoft guidance itself is not in the registry yet; the week 3 author adds the entries, and until then every statement about Microsoft behaviour here is ASSUMED. Pointer: `docs/curriculum/v2/RESOURCE_REGISTRY.md`.

### 10.1 Why a domain-controller snapshot is not a backup (INFERRED, ASSUMED sources)

A directory database carries its own version counters and an identity of the database copy (its invocation id). Returning the disk to an earlier moment makes the controller reuse counter values it already issued, which can diverge it from any partner (a rolled-back update sequence) and can reissue identifiers it already handed out (relative-id pools). Windows Server 2012 and later controllers on a hypervisor that exposes a VM generation identifier compare it with the value stored in the database at start; when it differs they treat the start as a rollback and take protective steps (a new invocation id, a discarded relative-id pool, a non-authoritative replication of the shared folders). This protection exists only if the hypervisor presents the identifier and changes it when the machine's position in time changes. Separately, a member computer's machine-account password changes on its own schedule, so a controller restored alone can disagree with a member that was not restored (a broken secure channel).

### 10.2 What libvirt provides (ASSUMED)

Libvirt can expose a generation identifier to a guest with a `<genid>` element in the domain definition (ASSUMED; read the domain XML documentation for the installed version). Whether a given libvirt and QEMU build changes the identifier when a disk is reverted by file copy, when an overlay is discarded, or when a snapshot is reverted is not established here. Rule: no restore of `G9-DC-01` is trusted on the strength of the identifier; the restore test (T9, AD variant) observes the controller's own behaviour.

### 10.3 Recovery levels for the directory

| Level | Method | Status |
|---|---|---|
| R-AD1 | Rebuild from the domain build record (10.4), on a fresh baseline of `G9-DC-01` and `G9-WKS-01`. The primary recovery | PROPOSED; used when anything is in doubt |
| R-AD2 | Restore from a system-state backup taken with Windows Server Backup to a separate virtual disk, booting into directory services restore mode | PROPOSED; tested once (T9, AD variant) |
| R-AD3 | Return the whole set (`G9-DC-01`, `G9-WKS-01`, `G9-SIEM-01`) to a cold baseline set taken together while powered off | a lab shortcut, not an AD backup. Valid for the single-controller lab (INFERRED: there is no replication partner to diverge from); never the only recovery; never applied to the controller alone |

If a second controller is ever added, R-AD3 is withdrawn for it.

### 10.4 Domain build record

A text record kept in the evidence folder: forest and domain names (`corp.g9.test`), functional levels, the controller's name and address, DNS zones, the organizational units, groups, synthetic users and their roles, group policy objects with their links, the audit policy, the order of the steps, the exact commands run with their times, and the synthetic directory-services restore mode secret kept in the owner's vault, never in a course file. It is complete when a stranger could rebuild the domain from it.

### 10.5 Proposed commands in the Windows guests (class D; no approval recorded)

PowerShell and command-line tools of Windows Server; names and options are ASSUMED from Microsoft's tools and must be checked against the installed version before use.

| ID | Command | Target | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| AD-01 | `Install-WindowsFeature Windows-Server-Backup` | `G9-DC-01` | adds the backup feature; reports success. It may need installation media because the guest has no Internet | installs a feature; may need a restart | local administrator | the controller | `Uninstall-WindowsFeature Windows-Server-Backup`, or return to the baseline |
| AD-02 | `wbadmin start systemstatebackup -backuptarget:<target volume>: -quiet` | `G9-DC-01` | writes a system-state backup to a separate virtual volume; ends with a completion message | writes the backup; uses time and space; a second virtual disk must be attached under a class C record | local administrator | the controller | delete the backup set; detach the disk |
| AD-03 | `wbadmin get versions` | `G9-DC-01` | lists the backup versions with their identifiers | none | local administrator | the controller | none needed (read-only) |
| AD-04 | `bcdedit /set safeboot dsrepair`, then restart, then `wbadmin start systemstaterecovery -version:<version identifier>` | `G9-DC-01` | boots into directory services restore mode and restores the system state. The restore type (authoritative or not) is chosen by the week 3 author from Microsoft's current guidance | replaces the directory database and the shared policy folder; reboots | local administrator in restore mode | the controller | `bcdedit /deletevalue safeboot` to leave restore mode; otherwise return to R-AD1 |
| AD-05 | `dcdiag /q` | `G9-DC-01` | prints only failed tests; no output means no failure was reported | none (generates test traffic to the controller itself) | domain administrator | the domain | none needed (read-only) |
| AD-06 | `repadmin /showrepl` | `G9-DC-01` | replication status and the controller's invocation id | none | domain administrator | the domain | none needed (read-only) |
| AD-07 | `nltest /dsgetdc:corp.g9.test` | `G9-WKS-01` | shows which controller the workstation locates | none | domain user | the domain | none needed (read-only) |
| AD-08 | `Test-ComputerSecureChannel -Verbose` | `G9-WKS-01` | tests the workstation's secure channel with the domain; returns True or False. Without `-Repair` it changes nothing | none | local administrator | the workstation | none needed (read-only) |
| AD-09 | `Get-WinEvent -LogName 'Directory Service' -MaxEvents 50` | `G9-DC-01` | the latest directory-service events, to look for the controller's reaction to a start after a restore (event ids to be recorded after observation) | none | local administrator | the controller | none needed (read-only) |

### 10.6 Pairing and honesty

Restore the controller, the workstation and the SIEM from the same baseline set. A restored SIEM manager can lack the agents' registration; a restored workstation can disagree with a controller restored to another time. Nothing in this section is claimed to work: the only evidence that restoration works is a recorded T9 AD variant, and the state until then is NOT RUN.

## 11. Isolation acceptance tests

The plan's nine tests (plan section 3.3) are mandatory evidence before any intrusive local work. All are NOT RUN.

### 11.1 Conventions

| Item | Rule |
|---|---|
| Scope | each test is a class E activity under a scope line and an RoE (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`). The sources and destinations are lab addresses, the owner-designated address of T5, and documentation-reserved addresses (RFC 5737 and RFC 3849) in T6 and T7 |
| Independent observation | each result has at least two independent observations: the router or guest view, and a host-side capture on the virtual interface of a guest (found by `virsh domiflist`). A bridge device may not show frames exchanged between other ports, so captures are taken on the guests' own tap interfaces |
| Order of execution | not the plan's table order. Containment tests first, positive tests last: T8, T6, T7, T5, then T2, T3, then T1, T4, then T9. No intentionally vulnerable service is started before T8, T6, T7 and T5 pass for the stack |
| Result values | PASS, FAIL, INCONCLUSIVE, NOT RUN. A PASS holds only for the stack, the baselines and the policy version recorded with it |
| Evidence record | test id; run id; UTC and local time with zone; the stack with guest ids and baseline ids; the policy version; the host baseline reference (P-05); commands as run, verbatim, with outputs; capture files with SHA-256; result and rationale; deviations; operator |
| Retest triggers | any change to a network definition, a guest NIC, the router ruleset or an exception; a host firewall, sysctl, VPN or Docker change; a QEMU or libvirt update; a host reboot; a baseline restore; a new guest. The retest is the tests the change can affect, plus T8 |
| Applicability by stack | see section 13: the tests that must be current before intrusive work |

Recurring command rows (class E or read-only; no approval recorded), referred to by id in the tests:

| ID | Command | Target | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| X-01 | `virsh -c qemu:///system domiflist <domain>` | one guest | its NICs: interface (tap) name, network, model, MAC | none | libvirt system connection | the lab guests | none needed (read-only) |
| X-02 | `sudo tcpdump -i <tap> -nn -p -w <evidence-folder>/<name>.pcap '<filter>'` | one guest's tap interface on the host | writes the matching packets to a file; stop with an interrupt | a capture file; reading raw packets | root | lab taps and, in T5 to T7, the host's LAN and VPN interfaces with the filter `net 10.77.0.0/16` | stop the capture; keep or delete the file under the evidence rules |
| X-03 | `sudo tcpdump -nn -v -r <evidence-folder>/<name>.pcap` | a saved capture | prints packets with TTL and flags | none | root, or the file's owner | the capture | none needed (read-only) |
| X-04 | `sha256sum <evidence-folder>/<name>.pcap` | a saved capture | the file's SHA-256, recorded | none | user | the evidence folder | none needed (read-only) |
| X-05 | `ip route get <address>` | a Linux guest | the kernel's route choice, no packet sent | none | user | the guest | none needed (read-only) |
| X-06 | `ip -br addr` and `ip route` | a Linux guest | interfaces, addresses, routes | none | user | the guest | none needed (read-only) |

Domain names are the lowercase guest ids. `<tap-red>`, `<tap-dmz>` and `<tap-corp>` are the router's interfaces on the three networks, taken from X-01 on `g9-rtr-01`.

### 11.2 T1: operator and RED to a permitted DMZ test service (plan: allowed)

Preconditions: the RED-DMZ stack runs; the router has one pass rule for exception E1 from `10.77.10.10` to `10.77.20.20` TCP 8080, logged; a benign listener serves one synthetic file.

| # | Where | Command | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| 1 | host | X-01 on `g9-rtr-01` | maps the router's taps | none | libvirt | router | none needed (read-only) |
| 2 | host | X-02 twice: `<tap-red>` and `<tap-dmz>`, filter `host 10.77.20.20 and tcp port 8080` | two capture files | capture files | root | the router's two taps | stop; keep files |
| 3 | `G9-TGT-01` | `python3 -m http.server 8080 --bind 10.77.20.20 --directory <empty directory holding one synthetic index file>` | a benign listener; request lines print to its output | opens a port in the guest until stopped | user | the guest | interrupt the process |
| 4 | `G9-ATK-01` | `curl -sS --max-time 5 -o /dev/null -w '%{http_code}\n' http://10.77.20.20:8080/index.html; echo "exit=$?"` | expect `200` and `exit=0` | one request | user | the target address only | none needed |
| 5 | `G9-ATK-01` | `curl -sS --max-time 5 -o /dev/null -w '%{http_code}\n' http://10.77.20.20:8081/; echo "exit=$?"` | expect `000` and `exit=28` (timeout): port 8081 is not allowed | one blocked attempt | user | the target address only | none needed |
| 6 | host | stop the captures; X-03 and X-04 on both files | the SYN appears on both taps; the TTL on the DMZ tap is one lower than on the RED tap; hashes recorded | none | root | the captures | none needed (read-only) |
| 7 | router | export the firewall log lines for the window | the pass rule hit for step 4 and a block entry for step 5, with rule labels | none | router admin | the router | none needed |

Expected result: HTTP 200; the service log shows client `10.77.10.10` (no translation); a pass entry with its rule id; the same connection on both taps with the TTL reduced by one (a routed hop, not a layer-2 bypass); port 8081 blocked with a log entry and no SYN on the DMZ tap.
Evidence to keep: both captures and hashes; router log excerpt with rule ids and times; the listener's output; the curl outputs with times; the policy version; stack and baseline ids.
A failure means: success with no router log, or an unchanged TTL, indicates a layer-2 path around the router (an extra NIC or shared bridge): stop (STOP-03, class B2). A failed request with a SYN on the RED tap only is a rule, route or offload problem, not a safety failure; repair under class D. Success on 8081 means the allow rule is too wide. A service log showing the router's address means translation is on, which contradicts the design.

### 11.3 T2: RED to a non-approved CORP service (plan: denied)

Preconditions: the CORP guests run; a listener exists at `10.77.30.30` TCP 443 (the dashboard) or a benign listener on `G9-SIEM-01`; `G9-ATK-01` has a route to the CORP prefix via `10.77.10.1` so that the router decides; no exception E3 is open.

| # | Where | Command | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| 1 | `G9-ATK-01` | X-05 with `10.77.30.30` | shows the route via `10.77.10.1`: the denial will be policy, not a missing route | none | user | the guest | none needed (read-only) |
| 2 | `G9-WKS-01` | `Test-NetConnection -ComputerName 10.77.30.30 -Port 443` | control: expect `TcpTestSucceeded : True`, so the listener is up | one connection inside CORP | domain or local user | the SIEM address | none needed |
| 3 | host | X-02 on `<tap-red>` and `<tap-corp>`, filter `host 10.77.30.30 and tcp port 443` | capture files | capture files | root | the router's taps | stop; keep files |
| 4 | `G9-ATK-01` | `curl -k -sS --max-time 5 -o /dev/null -w '%{http_code}\n' https://10.77.30.30/; echo "exit=$?"` | expect `000` as the code and `exit=28` (timeout) | one blocked attempt | user | the SIEM address | none needed |
| 5 | router | export the log lines for the window | a block entry: source `10.77.10.10`, destination `10.77.30.30`, port 443, rule label | none | router admin | the router | none needed |
| 6 | host | X-03 and X-04 | a SYN on the RED tap, none on the CORP tap | none | root | the captures | none needed (read-only) |

Expected result: the control connects from inside CORP; from RED the connection times out; the router logs the block; the SYN is seen at the RED tap and never at the CORP tap.
Evidence to keep: route output; the control result; the router log; both captures with hashes; policy version; stack and baselines.
A failure means: a connection succeeded: RED reaches CORP, stop and contain (STOP-03, B2; step C4 on the router). A timeout with a SYN on the CORP tap means the router forwarded the packet and something later dropped it: the policy did not deny, INCONCLUSIVE for policy and an error in the ruleset. A timeout with no router log entry means it was dropped elsewhere (for example the host firewall or no route): the test does not show router policy; repeat with the route in place. "Network is unreachable" at step 1 means the deny was a missing route; the test is repeated with the route.

### 11.4 T3: DMZ to a non-approved CORP service (plan: denied)

Preconditions as T2, with `G9-TGT-01` as the source and the same listener; exception E4 (telemetry) is in place for the agent ports only.

| # | Where | Command | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| 1 | `G9-TGT-01` | X-05 with `10.77.30.30` | the route via `10.77.20.1` | none | user | the guest | none needed (read-only) |
| 2 | host | X-02 on `<tap-dmz>` and `<tap-corp>`, filter `host 10.77.30.30 and tcp port 443` | capture files | capture files | root | the router's taps | stop; keep files |
| 3 | `G9-TGT-01` | `timeout 5 bash -c ': < /dev/tcp/10.77.30.30/443'; echo "exit=$?"` | expect `exit=124` (timed out) | one blocked attempt | user | the SIEM address | none needed |
| 4 | router | export the log lines | a block entry: source `10.77.20.20`, destination `10.77.30.30`, port 443 | none | router admin | the router | none needed |
| 5 | host | X-03 and X-04 | SYN on the DMZ tap, none on the CORP tap | none | root | the captures | none needed (read-only) |

Expected, evidence and failure meaning as T2, with DMZ as the source. A connection to 443 succeeding means E4 was written too wide.

### 11.5 T4: CORP endpoint to Wazuh, approved telemetry (plan: allowed)

T4a is inside CORP (no router in the path); T4b is the DMZ agent through the router. Preconditions: the SIEM and agents are installed under class D records (week 7); `G9-SIEM-01`'s host firewall allows only the ports read from its own listening sockets; the method of finding an event in the SIEM is recorded by the week 7 author (archives or a test rule); until then the SIEM-side step is NOT RUN and T4 is INCONCLUSIVE.

| # | Where | Command | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| 1 | `G9-SIEM-01` | `sudo ss -tlnp` | the listening ports and their processes: the "actual required ports" | none | root | the guest | none needed (read-only) |
| 2 | `G9-SIEM-01` | `sudo nft list ruleset` (or the firewall tool the guest uses) | default drop inbound; only the ports of step 1 allowed, from named sources | none | root | the guest | none needed (read-only) |
| 3 | `G9-WKS-01` | `eventcreate /T INFORMATION /ID 901 /L APPLICATION /SO G9LAB /D "G9-T4 marker <run id>"` | writes one marker event to the Application log; reports success (ASSUMED tool behaviour) | one log entry that is not removed | local administrator | the workstation | none (record the run id) |
| 4 | `G9-SIEM-01` | search for the marker by the method recorded for week 7 | the marker appears with the source time, the ingestion time and the host id | none | SIEM user | the SIEM | none needed |
| 5 | `G9-WKS-01` | `Test-NetConnection -ComputerName 10.77.30.30 -Port 9200` and the same with `-Port 22` | expect `TcpTestSucceeded : False` for ports the SIEM does not need to expose to the workstation | one blocked attempt each | local user | the SIEM address | none needed |
| 6 | `G9-SIEM-01` | the firewall's log or counters for the window | drops for the step 5 attempts | none | root | the guest | none needed (read-only) |
| 7 (T4b) | `G9-TGT-01` | `timeout 5 bash -c ': < /dev/tcp/10.77.30.30/1514'; echo "exit=$?"` | expect `exit=0` under exception E4 (agent port, ASSUMED default) | one connection | user | the SIEM address | none needed |
| 8 (T4b) | host and router | X-02 on `<tap-dmz>` and `<tap-corp>`; router log export | the connection on both taps with the TTL reduced by one; a pass entry for E4 | capture files | root | the router's taps | stop; keep files |

Expected result: the marker is found in the SIEM with timestamps whose difference is recorded; the required ports are open to the endpoints and no others; the DMZ agent reaches only the agent ports.
Evidence to keep: the source event (log excerpt), the SIEM record, both timestamps, the host id, the firewall rule list, the connection captures and router log, the policy version.
A failure means: a missing marker does not mean no event occurred. Separate the links: the event in the source log, the agent's connection, the manager's ingestion, the search method. A port that is reachable but not required means the SIEM's firewall does not restrict. A DMZ connection to a port other than the agent ports means E4 is too wide.

### 11.6 T5: guest to the household LAN (plan: no authorized route or connectivity)

The owner designates one address on the household LAN that belongs to a device the owner controls (the host's own LAN address is the proposed choice); it is written `<lan-address>`. One probe at most per variant. No other LAN address is touched.

| # | Where | Command | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| 1 | host | X-01 for every guest | NICs only on lab networks | none | libvirt | the lab guests | none needed (read-only) |
| 2 | each Linux guest | X-06 | one zone address; routes only to lab prefixes; no `default` route | none | user | the guest | none needed (read-only) |
| 3 | each Windows guest | `Get-NetIPConfiguration` and `route print -4` | one zone address; no default gateway | none | local user | the guest | none needed (read-only) |
| 4 | host | `sudo nft list ruleset` | rules that drop input and forward for the three lab bridges (section 7) | none | root | the host | none needed (read-only) |
| 5 | host | X-02 on the host's LAN interface and on the VPN interface, filter `net 10.77.0.0/16` | two capture files; expect no packets | capture files | root | those two interfaces | stop; keep files |
| 6 | `G9-ATK-01` | `timeout 5 bash -c ': < /dev/tcp/<lan-address>/<port>'; echo "exit=$?"` | expect failure with no route | one failed attempt | user | `<lan-address>` | none needed |
| 7 (variant: mistaken route) | `G9-ATK-01` | `sudo ip route add default via 10.77.10.1`, then step 6 again, then `sudo ip route del default via 10.77.10.1` | simulates a configuration mistake; expect the router to drop or refuse (it has no route out) and still no packet on the host captures | a temporary route in the guest | root in the guest | the guest's route table | the delete command |
| 8 (variant: on-link host probe) | `G9-ATK-01` | `sudo ip route add <lan-address>/32 dev <zone interface>`, then step 6, then `ip neigh show <lan-address>`, then `sudo ip route del <lan-address>/32 dev <zone interface>` | tests the host-exposure path (section 7): the guest asks for the host's address directly on the bridge. The TCP connection must fail | a temporary route; one failed attempt | root in the guest | the guest and `<lan-address>` | the delete command |

Expected result: no guest has a route toward the LAN; no connection to `<lan-address>` succeeds in any variant; the LAN and VPN captures hold no packet from `10.77.0.0/16`.
Evidence to keep: NIC inventory; each guest's address and route output; the host firewall rule list; the capture files with hashes; the probe outputs with times; the neighbour entry of step 8.
A failure means: any `10.77.0.0/16` packet on the LAN or VPN interface is a leak (STOP-03, B2). A successful TCP connection to the host means the host is reachable from a zone (B2). An ARP answer from the host with the TCP connection still failing is a weak-host behaviour held back only by the firewall: PASS with a recorded residual, and a proposal for the `arp_ignore` control. No route and an immediate error in step 6 is expected, not a failure.

### 11.7 T6: guest to a public network during active training (plan: no route or connection)

No real public address is probed. Route queries send no packets. The only active probe is to an address the Internet standards reserve for documentation (RFC 5737), which no real host holds.

| # | Where | Command | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| 1 | each Linux guest | X-05 with `192.0.2.1` | expect "Network is unreachable" (no default route) | none | user | the guest | none needed (read-only) |
| 2 | each Windows guest | `Find-NetRoute -RemoteIPAddress 192.0.2.1` | the route Windows would use; expect none beyond a lab route | none | local user | the guest | none needed (read-only) |
| 3 | each guest | `resolvectl status` or `cat /etc/resolv.conf` on Linux; `Get-DnsClientServerAddress` on Windows | no public resolver configured | none | user | the guest | none needed (read-only) |
| 4 | router | export of the configuration | no WAN role, no default gateway, no NAT rule | none | router admin | the router | none needed (read-only) |
| 5 | host | X-02 on the uplink and the VPN interface, filter `net 10.77.0.0/16` | capture files; expect no packets | capture files | root | those interfaces | stop; keep files |
| 6 (optional active probe) | `G9-ATK-01` | `ping -c 1 -W 2 192.0.2.1` | expect no reply | one echo request to a documentation address | user | `192.0.2.1` only | none needed |

Expected result: no route off the lab, no resolver outside it, no NAT, and nothing on the host's outside interfaces.
Evidence to keep: the route and resolver outputs per guest; the router export; the capture files with hashes.
A failure means: a route or resolver outside the lab, or a lab-sourced packet on an outside interface, is containment failure (B2). Do not run any other probe to "find out more".

### 11.8 T7: IPv6 bypass attempt (plan: no unintended route)

| # | Where | Command | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| 1 | each Linux guest | `ip -6 addr` and `ip -6 route` | no global address; no default route; at most a link-local address | none | user | the guest | none needed (read-only) |
| 2 | each Windows guest | `Get-NetIPAddress -AddressFamily IPv6` and `Get-NetRoute -AddressFamily IPv6` | same | none | local user | the guest | none needed (read-only) |
| 3 | each Linux guest | `sysctl net.ipv6.conf.all.disable_ipv6` | `1` if the design disables IPv6 | none | user | the guest | none needed (read-only) |
| 4 | router | export of the configuration | IPv6 disabled; no router advertisements | none | router admin | the router | none needed (read-only) |
| 5 | host | `ip -6 addr show dev virbr91` (and 92, 93) and `sysctl net.ipv6.conf.virbr91.disable_ipv6` | no address, or link-local only; the sysctl is `1` if the control of section 7 is applied | none | user | the lab bridges | none needed (read-only) |
| 6 | host | `sudo nft list ruleset` | an IPv6 drop for the lab bridges | none | root | the host | none needed (read-only) |
| 7 | `G9-ATK-01` | `ping -6 -c 1 -W 2 2001:db8::1` | a documentation-prefix address (RFC 3849); expect "Network is unreachable" | one failed attempt | user | that address | none needed |
| 8 | `G9-ATK-01` | `ping -6 -c 1 -W 2 ff02::1%<zone interface>` | link-local all-nodes echo; note which neighbours answer. The host must not | one multicast echo on the zone | user | the zone | none needed |
| 9 | host | X-02 on `<tap-red>` with filter `ip6`, and on the uplink with filter `ip6 and dst net 2001:db8::/32` | capture files; expect nothing for the documentation prefix on the uplink | capture files | root | those interfaces | stop; keep files |

Expected result: no IPv6 route off the zone, no host answer on the bridge, the documentation-prefix packet never leaves.
Evidence to keep: the per-guest address and route outputs; sysctl values; router export; host firewall rules; captures with hashes; the neighbour list from step 8.
A failure means: a global IPv6 address or default route in a guest, a router advertisement, a host answer on a lab bridge, or the test packet on the uplink: B2.

### 11.9 T8: unneeded extra NICs (plan: none)

| # | Where | Command | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| 1 | host | X-01 for each guest | counts and MACs against section 3.1: router 3, `G9-ATK-01` 1, `G9-TGT-01` 1, `G9-DC-01` 1, `G9-WKS-01` 1, `G9-SIEM-01` 1, `G9-MAL-01` 0 | none | libvirt | the lab guests | none needed (read-only) |
| 2 | host | `virsh -c qemu:///system dumpxml --xpath '//interface' <domain>` and the same with `--inactive` | live and persistent interface lists agree (a difference reveals a hot-plugged device) | none | libvirt | one guest at a time | none needed (read-only) |
| 3 | host | `virsh -c qemu:///system dumpxml <domain>` reviewed for `hostdev`, `redirdev`, `filesystem`, `channel`, `sound`, and any `graphics` listener other than a local socket | none present beyond the design | none | libvirt | one guest at a time | none needed (read-only) |
| 4 | host | `ip -br link` | only the expected `virbr91`, `virbr92`, `virbr93` bridges among the existing interfaces; the number of `vnet` taps equals the NIC total of the running stack (8 for the full stack) | none | user | the host | none needed (read-only) |
| 5 | host | `virsh -c qemu:///system net-list --all` | only the three lab networks and any pre-existing one with no lab guest attached | none | libvirt | listing | none needed (read-only) |
| 6 | each guest | `ip -br link` (Linux) or `Get-NetAdapter` (Windows) | only the expected adapters | none | user | the guest | none needed (read-only) |

Expected result: the inventory matches the MAC plan exactly, in the live and persistent definitions, on the host and inside each guest.
Evidence to keep: the outputs and the MAC comparison.
A failure means: any additional NIC, an unknown MAC, a lab guest attached to another network, or a device outside the design is an isolation failure (B2); `G9-MAL-01` with any interface is B7.

### 11.10 T9: restore of a disposable machine (plan: recovers; AD special handling)

Preconditions: baselines exist (section 12). Disposable guests are `G9-TGT-01` first, then `G9-ATK-01`. `G9-DC-01` follows the AD variant, never a plain revert.

| # | Where | Command | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| 1 | guest | `sha256sum <marker file>` and `ls -l <marker file>` | the baseline marker state | none | user | the guest | none needed (read-only) |
| 2 | guest | create and delete synthetic files, and change the marker | a recorded dirty state | changes inside the guest | user | the guest | the restore itself |
| 3 | host | the shutdown and restore commands of section 12 | the guest returns to the baseline | replaces the guest's disk state | libvirt and file access | one guest | the evidence copy of the dirty state, kept first |
| 4 | guest | step 1 again | marker state equals the baseline | none | user | the guest | none needed (read-only) |
| 5 | host and guest | T8 and the T5 to T7 checks for the guest | isolation intact after restore | as those tests | as those tests | the guest | as those tests |
| 6 | host | the time taken, from the commands' timestamps | a recorded restore time | none | user | none | none needed |

AD variant, for `G9-DC-01`: restore by R-AD1 or R-AD2 (section 10), with the AD-05 to AD-09 rows before and after; then a domain logon of a synthetic user on `G9-WKS-01`.
Expected result: the guest reaches the baseline state in a recorded time; isolation checks still pass; the controller answers locator, logon and diagnostic checks and the workstation's secure channel is good.
Evidence to keep: marker hashes before, dirty and after; the restore commands with times; the T8 and T5 to T7 outputs; for the controller the AD rows' outputs.
A failure means: a marker that differs means the baseline was contaminated or the restore was wrong; isolation checks that fail after restore mean the baseline was captured with a faulty configuration (B2) and every guest made from it is suspect; for the controller, any diagnostic failure or a broken secure channel means the method is inadequate: use R-AD1 and treat the restored controller as untrusted.

## 12. Recovery and snapshot plan

### 12.1 Baseline sets

| Set | Taken | Content |
|---|---|---|
| B0 | after the clean installation and the staged tools, powered off, before any intentionally vulnerable software or exercise configuration | per guest, hashed |
| B1 | after the networks, router policy, directory build and SIEM enrollment, with T1 to T9 passed for the stack | the tested set, named with the policy version |
| Wnn | at the start of each week, powered off | the week's working baseline; kept until the week closes |

Names: `<guest id>-<set>-<yyyymmdd>`. The SIEM, the controller and the workstation are captured together as one set.

### 12.2 Method by guest

| Guest | Baseline | Revert | Note |
|---|---|---|---|
| `G9-RTR-01` | cold copy of the disk plus the text export of the configuration | return the copy, or rebuild from the export | the export is the policy version |
| `G9-ATK-01` | cold copy; sessions run on an overlay | discard the overlay | disposable |
| `G9-TGT-01` | cold copy after staging (images pulled, digests recorded); sessions on an overlay | discard the overlay | disposable |
| `G9-DC-01` | cold copy as part of the CORP set | R-AD1 first; R-AD2 tested once; R-AD3 only for the whole set | never reverted alone |
| `G9-WKS-01` | cold copy in the CORP set | with the set | machine-account password pairing |
| `G9-SIEM-01` | cold copy in the CORP set, taken with the indexer stopped | with the set; agents may need re-enrollment | take it cold |
| `G9-MAL-01` | read-only base, overlay per session | discard the overlay | contaminated means discard |

### 12.3 Snapshot facts

Snapshots are not guaranteed instant and are not an AD backup (plan section 1). A snapshot that includes memory writes the guest's whole RAM (6 or 8 GiB for the larger guests) and can take minutes; an offline disk snapshot is quick but needs the guest powered off. Internal snapshots of the controller and the SIEM are not used. External overlays are kept shallow; a long chain is committed or the guest is rebuilt from a cold copy. The method of copying depends on the filesystem (HOST_READINESS, F-22): copy-on-write copies are fast on filesystems that support them, otherwise a full sparse copy is made (open decision O-LA-06).

### 12.4 Proposed commands (class C or D; no approval recorded)

| ID | Command | Target | Purpose and expected output | Side effects | Privilege | Safe scope | Rollback |
|---|---|---|---|---|---|---|---|
| R-01 | `virsh -c qemu:///system shutdown <domain>` then `virsh -c qemu:///system domstate <domain>` | one guest | graceful power-off; the state reads `shut off` | volatile state is lost after power-off | libvirt | one guest | `virsh -c qemu:///system start <domain>` |
| R-02 | `qemu-img info --force-share <image>` | one image file | virtual size, actual size, format and backing file | none | read access to the image | one image | none needed (read-only) |
| R-03 | `sha256sum <image>` | one powered-off image | the image hash, recorded; it reads the whole file, so it takes time | heavy read I/O | read access | one image | none needed (read-only) |
| R-04 | `cp --reflink=auto --sparse=always <image> <baseline path>` | a powered-off guest's image | a cold baseline copy; the guest must be off | uses disk space; the copy shares blocks if the filesystem supports it | read and write access | one image | delete the copy |
| R-05 | `qemu-img create -f qcow2 -b <baseline path> -F qcow2 <overlay path>` | a baseline | a thin overlay for sessions | creates a small file | write access to the directory | one overlay | delete the overlay |
| R-06 | `cp --reflink=auto --sparse=always <baseline path> <active image>` | a powered-off guest | restores the guest to the baseline (replaces the active image) | overwrites the active image: its dirty state is lost | write access | one image, after the dirty state's evidence is copied | none: copy the evidence first |
| R-07 | `du -sh <directory>` | the image directory | the real space used by baselines and overlays | none | read access | one directory | none needed (read-only) |

Optional protection of baselines: marking a baseline file immutable (`chattr +i`) so that nothing writes it by mistake; reversible with `chattr -i`; class C; depends on the filesystem.

### 12.5 Capacity and records

Keep B0, the current B1, and the current week's Wnn; delete older sets only after the weekly review. Record the size of each set (R-07) and each restore time. Evidence of a dirty state is copied before any revert (`docs/curriculum/v2/SAFETY_AUTHORIZATION.md`, section 10).

### 12.6 What is not claimed

No baseline exists. No restore time is known. No statement here says that restoration works.

## 13. Per-week lab stack and the 31 GiB budget

Rules:
1. Simultaneous configured RAM never exceeds 31 GiB. `G9-MAL-01` counts at 8 GiB.
2. The column is an upper bound for the week. A day guide may use fewer guests, never more or different ones without a change here.
3. A guest that is not needed is shut down, not suspended: a suspended guest keeps its memory allocated.
4. At the start of each session: available host memory is at least the stack's RAM plus 8 GiB (P-06, PROPOSED heuristic), and the free space and the tests of the right column are current.
5. Days that use only hosted platforms run no guest.
6. A resize inside the budget is recorded in the lab record.

| Week | Guests at peak | vCPU | RAM GiB | Share of 31 GiB | Configured disk GiB | Tests current before intrusive work | Notes |
|---:|---|---:|---:|---:|---:|---|---|
| 1 | RTR, ATK, TGT | 8 | 11 | 35% | 140 | T8, T6, T7, T5, T1, T9 | the first sub-step (two guests, static addresses) can start with ATK and TGT on separate bridges, with no router |
| 2 | RTR, ATK, TGT | 8 | 11 | 35% | 140 | as week 1 | captures taken on the router's taps |
| 3 | DC, WKS | 4 | 12 | 39% | 150 | T8, T6, T7, T5, T9 (AD variant) | CORP only: no router needed; T2 and T3 wait for ATK and the router |
| 4 | RTR, ATK, TGT | 8 | 11 | 35% | 140 | as week 1 | Docker inside the target; most PortSwigger work is hosted and runs no guest |
| 5 | RTR, ATK, TGT | 8 | 11 | 35% | 140 | as week 1 | crAPI memory fit is not established; the target may be resized inside the budget |
| 6 | RTR, ATK, TGT, DC, WKS | 12 | 23 | 74% | 290 | T1 to T3, T5 to T9 and the retest after the day 29 exception | the plan's core sites for the day 30 midpoint are hosted machines, so no guest is assumed for that day (INFERRED) |
| 7 | RTR, ATK, TGT, DC, WKS, SIEM | 16 | 31 | 100% | 390 | T1 to T9 | days 31 to 33: DC, WKS, SIEM (8 vCPU, 20 GiB, 250 GiB); day 34: RTR, ATK, TGT, SIEM (12 vCPU, 19 GiB, 240 GiB); the full stack only if the day needs it |
| 8 | RTR, ATK, TGT, WKS | 10 | 17 | 55% | 210 | T8, T6, T7, T5, T1, T9 | dataset analysis runs on ATK; Windows artifacts from WKS |
| 9 | MAL, ATK | 8 | 14 | 45% | 140 | T8 (MAL has zero NICs), T9 (overlay discard), T5 to T7 for ATK | no router, no SIEM; MAL never runs with the full stack |
| 10 | RTR, ATK, DC, WKS, SIEM | 14 | 28 | 90% | 350 | T1 to T4, T5 to T9; E3 for directory collection from RED, with removal retest | where BloodHound CE runs is open decision O-LA-08 |
| 11 | RTR, ATK, TGT | 8 | 11 | 35% | 140 | as week 1 | the cloud lab is hosted (no guest); the target may be resized to 10 GiB and 4 vCPU for the Kubernetes lab (10 vCPU, 18 GiB, still inside the budget); the toy agent runs on ATK or on a path chosen under open decision O-05 of `docs/curriculum/v2/SAFETY_AUTHORIZATION.md` |
| 12 | RTR, ATK, TGT, DC, WKS, SIEM | 16 | 31 | 100% | 390 | T1 to T9 | day 57 is a hosted unknown target (no guest); days 58 and 59 are the full instrumented stack; day 60 reproduces recorded results and may need it, as the day guide says |

Full-stack weeks (7 and 12) leave no margin inside the plan's 31 GiB: the host-memory rule (31 plus 8, that is 39 GiB available) applies strictly, and an AI or other heavy workload on the host is not run at the same time. The owner's observation of 55 GiB available is one moment, not a guarantee.

## 14. Departures from the plan

| What | Why |
|---|---|
| DMZ to RED, CORP to RED and CORP to DMZ default to deny, with exceptions E2 and E5 | the plan gives defaults only for RED to DMZ, RED to CORP, DMZ to CORP and CORP to Wazuh; deny by default is the project's rule; reverse-connection and client exercises need a named exception |
| The CORP to Wazuh restriction is enforced on `G9-SIEM-01`, not on the router | the three CORP guests share one bridge, so that traffic never crosses the router (INFERRED from the topology) |
| The malware workspace gets an allocation (4 vCPU, 8 GiB, 60 GiB) and no NIC | the plan names the guest without sizing it; "disconnected adapters" is read as no adapter (PROPOSED) |
| "About 33 GiB uncommitted of nominal 64 GiB" becomes about 29 GiB of the 60 GiB observed | the observed total is used for capacity |
| Guest ids, the MAC plan and domain names are added | the tests and scope lines need stable names and a comparable inventory |
| No host address on the lab bridges, so the operator uses local consoles and reaches the SIEM dashboard from the workstation guest | the plan's example defines no `<ip>` and asks for least host exposure; the consequence is no SSH from the host to a guest |
| No autostart for the lab networks | keeps host exposure to lab sessions |
| IPv6 is disabled or dropped everywhere, on the host bridges too | the plan requires an IPv6 test but not that IPv6 be off; off is the simplest condition to test |
| T4 is split into T4a (inside CORP) and T4b (DMZ agent); T5 adds a mistaken-route variant and an on-link host probe; the execution order is not the plan's table order | the plan's T4 mixes two paths; the host-exposure and misconfiguration cases are the plan's own failure modes; containment tests must precede positive ones |
| The per-week stacks (section 13) are an upper bound | the plan gives one allocation, not a per-week one; where a day guide conflicts, this file is changed or the guide is |
| T4 includes an event marker made with a Windows tool and a SIEM-side search method left to week 7 | the plan asks for source event, SIEM ingestion and timestamps; the SIEM search method depends on a configuration not yet decided |

## 15. Open decisions for the owner

| ID | Decision | Default until decided |
|---|---|---|
| O-LA-01 | The router product (OPNsense or pfSense), after reading the licence and registration terms | not decided |
| O-LA-02 | The router's management path (console only, or a GUI in a maintenance window on one interface from one source) | console only |
| O-LA-03 | How the operator sees the Wazuh dashboard (the browser inside `G9-WKS-01`) | the workstation's browser |
| O-LA-04 | The lab's time policy (host start time and drift recording, or the router as the lab time source) | record drift at each session |
| O-LA-05 | Whether to use libvirt's network filters for anti-spoofing | not used |
| O-LA-06 | Where images live and how baselines are copied, which depends on the filesystem | decided after HOST_READINESS F-22 |
| O-LA-07 | The household LAN address designated for the T5 probe (the host's own LAN address is proposed) | no probe is run |
| O-LA-08 | Where BloodHound CE runs in week 10 (inside `G9-ATK-01`, or another place) and whether its memory fits | not decided |
