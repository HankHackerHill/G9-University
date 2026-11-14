# Week 8 — DFIR, malware and hunting

**Purpose.** When detection fires, someone has to decide what happened, how bad it is and what to do, using evidence that can be trusted. This week covers the whole path on lab systems: scoping and severity, preserving evidence so that it survives scrutiny, reading host artifacts and network captures, building a timeline in which every statement is labelled, triaging a suspicious program by static means only, and hunting through telemetry for what no rule has found. The Saturday assessment gives you an incident dataset you have not seen and asks for triage, a timeline and a recovery recommendation.

**Lab stack.** The week 7 lab plus acquisition and analysis tools: disk and memory images taken from lab guests (the hypervisor can dump a guest's memory and copy its disk image), the Sleuth Kit or Autopsy, Plaso, Volatility 3, Zeek and Wireshark, YARA, `capa`, Python with `pefile`, and your own benign fixtures. Malware work is **static only**, on fixtures you compile yourself or that a training source provides as safe. Do not execute an unknown program. A real sample is handled only in a disposable guest with no network and a snapshot, after you have decided in writing to do so.

**Competencies.** `ir.scoping-triage`, `dfir.chain-of-custody`, `dfir.host-artifacts`, `dfir.network-evidence`, `malware.static-triage`, `hunt.approved-telemetry`, `ir.recovery-recommendation`, `dfir.blind-incident` (pack `dfir-forensics-malware`), with `hunt.hypothesis`, `det.correlation-timeline`, `net.pcap-analysis`, `det.sigma-rules` and `linux.evidence-completeness` revisited.

**Accelerator.** An incident responder who can state the order of volatility and the custody record fields from memory in the Monday recall block can treat the acquisition half of day 43 as review; days 44 to 47 and the Saturday assessment stay.

## D43 — Incident scoping, severity and evidence preservation

**Hand in.** IR triage report — the scoping questions and answers, the severity with its criteria, the custody record and the preserved evidence with hashes, and the triage plan with decision points.

### Objectives
1. Scope incident and severity — from an initial report define what is affected, what is known and unknown, a severity with stated criteria, and the first questions and decisions [ir.scoping-triage]
2. Preserve evidence — collect and preserve volatile and non-volatile evidence from a lab host in a defensible order, with hashes and a custody record, changing what you collect as little as you can [dfir.chain-of-custody]
3. Draft triage plan — write a plan with priorities, owners, time boxes, containment options and their risks, and the decision points [ir.scoping-triage]

### Recall (15 min)
Without notes: which two hunts did you run last week and what did each find? What does a null result tell the reader that "nothing found" does not? Name three limits of the false-positive estimate you made.

### Theory (45 min)
- Incident handling is a cycle: prepare, detect and analyse, contain and eradicate and recover, then learn. The national standard for incident handling (NIST SP 800-61) describes the activities and, in its current revision, aligns them to the cybersecurity framework's functions. Read it for vocabulary; the plan is yours.
- Scoping asks: what is the report, what systems and data are involved, what is the business effect, what is known, what is assumed, who decides containment, who must be told and by when. Severity needs criteria written before the incident (impact on confidentiality, integrity and availability, scope, data class, time criticality); the number is a decision aid, not a fact.
- Evidence is collected in order of volatility (RFC 3227): processor state and memory first, then network state and running processes, then disk, then remote logs. Every action changes the system; record what you did, when, with what tool and its version, and hash everything you keep. Work on copies, keep originals read-only, and say what you did not collect.
- A custody record says who held what, when, where and what they did with it, so that a stranger can believe the evidence is what you say it is.

### Guided lab (120 min)
1. Run a scenario on two clones (one Linux, one Windows) that a helper prepared: a user reports unexpected behaviour. Write the scoping questions you would ask, the answers you have, and the assumptions you are making. Choose a severity with criteria.
2. Collect from the Linux clone: running processes, sockets, logged-on users and the time with its source; then a memory image through the hypervisor, then a copy of the disk image. From the Windows clone: a memory dump through the hypervisor, then the disk image. Hash every item at once and again after copying.
3. Keep a custody record as you work (what, when in UTC, who, how, hash).
4. Write the triage plan: first hour, first day, containment options with risks (isolate, snapshot, stop, rebuild), decision points and who decides.

### Independent lab (120 min)
A second scenario is prepared on a different clone from a list you wrote. Without reading the list, within sixty minutes write the scoping record, severity and plan, and within the rest of the block preserve the evidence with a custody record. Then list every assumption you made and what would have disproved it.

### Defence and verification (60 min)
Build an evidence intake tool: it hashes a file, stores it read-only, and appends a record (what, when, who, hash, previous record's hash) to a custody log whose entries link to each other. A second command verifies the whole log and every stored file. Tamper with a stored file and with a log entry and show the verification finds both; state what the log cannot prove (that the evidence was genuine before you took it).

### Evidence and report (60 min)
Hand in the scoping record, the severity criteria and result, the custody log with its verification output, the hash list, and the plan. It is accepted when every preserved item has a hash taken at collection and verified after copying, and every assumption is written down.

### Resources
- https://csrc.nist.gov/pubs/sp/800/61/r3/final | External | SP 800-61, the current revision; note which revision you read (an earlier revision is Rev. 2, August 2012) | NIST publication, freely available, ASSUMED
- https://www.rfc-editor.org/rfc/rfc3227 | External | RFC 3227 (February 2002) | IETF document, freely available
- https://csrc.nist.gov/pubs/sp/800/86/final | External | SP 800-86 (August 2006) | NIST publication, freely available

### Watch for
Rebooting the clone before taking memory. Hashing after copying only. A severity chosen before the scoping questions were answered.

## D44 — Host artifacts and the event timeline

**Hand in.** Host forensic timeline — the artifacts interpreted from the Windows and Linux images, the merged timeline in UTC, and a statement of what each source can and cannot prove with the custody assumptions behind each conclusion.

### Objectives
1. Analyze host artifacts — extract and interpret key artifacts from Windows and Linux images (evidence of execution, persistence, account activity, file system timestamps, logs) with a tool and by hand [dfir.host-artifacts]
2. Construct event timeline — merge artifacts into one timeline in UTC, normalise timestamps with their exact semantics (modified, accessed, changed, created) and mark what each source can and cannot prove [dfir.host-artifacts, det.correlation-timeline]
3. Explain chain-of-custody assumptions — state what each conclusion assumes about the integrity of the evidence (acquisition method, tool, clock, hash) and what would invalidate it [dfir.chain-of-custody]

### Recall (15 min)
Without notes: list the order of volatility from most to least volatile. Write the fields of a custody record. What is the difference between a severity and a priority?

### Theory (45 min)
- Artifacts are by-products of use: on Windows the execution records (prefetch files, application compatibility and amcache entries), registry hives, shell and jump-list records, the master file table and the change journal, event logs; on Linux the authentication and system logs, the login records, shell history, scheduled jobs, package manager logs, file timestamps, `~/.ssh`. Each records something different and is kept for a different time.
- File timestamps have meanings that depend on the file system and mount options. Modified and changed are not created; access times may not be updated at all; a copy keeps or resets times depending on the tool. Some systems store two sets of times for the same file; differences between them are a clue to time manipulation, not proof of it.
- A timeline merges sources into one clock. Normalise to UTC, keep the original text, note each source's resolution and its known skew, and do not conclude from one source what another could contradict.
- Read the documentation of the timeline and file system tools you use, and the sections of SP 800-86 on data collection and examination.

### Guided lab (120 min)
1. Mount the images read-only with the access times left alone. Record the command that makes the mount read-only and show that a write is refused.
2. On the Linux image extract logs, history, scheduled jobs, `~/.ssh`, package logs and the listing of recently changed files. On the Windows image extract execution evidence, the registry hives, event logs and the file system metadata with the tools of your choice.
3. Build a super-timeline with Plaso for each image, filter it to the scenario's window, and verify five entries by reading the artifact itself.
4. For three timestamps explain exactly what the time means (which attribute, which file system, which tool) and what would make it wrong.

### Independent lab (120 min)
Restore a second prepared image pair and build the timeline within ninety minutes. Mark each entry's source and resolution, flag every place where two sources disagree and say which you trust and why. Then read the helper's script and mark what your timeline missed and what it invented.

### Defence and verification (60 min)
Assess forensic readiness: list which artifacts your lab would no longer hold ninety days from now (log rotation, disabled history, solid-state trim, short event log sizes) and what each loss would cost an investigator. Implement three improvements (longer retention, command-line auditing, forwarding to the collector), then rerun the scenario and show the evidence of the improved items.

### Evidence and report (60 min)
Hand in the extracted artifacts with their sources, the merged timeline with resolution and skew notes, the verified entries, the three timestamp explanations, the assumptions list and the readiness changes with their proof. It is accepted when every timeline entry names its artifact and every conclusion lists what it assumes.

### Resources
- https://www.sleuthkit.org/ | External | The Sleuth Kit and Autopsy, current release; pin the version you run | open source project, ASSUMED
- https://plaso.readthedocs.io/ | External | current documentation; pin the version you run | open source project, ASSUMED
- https://volatility3.readthedocs.io/ | External | current documentation; pin the version you run | open source project, ASSUMED

### Watch for
Mounting read-write and changing the evidence. Treating a file's modified time as the time of an action. A timeline in local time with no zone.

## D45 — Network evidence

**Hand in.** Network forensic report — the suspicious conversations in a capture described with statistics, their link to host processes and users with the strength of each link, and the gaps with what would close them.

### Objectives
1. Analyze suspicious PCAP — read a capture for a suspicious conversation (a regular beacon, an odd name lookup, a large upload, an unusual protocol use) and describe it with flows and statistics [dfir.network-evidence, net.pcap-analysis]
2. Correlate network and host evidence — tie flows to host processes and users using time, ports and host logs, and state the strength of each link [dfir.network-evidence, dfir.host-artifacts]
3. Identify gaps — state what the capture cannot show (encryption, sampling, missing segments, address translation) and what other evidence would close each gap [dfir.network-evidence, linux.evidence-completeness]

### Recall (15 min)
Without notes: name the artifact that shows a program ran and one that shows only that it existed. What does the modified time of a file tell you and what does it not? Where do you record the clock error of an image?

### Theory (45 min)
- A capture is a set of packets; a flow log (as Zeek writes) is a summary of conversations. Start from the summary: who talked to whom, how much, how often, for how long. Then read the packets that matter.
- Suspicious patterns are statistical before they are specific: connections at near-regular intervals (a beacon, with or without random delay), a few very large outbound transfers, names that look generated, a protocol on an unexpected port, a certificate that does not match its name. Each has benign look-alikes (software updates, time synchronisation, monitoring checks).
- Encrypted traffic still carries metadata: addresses, sizes, timing, the server name in the handshake, certificate fields. It does not carry content; say so.
- A flow becomes evidence about a host only when it is tied to the host: by the address and port at the time, by the host's own records of the connection, by the process that owned the socket. The link has a strength; write it down.

### Guided lab (120 min)
1. Take the capture of the scenario's network from the router and run Zeek over it. Read the connection, name lookup, web and certificate logs. Use Wireshark's conversation and endpoint statistics and the I/O graph.
2. Compute inter-arrival times for each destination with a short Python script and find the one with the most regular intervals. Check the lookups preceding it.
3. Tie the flow to a host process using the Windows guest's connection events and the Linux guest's socket snapshot taken at the time. State the link's strength (time-only, address and port, process-owned).
4. List what you could not see and what would close each gap.

### Independent lab (120 min)
Analyse a capture you have not seen: choose one from a public source of training captures, or a scenario recorded by a helper. Examine flows only: do not extract files from captures of real malware. Describe the three most suspicious conversations with their statistics and the benign explanations you excluded.

### Defence and verification (60 min)
Write a beacon detector over the connection logs: group by source and destination, compute the regularity of intervals (for example the spread relative to the mean) and alert on regular, long-lived patterns. Test it with a synthetic beacon with and without random delay, with periodic benign traffic (time synchronisation, updates) as false-positive tests, and record the thresholds. State what an adversary can do to hide from it.

### Evidence and report (60 min)
Hand in the flow statistics, the three conversations with reasoning, the host links with their strength, the detector with its test results and the gap list. It is accepted when every link is labelled with its strength and every gap names the evidence that would close it.

### Resources
- https://docs.zeek.org/ | External | current documentation; pin the Zeek version you run | open source project, ASSUMED
- https://www.wireshark.org/docs/wsug_html_chunked/ | External | current User's Guide; pin the Wireshark release you install | project documentation, ASSUMED
- https://www.malware-traffic-analysis.net/ | External | training exercises as of the day you use them; note the exercise | site terms apply; captures may contain real malware traffic, analyse flows only, ASSUMED

### Watch for
Taking a regular interval as proof: updates are regular too. Linking a flow to a process by time alone. Opening files carried in a capture of real malware.

## D46 — Static triage of a safe malware fixture

**Hand in.** Malware triage — the static characteristics of one fixture recorded with the commands that produced them, a rule that identifies it, its false-positive test, and the behaviour inferred from static evidence with a confidence for each statement.

### Objectives
1. Inspect safe static malware fixture — examine a benign fixture built to look suspicious without running it (type, hashes, strings, headers, imports, sections, entropy, embedded data, signing) [malware.static-triage]
2. Record static characteristics — record every characteristic with the command, tool version and output, in a form another analyst can reproduce [malware.static-triage]
3. Distinguish behavior from static inference — separate what the file can be observed to contain from what you infer it would do, label each, and say what a dynamic analysis would add and what it would risk [malware.static-triage]

### Recall (15 min)
Without notes: what makes a network link between a flow and a process strong or weak? Name two benign things that beacon. What does an encrypted flow still reveal?

### Theory (45 min)
- Static analysis examines a file without running it. It is safe by construction and limited by construction: it shows what is present (strings, imports, structure, embedded data), not what happens. Packing, encryption and obfuscation hide content from static views and are themselves evidence, usually of intent to hide.
- Identify with hashes (use SHA-256; short hashes collide), file type by content and not by name, headers and sections, the functions the file imports, readable strings, embedded resources and the entropy of each section (a very high value suggests compressed or encrypted data). Tools such as `capa` describe capabilities that follow from the imports and code patterns; they are inferences.
- A detection rule for a file (YARA) names byte patterns and conditions. It must be specific enough not to match benign programs and tested against a corpus of them.
- Handling rules: never execute an unknown program on a host you care about; keep samples renamed and in an archive with a clear marker; do not run anything even in the lab without a written decision for that sample.

### Guided lab (120 min)
1. Build three benign fixtures yourself: a small compiled program that references network functions and carries an embedded address and an encoded string but only prints a message; a program with a large high-entropy data section; a script with encoded-looking but harmless strings. Do not run them.
2. Triage one of them (chosen by your helper) as if it were unknown: file type, SHA-256, strings, headers and sections, imports, entropy, embedded data, any signature. Record each command with its tool version and the output.
3. Run `capa` and read what it infers and from what. Write what you would expect the file to do, separately from what you observed it contains.
4. Write a YARA rule that identifies the fixture by two or more independent features.

### Independent lab (120 min)
Have a helper or a script prepare three fixtures, one of them a decoy that looks suspicious and is plain, and give you one without saying which. Triage it blind in ninety minutes and write the report with a confidence for each statement. Then compare with what the fixture was built to do and record which inferences were wrong and why.

### Defence and verification (60 min)
Test your rule for false positives against a corpus of benign programs (the system binaries of your guests and a set of open source programs) and record any match. Tighten the rule until it has none while still matching the fixture and a recompiled variant of it. Record the variants it misses (a different compiler setting, a changed string) and say what that means for how long the rule would stay useful.

### Evidence and report (60 min)
Hand in the triage record with commands and tool versions, the inferred behaviour with confidence labels, the rule, the false-positive test and the list of variants it misses. It is accepted when each characteristic is reproducible from its recorded command and each behavioural statement is labelled observed, inferred or assumed.

### Resources
- https://yara.readthedocs.io/ | External | current documentation; pin the YARA version you run | open source project, ASSUMED
- https://github.com/mandiant/capa | External | current release; pin the version you run | open source project, ASSUMED
- https://github.com/erocarrera/pefile | External | current release; pin the version you install | open source project, ASSUMED

### Watch for
Running the fixture "just to see". Reading a capability list as a behaviour. A rule built on one string.

## D47 — Hypothesis-driven hunting

**Hand in.** Hunt report — two hunts, each with hypothesis, data, queries, volume examined, exclusions with reasons, findings (including none) and confidence, and one repeatable finding converted into a tested rule.

### Objectives
1. Define hunt hypothesis — write a hunt hypothesis from a threat behaviour, an intelligence report or a gap in your coverage, with the data it needs, the normal pattern expected and the criteria for stopping [hunt.hypothesis]
2. Examine approved telemetry — run the hunt over the approved telemetry (your store and datasets) and record the queries, the volume examined and what you excluded and why [hunt.approved-telemetry]
3. Record findings/detections — document the findings (including none), the confidence, and convert any repeatable finding into a rule with fixtures [hunt.approved-telemetry, det.sigma-rules]

### Recall (15 min)
Without notes: list five static characteristics you recorded yesterday and the command for each. Which of your statements were inferred? What would a dynamic analysis have added and what would it have risked?

### Theory (45 min)
- A hunt looks for activity that no rule has caught. It starts from a hypothesis that is specific and testable, names the data it needs, and says what normal looks like. It ends with a finding, an improvement (a new rule, a new source) or a recorded null result with the coverage it had.
- Attackers change tools more easily than behaviours. The pyramid of pain ranks what you can detect by how costly it is to change: hash values, addresses, domain names, artifacts, tools, techniques. Hunt higher up when you can.
- Hunt only in telemetry you are approved to examine. Write the approval and the scope in the report; do not widen the data because it would be interesting. Personal data seen during a hunt is handled as the policy says.
- A null result is a result: it says the behaviour was not seen in this data over this period, with these exclusions, and nothing about data you did not examine.

### Guided lab (120 min)
1. Write two hypotheses: a scheduled task or service created from a parent process that is not an administrative tool; outbound connections from servers to destinations seen rarely, at near-regular intervals.
2. For each state the data, the normal pattern, the stopping criteria. Run them over the lab store and one dataset you chose, recording each query, the number of events examined and the time range.
3. Classify what you find as true, benign or unknown, and what you excluded and why. Record the confidence.
4. Convert the first hypothesis into a rule with positive and negative fixtures and run it over the baseline.

### Independent lab (120 min)
Take one technique at random from the knowledge base's list and write and run a hunt for it over your data. Document either a finding or a null result. State the share of hosts and time your data covered, so that a null result can be read correctly.

### Defence and verification (60 min)
Make the hunt repeatable: store the queries as code with parameters, run them against a snapshot of the store, and produce the report skeleton automatically. Measure what the first hunt's rule alerts on across the baseline and record its false positives. Add the hunt to a schedule and note what review the output needs.

### Evidence and report (60 min)
Hand in the hypotheses, queries, volumes, exclusions, findings and confidence, the converted rule with its fixtures and false-positive measurement, and the repeatable hunt code. It is accepted when each hunt names its approved data, its coverage and its stopping criteria.

### Resources
- https://attack.mitre.org/techniques/enterprise/ | External | ATT&CK Enterprise techniques; pin the release you read | MITRE terms of use, ASSUMED
- https://sigmahq.io/docs/ | External | Sigma specification and guides, current version; note the specification version | open specification, licence to be checked, ASSUMED

### Watch for
A hypothesis too vague to fail. Widening the data when the first query finds nothing. Reporting a null result as "clean".

## D48 — Week 8 assessment (blind practical)

**Hand in.** Week 8 assessment — the report of the blind practical: scoping and severity, the evidence handling, the timeline, the indicators, and the recovery recommendation with its risks and limits.

### Objectives
1. Triage blind incident dataset — scope an unseen incident dataset, set a severity with criteria, and preserve and verify the evidence you work from [ir.scoping-triage, dfir.chain-of-custody]
2. Reconstruct timeline — reconstruct what happened from host and network evidence, labelling every statement observed, inferred or assumed [dfir.host-artifacts, dfir.network-evidence]
3. Recommend recovery — recommend containment, eradication and recovery steps with their risks and the evidence that supports each, and state what you could not determine [ir.recovery-recommendation, dfir.blind-incident]

### Blind practical (120 min)
Open the assessment in the application, start the attempt and read the task statement. Take the dataset from the source you fixed before the week and do not read its description or answer notes before the clock starts. Stop at 120 minutes.

### Remediation (60 min)
Grade yourself against the rubric before reading any answer key. For every criterion below your target find the cause in your method (the artifact you did not examine, the assumption you did not write, the hash you did not verify) and fix it with a short exercise on your own images. Then redo the failed part on a different dataset.

### Report (60 min)
Write the report in the structure of the task statement: scope and severity, evidence handling, timeline with labels, indicators, recommendation with risks, limitations. Attach the evidence list with SHA-256 values.

### Performance review (60 min)
Compare the grade with the week's targets: independent execution, evidence verified before analysis, every statement labelled, recommendations tied to evidence and their risks, honest limits. Record the three habits that cost you most and one change for next week.

### Assessment
- Code: W08-BLIND
- Title: Week 8 blind practical: triage an unseen incident dataset and recommend recovery
- Kind: BLIND_PRACTICAL
- Competency: dfir.blind-incident
- Difficulty: 4

Task statement:
You have 120 minutes. You are given an incident dataset you have not seen (any of: a disk image, a memory image, a packet capture, logs) with a short initial report. Work only inside the lab, on the dataset named in your scope statement, and by static and read-only means. Do not execute any program from the dataset.

Scenario source (fixed before the week, not inspected): a dataset prepared by another person from the catalogue below, or, working alone, an unseen dataset of a public forensic challenge or reference collection chosen at random, or images from a clone on which a script you have not read ran a scenario. Scenarios are drawn from: a compromised account used to create persistence on a host; a web application defect leading to a file written on the server; a user-run program that beacons; data staged and sent out; a benign administrator activity that looks like an attack.

Deliverables, in this order. First, scoping: what is affected, what is known, assumed and unknown, and a severity with criteria. Second, evidence handling: hashes at the start and after each copy, the read-only method, and a custody record. Third, a timeline in UTC from at least two kinds of evidence, each statement labelled observed, inferred or assumed with its source. Fourth, indicators found and what each is evidence of. Fifth, a recommendation for containment, eradication and recovery, with the risk of each step and the evidence behind it, and the decisions the owner must make. Sixth, what you could not determine and what would settle it. Seventh, a report of at most five pages with the evidence hashes.

You are assessed on: independent execution during the block; evidence verified before analysis; every statement labelled; recommendations tied to evidence and their risks; honest limits. The pass rule is non-compensatory: every criterion must reach the pass level.

### Resources
- https://cfreds.nist.gov/ | External | Computer Forensic Reference Data Sets; note the dataset you use | NIST publication, terms to be checked per dataset, ASSUMED
- https://digitalcorpora.org/ | External | public forensic corpora; note the corpus you use | licence to be checked per corpus, ASSUMED

### Watch for
Analysing before verifying the hash. Recommending a rebuild without saying what evidence it destroys. Executing something "to see what it does".
