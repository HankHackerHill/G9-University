# Week 2 — Networking, protocols and scoped reconnaissance

**Purpose.** Read the network as evidence. By Friday you can say, from a capture and from the hosts themselves, which layer a failure lives in, why a packet took the path it did, what a DNS answer is really saying, which flows a policy allows and which it blocks, and how much a scan result can be trusted. Reconnaissance is taught last and only inside a written scope, because it is the week's authorization lesson: the tool is easy, the permission is the skill.

**Lab stack.** The week 1 lab plus two guests: `lin-aux` (client) and `net-router` (Linux with forwarding and nftables), on two isolated subnets, and a capture point on every link (`tcpdump`, Wireshark or `tshark`). Python 3 with the `ipaddress` module. Optional: scapy for building packets. Snapshot every guest before each lab.

**Competencies.** `net.layers-addressing`, `net.tcp-states`, `net.dns-resolution`, `net.routing-segmentation`, `recon.scope-definition`, `recon.discovery-interpretation`, `net.pcap-analysis`, `net.failure-diagnosis` (pack `networking-protocols-recon`), with `lab.authorization` revisited.

**Accelerator.** A network engineer can skip the subnetting and routing drills of days 7 and 10 after getting them all right from memory in the Monday recall block, and spend the time on the independent labs; days 8, 9 and 11 and the Saturday assessment stay.

## D07 — Layers, addressing and pathways

**Hand in.** Network architecture map — the addressing plan of the lab with the subnet arithmetic shown, the path of one request through every layer and hop, and the capture that proves each claim.

### Objectives
1. Map OSI/TCP-IP layers — for one exchange between two lab guests name each layer's unit of data, address and protocol, and capture one packet that shows the headers of every layer [net.layers-addressing]
2. Explain addressing and subnetting — given a prefix compute network, broadcast, mask, usable range and host count by hand, verify with a tool, and say why two hosts are or are not on-link [net.layers-addressing]
3. Diagram network pathways — draw the path of a packet to a neighbour and across the router, with each hop's address rewriting and the next-hop resolution, and check the diagram against captures [net.layers-addressing]

### Recall (15 min)
Without notes: which seven questions does the evidence completeness checklist ask of every piece of evidence? State your troubleshooting method in six words. What does `ss -tulpn` show that `systemctl status` does not?

### Theory (45 min)
- Encapsulation: an application message becomes a TCP or UDP segment, then an IP packet, then an Ethernet frame. Each layer adds a header that the layer below treats as data. The OSI model is a teaching vocabulary; the Internet protocol suite is what runs.
- An IPv4 prefix splits an address into network and host bits. Two hosts are on-link when their addresses share the sender's prefix; otherwise the sender uses its default gateway. On a link the next hop is found by ARP (IPv6: neighbour discovery) and the frame is rewritten at every router while the IP addresses stay end to end, unless something translates them.
- Private ranges (RFC 1918), link-local addresses and loopback are not routed on the Internet. A /30 has two usable hosts, a /31 is a point-to-point link, a /24 has 254 usable hosts.
- Read RFC 791 and the first sections of RFC 9293, then the Wireshark guide's chapter on capturing.

### Guided lab (120 min)
1. Write the addressing plan before touching a guest: subnet A `10.20.1.0/24` (`lin-target`, `lin-aux`), subnet B `10.20.2.0/24` (a third guest), `net-router` with one address in each.
2. Create the two isolated virtual networks, attach the guests, set addresses with `ip address add`, routes with `ip route add`, enable forwarding on the router (`sysctl net.ipv4.ip_forward=1`). Check with `ip -br address` and `ip route get ADDRESS` on every guest.
3. Capture on both router interfaces while a guest in A fetches a page from the guest in B. For one packet write down: Ethernet source and destination, IP source and destination, TTL, TCP ports, payload. Do it on both interfaces and show that MAC addresses changed, IP addresses did not, and the TTL fell by one.
4. By hand, then with `python3 -c "import ipaddress; ..."`: network, broadcast, usable range and host count of `192.168.10.77/27`, `172.16.5.130/25` and `10.0.0.9/30`.

### Independent lab (120 min)
Design an addressing plan for a pretend office with 60 users, 25 servers and 10 management devices using the smallest prefix that fits each, with room to grow by half. Implement it across the guests, make users able to reach servers and not management using routes only (no firewall yet), prove each with `ip route get` and a capture, then draw the pathway diagram from memory and correct it against the captures.

### Defence and verification (60 min)
Duplicate addresses and neighbour-table tampering break the assumption that an address names one machine. Write a small script that reads `ip neigh` (or a capture) and alerts when one IP address is claimed by a second hardware address. Plant a conflict in the lab (give two guests the same address) and detect it with your script and with `arping -D`; then show the script is silent on the clean network and state one way an attacker on the link could avoid your alert.

### Evidence and report (60 min)
Hand in the map: addressing plan (table), the subnet arithmetic, the annotated captures with file names and SHA-256 values, the path diagram, and a list of every place the diagram and the capture disagreed and how you resolved it. It is accepted when each claim on the diagram points to a capture.

### Resources
- https://www.rfc-editor.org/rfc/rfc791 | External | RFC 791 (September 1981) | IETF document, freely available
- https://www.rfc-editor.org/rfc/rfc9293 | External | RFC 9293 (August 2022) | IETF document, freely available
- https://www.rfc-editor.org/rfc/rfc1918 | External | RFC 1918 (February 1996) | IETF document, freely available
- https://www.wireshark.org/docs/wsug_html_chunked/ | External | current User's Guide; pin the Wireshark release you install | project documentation, ASSUMED

### Watch for
Reading the capture on the wrong interface. Forgetting that a router's two sides see different frames. Treating the TTL as a distance rather than a counter.

## D08 — TCP states and failure interpretation

**Hand in.** Packet analysis report — three annotated captures (a clean connection, a refused one, a lossy one) with the TCP state of each endpoint at every point and what each failure says about the path or the host.

### Objectives
1. Explain TCP states — from a capture label the handshake, data transfer, teardown and reset, and name the state of each endpoint at every point [net.tcp-states]
2. Interpret retransmission/failure PCAP — identify retransmissions, duplicate acknowledgements, a zero window, resets and half-open connections in a capture and say what each indicates about the path or the endpoint [net.tcp-states]
3. Diagnose connectivity in lab — given reports that a service is unreachable, separate an unreachable host, a closed port, a filtered port, a listener on the wrong interface and an application fault using `ss`, a capture and one probe [net.tcp-states]

### Recall (15 min)
Without notes: draw the three guest-to-guest hops of yesterday's path. Which field changes at every router and which does not? Compute the usable host range of `10.20.4.0/26`.

### Theory (45 min)
- The states are LISTEN, SYN-SENT, SYN-RECEIVED, ESTABLISHED, FIN-WAIT-1 and -2, CLOSE-WAIT, LAST-ACK, CLOSING and TIME-WAIT. Sequence and acknowledgement numbers count bytes; the window limits what is in flight.
- What a refusal looks like depends on who answers: a closed port answers a SYN with a reset; a firewall that drops answers nothing, so the sender retransmits its SYN with growing delay and then gives up; a firewall that rejects answers with a reset or an ICMP error.
- Loss shows as retransmissions after a timeout, or as a fast retransmit after three duplicate acknowledgements; a zero window means the receiver's buffer is full, which is the application's problem, not the network's.
- Read the TCP sections of RFC 9293 on the state diagram and on retransmission.

### Guided lab (120 min)
1. Run a listener on `lin-target` (`python3 -m http.server 8080`). Capture a full request from `lin-aux`, with display filter `tcp.port == 8080`. Label every packet with the state of both ends.
2. Make the port closed (stop the listener) and capture the refusal. Then make it filtered with an nftables rule that drops, and again with one that rejects; capture each and note the difference in what the client sees and how long it takes.
3. Bind the listener to `127.0.0.1` only and show from `lin-aux` what that looks like, and how `ss -tln` on the target shows the cause.
4. Add loss and delay on the target's interface (`tc qdisc add dev IF root netem loss 5% delay 80ms`) and capture a transfer. Find the retransmissions with `tcp.analysis.retransmission` and a duplicate-acknowledgement sequence.

### Independent lab (120 min)
Prepare three faults of different kinds on three clones (a service bound to the wrong interface, a drop rule on one port, a listener that accepts and never replies). Draw one at random and diagnose it from the symptom ("the page does not load") using at most a client, `ss` and one capture. Write the order in which you ruled hypotheses out. Repeat with a second fault and compare which instrument settled each.

### Defence and verification (60 min)
Rate-limit new connections to the service with an nftables rule (for example `ct state new limit rate 20/second accept`, then drop the rest) and prove two things: a burst from `lin-aux` hits the limit (the counter moves, some connections fail) and an ordinary client below the limit is unaffected. State what the rule does not protect against.

### Evidence and report (60 min)
Hand in three annotated captures with the endpoint state at each step, the table of failure signatures (closed, dropped, rejected, loopback-only, lossy, application stalled) with the observation that identifies each, and the diagnosis logs of the independent lab. It is accepted when each failure signature is shown in a capture of your own.

### Resources
- https://www.rfc-editor.org/rfc/rfc9293 | External | RFC 9293 (August 2022) | IETF document, freely available
- https://www.rfc-editor.org/rfc/rfc5681 | External | RFC 5681 (September 2009) | IETF document, freely available
- https://man7.org/linux/man-pages/man8/ss.8.html | External | current man-pages project | freely readable, ASSUMED

### Watch for
Concluding "the firewall blocks it" from silence alone: silence also means loss or a dead host. Comparing a client-side and a server-side capture without allowing for clock difference.

## D09 — DNS resolution, caching and failure

**Hand in.** DNS investigation — the resolution of one name traced hop by hop, the effect of caching shown with timestamps, and the diagnosis of a planted defect.

### Objectives
1. Trace recursive/authoritative DNS — follow a resolution from the stub resolver through the recursive resolver to the authoritative server and explain each referral, record type and TTL [net.dns-resolution]
2. Analyze resolution cache and failure — show how TTLs and negative caching change answers over time and read the fields of the DNS messages in a capture (identifier, flags, response code, sections) [net.dns-resolution, net.pcap-analysis]
3. Diagnose test DNS defect — find a planted fault (missing record, wrong glue, lame delegation, a TTL that is too long, a resolver pointing at the wrong server) from the symptom to the cause [net.dns-resolution]

### Recall (15 min)
Without notes: what does a closed port, a dropped port and a rejected port each look like to the client? What does `ss -tln` show for a service bound to `127.0.0.1`? Name two TCP states that persist after a connection ends.

### Theory (45 min)
- The stub resolver on a host asks a recursive resolver, which walks the hierarchy (root, top-level domain, the zone's authoritative servers), caches what it learns for each record's TTL, and caches failures for a shorter time (negative caching).
- A referral is not an answer: it points to the next servers. Glue records give addresses for name servers inside the zone they serve. A delegation that points at a server that does not answer for the zone is lame.
- Message fields to read: query identifier, flags (QR, AA, RD, RA, TC), response code (NOERROR, NXDOMAIN, SERVFAIL, REFUSED) and the answer, authority and additional sections. `.test` is reserved for testing and never exists in the public DNS.
- Read RFC 1034 and RFC 1035 on resolution, and RFC 2308 on negative caching.

### Guided lab (120 min)
1. On one guest run an authoritative server for the zone `lab.test` (BIND or NSD) and on another a recursive resolver (Unbound) configured to ask it for that zone. Point a client at the recursive resolver.
2. Walk the resolution by hand with `dig +norecurse` against each server, noting flags, TTL and sections. Capture the whole exchange and annotate it.
3. Resolve the same name twice, a few seconds apart, and show the TTL falling in the cached answer. Query a name that does not exist twice and show the negative answer's lifetime.
4. Change a record on the authoritative server without changing its serial, then with a new serial, and describe what the recursive resolver and a fresh client see over the next minutes.

### Independent lab (120 min)
Prepare three faults (a delegation to a server that is down, a missing glue record, a client configured with a resolver that has no route). Draw one at random and find it using `dig`, a capture and the server logs only. Record the sequence of tests. Then explain in writing why the same user-visible symptom, "the name does not resolve", has at least four distinct causes and which test separates them.

### Defence and verification (60 min)
Restrict the guests so that only the internal recursive resolver may send DNS traffic (drop outbound port 53 from every other source) and turn on query logging on the resolver. Write a script that flags names with unusually long labels or unusually many distinct subdomains of one parent in a window. Test it with synthetic names that mimic a tunnel and with normal traffic, record false positives, and state what a patient adversary can do to stay under your thresholds.

### Evidence and report (60 min)
Hand in the DNS investigation: the traced resolution with message fields, the TTL evidence, the diagnosis log of the fault you drew and a table of "does not resolve" causes with the test that identifies each. It is accepted when every claim names a capture or log line.

### Resources
- https://www.rfc-editor.org/rfc/rfc1034 | External | RFC 1034 (November 1987) | IETF document, freely available
- https://www.rfc-editor.org/rfc/rfc1035 | External | RFC 1035 (November 1987) | IETF document, freely available
- https://www.rfc-editor.org/rfc/rfc2308 | External | RFC 2308 (March 1998) | IETF document, freely available
- https://www.rfc-editor.org/rfc/rfc6761 | External | RFC 6761 (February 2013) | IETF document, freely available

### Watch for
Forgetting that the client's own cache and the resolver's cache both exist. Editing a zone and not raising its serial. Testing against the wrong server because `/etc/resolv.conf` was rewritten.

## D10 — Routing, NAT and segmentation

**Hand in.** Route/control report — the routing decisions explained, the firewall and NAT policy read and predicted, and a segmentation matrix with every allowed and denied flow proven.

### Objectives
1. Explain route selection — for a routing table with overlapping prefixes predict which route is chosen (longest prefix, then metric) and verify with `ip route get` [net.routing-segmentation]
2. Examine NAT/firewall policy — read an nftables ruleset and a NAT configuration, predict what happens to given packets (accepted, dropped, rewritten) and confirm with captures [net.routing-segmentation]
3. Verify segmentation controls — write the intended policy between zones as a matrix, implement it with default deny, and prove every allowed and every denied flow with a test and a capture [net.routing-segmentation]

### Recall (15 min)
Without notes: in a DNS answer, what does the AA flag mean and what do the authority and additional sections hold? What distinguishes NXDOMAIN from SERVFAIL? How long does a negative answer live?

### Theory (45 min)
- Routing is a lookup: the most specific matching prefix wins; ties are broken by metric; a default route matches everything. Return paths matter: a packet that arrives but whose reply has no route is a common "firewall" mystery.
- A stateful firewall tracks connections: replies to allowed outbound connections are accepted by state, so rules describe who may start a conversation. Order matters: the first matching rule decides, so a broad accept above a narrow drop defeats it.
- NAT rewrites addresses and ports and keeps a table to undo it. It changes what a capture shows on each side and is not a security boundary by itself.
- Segmentation means that every path between zones is listed and each is allowed or denied on purpose. Default deny with explicit allows is testable; default allow with exceptions is not.

### Guided lab (120 min)
1. On `net-router` set up three zones (users, servers, management) as separate subnets.
2. Write the matrix first: which zone may start which connection to which port in which zone; everything else is denied.
3. Implement it in an nftables `forward` chain with policy drop, a rule accepting established and related traffic, one rule per allowed flow, and counters. Add source NAT for one zone going to an "outside" address and look at the entries with `conntrack -L`.
4. Prove every cell of the matrix with a connection attempt and read the counters; capture one allowed and one denied flow on both router interfaces and show the NAT rewrite.

### Independent lab (120 min)
Add a fourth zone (a DMZ) and extend the matrix. Prepare three flawed rulesets for it (an accept above a drop, a reply path with no route, a NAT rule that also rewrites management traffic). Draw one at random, predict its behaviour by reading it, then test your prediction and fix it with the smallest change.

### Defence and verification (60 min)
Turn the matrix into an automated test: a script that tries every (source zone, destination zone, port) combination, compares the outcome with the matrix and prints a difference report. Run it on the correct ruleset, then change one rule and show that the script catches it. State what the test cannot show (for example paths through a host that bridges two zones).

### Evidence and report (60 min)
Hand in the matrix (intended and observed), the ruleset with its hash, the counters, the two annotated captures, and the output of the test script before and after the planted change. It is accepted when every cell has an observed result and every difference is explained.

### Resources
- https://wiki.nftables.org/wiki-nftables/index.php/Main_Page | External | current wiki; pin the nftables version of your router guest | project wiki, ASSUMED
- https://man7.org/linux/man-pages/man8/ip-route.8.html | External | current man-pages project | freely readable, ASSUMED
- https://www.rfc-editor.org/rfc/rfc4787 | External | RFC 4787 (January 2007) | IETF document, freely available

### Watch for
A rule that matches nothing because the interface name is wrong. Testing from the router itself, which bypasses the forward chain. Calling a flat test "segmentation verified".

## D11 — Scoped reconnaissance

**Hand in.** Scoped recon report — the written scope, the method with exact commands, a results table with a confidence for each line, and a ranked list of risks with the evidence behind each.

### Objectives
1. Define reconnaissance scope — write the rules for a reconnaissance exercise on the lab: target list, permitted techniques and rate, forbidden actions, time window, data handling and stop condition, each tied to the day 1 authorization [recon.scope-definition, lab.authorization]
2. Interpret discovery results — run host discovery and service and version detection against the lab subnets inside the scope and explain each result (open, closed, filtered, banner, operating-system guess) with its confidence and what could make it wrong [recon.discovery-interpretation]
3. Translate observations into risks — turn the findings into a ranked list of risks with evidence, likelihood and impact reasoning, and say which findings are not yet risks [recon.discovery-interpretation]

### Recall (15 min)
Without notes: what makes a zone boundary testable? Write the nftables ruleset order rule in one sentence. What does source NAT change in a capture on each side of the router?

### Theory (45 min)
- Scope comes before the tool: who authorized what, which addresses and ports, which techniques (passive observation, host discovery, port scanning, version detection), at what rate, in what window, and what stops the exercise. A scan outside scope is an incident, not a mistake.
- Host discovery and port scanning are inferences from replies. A port is open if something answers, closed if a reset comes back, filtered if nothing useful does. Version detection and operating-system guesses are fingerprints: they compare observed behaviour with a database and can be wrong or spoofed.
- Scans have side effects: they fill logs, trip alarms and can crash fragile services. Rate and timing are part of the scope.
- Read the nmap reference guide on scan types, port states and timing; note which options need privileges.

### Guided lab (120 min)
1. Write the scope document for the three lab subnets and sign it as owner. It lists targets, allowed techniques, a maximum rate, a time window, the evidence folder and the stop condition.
2. Within it: host discovery (`nmap -sn`), a full TCP port scan of one host (`-sS -p-`), then service and version detection on the open ports (`-sV`), saving all output formats (`-oA`). Capture the scan traffic from the target side.
3. Compare the scan results with ground truth from `ss -tulpn` on each target. List every difference (a filtered port you know is open, a banner that names the wrong version) and explain it.
4. Show in the capture what the scanner sent for one open, one closed and one filtered port.

### Independent lab (120 min)
Prepare three surprises on clones (a service on an unusual port, a port filtered by a firewall rule, a banner configured to lie) and a decoy host. Draw a random configuration, scan it within your scope, and reconstruct the real service map with a confidence for every line. Then reveal the truth and score yourself: which lines were wrong and which instrument would have corrected them.

### Defence and verification (60 min)
Write a detection for the scan you just ran: from a capture or firewall log count distinct destination ports and hosts per source in a short window and alert above a threshold. Test it against a full port scan, a slow scan at a low rate and ordinary browsing; record what it catches and misses and tune it. State plainly what a slow scan does to your threshold.

### Evidence and report (60 min)
Hand in the report: scope; method with every command; the results table with confidence and reasoning; the comparison with ground truth; the ranked risks (an open management port is a risk, an open web port is a fact until you know what it serves); limitations. It is accepted when each risk cites an observation and each unsure result says what would settle it.

### Resources
- https://nmap.org/book/man.html | External | current reference guide; pin the nmap release you run | project documentation, ASSUMED
- https://csrc.nist.gov/pubs/sp/800/115/final | External | SP 800-115 (September 2008) | NIST publication, freely available
- https://man7.org/linux/man-pages/man8/tcpdump.8.html | External | current man-pages project | freely readable, ASSUMED

### Watch for
Scanning a range that is wider than your scope because of a typo in the prefix. Reporting a guess as a finding. Reporting a risk without the observation that supports it.

## D12 — Week 2 assessment (blind practical)

**Hand in.** Week 2 assessment — the report of the blind practical: the failure diagnosed, the evidence from the captures and hosts, and the conclusion with its limits.

### Objectives
1. Analyze unfamiliar PCAP — read a capture you have not seen and describe the conversations, the protocols and anything abnormal, with packet numbers [net.pcap-analysis]
2. Diagnose network failure — identify the layer and the cause of a failure in an unfamiliar lab network from captures, host state and configuration [net.failure-diagnosis]
3. Write evidence-backed conclusion — state the diagnosis, the evidence for it, the alternatives excluded and the confidence in a short report [net.failure-diagnosis]

### Blind practical (120 min)
Open the assessment in the application, start the attempt and read the task statement. Take the scenario from the source you fixed before the week; do not look at the captures or the configuration before the clock starts. Work from your method and your notes. Stop at 120 minutes.

### Remediation (60 min)
Grade yourself against the rubric criteria before reading any answer key. For each criterion below the level you wanted, find the cause in your method (what you did not check, what you assumed), repair it with a short targeted exercise (for example reading one more capture of that failure type) and redo the failed part on a fresh scenario.

### Report (60 min)
Write the report in the structure of the task statement: summary, per-finding diagnosis with packet numbers and command output, alternatives excluded, the repair or recommendation, confidence and limits. Attach the captures with their SHA-256 values.

### Performance review (60 min)
Compare the grade with the week's targets: independent execution, layer identified before cause, evidence cited by packet number, alternatives excluded, honest confidence. Record the habit that cost you most and the one change you will make to your method next week.

### Assessment
- Code: W02-BLIND
- Title: Week 2 blind practical: unfamiliar capture and network failure diagnosis
- Kind: BLIND_PRACTICAL
- Competency: net.failure-diagnosis
- Difficulty: 3

Task statement:
You have 120 minutes. You are given one or more packet captures you have not read, a short description of a small lab network (zones, addresses, services) and a report from a user that something does not work. Work only inside the lab and on the objects of your scope statement.

Scenario source (fixed before the week begins, not inspected): captures and configurations prepared by another person from the catalogue below, or, working alone, unseen captures chosen at random from a public collection of sample captures together with a failure you did not design, or a lab network faulted by a script whose log you have not read. Failures are drawn from: a routing fault (missing or wrong route, no return path); a firewall rule that drops or rejects the wrong flow; a name resolution fault (wrong server, missing record, negative caching); a transport fault (loss, retransmission storm, zero window, reset from a middlebox); an addressing fault (duplicate address, wrong mask, wrong gateway); a service bound to the wrong interface.

Deliverables, in this order. First, a description of the capture: conversations, protocols and anything abnormal, with packet numbers. Second, the layer at which the failure lives and why, with the evidence. Third, the cause, the alternatives you excluded and how, and your confidence. Fourth, the smallest change that repairs it and how you would verify it. Fifth, a report of at most three pages with the capture hashes.

You are assessed on: independent execution during the block; the layer identified before the cause; conclusions that cite packet numbers or command output; alternatives excluded rather than ignored; honest confidence and limits. The pass rule is non-compensatory: every criterion must reach the pass level.

### Resources
- https://wiki.wireshark.org/SampleCaptures | External | current sample capture collection; pin the capture file names you use | project wiki, licence of each capture to be checked, ASSUMED

### Watch for
Reading the capture before deciding what question it answers. Stopping at the first abnormal packet. A conclusion with no packet number.
