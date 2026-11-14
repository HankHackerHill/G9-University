# Week 7 — SOC, detection and threat hunting

**Purpose.** Detection is engineering: a pipeline that collects the right events, makes them comparable, expresses a hypothesis about abuse as a rule, tests the rule against what must and must not match, and measures what it costs in false alarms. This week you build that pipeline over the lab you have been attacking and defending for six weeks, write portable rules for the behaviours you have already seen, and learn to reconstruct what happened from several sources while keeping facts apart from inference. The Saturday assessment gives you telemetry you have not seen and asks for an investigation and two tested detections.

**Lab stack.** The week 6 lab plus a log pipeline: a collector guest, a forwarder on each Linux and Windows guest (Fluent Bit, Vector or an equivalent), network sensor logs from Zeek or Suricata on the router, and a store you can query (files with Python and SQLite are enough; OpenSearch is optional). Sigma tooling: pySigma with the backend for the store you chose. Public datasets of your choice, with their licences checked. Synthetic data and your own lab's activity only; no real logs of any organisation.

**Competencies.** `det.event-sources`, `det.normalization`, `det.sigma-rules`, `det.correlation-timeline`, `det.criteria-fp`, `det.sigma-backend-fidelity`, `hunt.hypothesis`, `det.blind-telemetry` (pack `detection-soc-hunting`), with `host.persistence-telemetry` and `python.testing-fixtures` revisited.

**Accelerator.** A detection engineer who already writes tested Sigma rules can build the pipeline from existing tooling on day 37 and spend the time on the independent labs; days 39 to 41's fixtures and the Saturday assessment stay.

## D37 — Event sources and coverage

**Hand in.** Coverage map — the inventory of every log source in the lab, the behaviours of weeks 3 to 6 mapped to the sources that could show them, the gaps ranked, and the collection you built for the three most valuable gaps with proof that events arrive.

### Objectives
1. Inventory event sources — list every log source of the lab (hosts, DNS, firewall, web, authentication, network sensors) with what it records, its format, its retention and who can write to or delete it [det.event-sources]
2. Evaluate coverage gaps — map twelve behaviours from weeks 3 to 6 to the sources that could show them and mark each as no source, a weak source or a good one, with the reason [det.event-sources]
3. Design telemetry mapping — for each needed behaviour name the source, the fields required, the collection method and the cost, and implement collection for the three highest-value gaps [det.event-sources, host.persistence-telemetry]

### Recall (15 min)
Without notes: which six identity behaviours did you write detections for last week? For one of them name the event ids and the fields. What did your baseline measure before you set a threshold?

### Theory (45 min)
- A detection pipeline has stages: collect, normalise, store, detect, triage, respond. A weakness in an early stage cannot be repaired later: an event that was never collected cannot be detected, an event whose clock is wrong cannot be correlated.
- A coverage map relates behaviours to the telemetry that would show them. Since release v18 (October 2025) the public knowledge base of adversary techniques describes detection through detection strategies, which point to analytics for each platform, which name the log sources and data components they need; the older data sources were deprecated, so a map made before v18 uses the old vocabulary. Either is a vocabulary for the map. Coverage is not efficacy: a source that exists but is not collected, or is collected without the field you need, covers nothing.
- Logs are evidence and a target. Ask who can write to a log, who can delete it, where it is stored while it is in transit and how long it lives. Logs also hold personal data; collect what the detection needs.
- Read the data-source pages of the ATT&CK knowledge base for three techniques you met in weeks 5 and 6.

### Guided lab (120 min)
1. Build the inventory as a table: source, host, what it records, format, how collected today, retention, writers and deleters.
2. Choose twelve behaviours you have performed in the lab (for example account created, scheduled task created, service installed, privileged group changed, a new listening port, a web request with an injection pattern, a name lookup for an unusual domain). For each name the best source and the fields it must carry.
3. Mark gaps. Rank them by the value of the behaviour (what it would let an adversary do) and the cost of the source (volume, effort, privacy).
4. Implement collection for the top three gaps with a forwarder of your choice, send the events to the collector, and show for a benign trigger of each behaviour that the event arrives with the right host name and a correct time.

### Independent lab (120 min)
Given a short description of a small company (ten servers, a web application, a directory, a cloud mailbox, a budget of fifty gigabytes of logs a day) write a telemetry plan and a coverage matrix. Argue for what you leave out. Review your plan against the behaviours of week 12's capstone and mark which it would miss.

### Defence and verification (60 min)
Write telemetry unit tests: for each behaviour in your matrix a small script creates a benign trigger and then waits for the event at the collector, failing with a clear message if it does not appear within a limit. Run the suite, then disable one source and show the matching test fails. State how often the suite should run and what a pass does not prove.

### Evidence and report (60 min)
Hand in the inventory, the matrix with the gap ranking, the three collection implementations with the proof of arrival (event, host, time), the test suite output with and without the disabled source, and the plan for the imaginary company. It is accepted when each cell of the matrix names a source and a field or says "no source", and each test fails when its source is disabled.

### Resources
- https://attack.mitre.org/resources/updates/updates-october-2025/ | External | ATT&CK v18 release notes (October 2025): detection strategies and analytics, data sources deprecated | MITRE terms of use; CHECKED 2026-11-14 by search result only, page not opened
- https://docs.fluentbit.io/ | External | current documentation; pin the Fluent Bit version you run | open source project, ASSUMED
- https://docs.zeek.org/ | External | current documentation; pin the Zeek version you run | open source project, ASSUMED
- https://docs.suricata.io/ | External | current documentation; pin the Suricata version you run | open source project, ASSUMED

### Watch for
Mapping a behaviour to a source that exists but is not collected. A forwarder that sends events with the collector's clock. Reading "covered" from a matrix instead of from a test.

## D38 — Normalization and enrichment

**Hand in.** Normalization report — the schema and field dictionary, the normalizer with its tests, the evidence on time and identity handling, and the measured cost and failure modes of each enrichment.

### Objectives
1. Normalize log events — convert three different sources (Windows events, authentication and system logs, web or network logs) into one schema with named fields, types and timestamps in UTC [det.normalization]
2. Validate time/source identities — verify timestamp parsing and clock skew, the resolution of host and user identities (names, security identifiers, aliases, addresses that change) and find events whose source cannot be trusted [det.normalization]
3. Assess enrichment — add enrichment (asset role, user tier, network zone, indicator lists) and measure its cost, correctness and failure modes [det.normalization]

### Recall (15 min)
Without notes: name three controls on a log source's integrity. What does a coverage map not tell you? Why must a telemetry test generate its own trigger?

### Theory (45 min)
- Detection logic is written against fields. If the same fact has three names across sources (`user`, `TargetUserName`, `acct`), every rule must know all three; a common schema removes that. Public schemas exist (the Elastic Common Schema, the Open Cybersecurity Schema Framework); use one as your dictionary instead of inventing names.
- Time is the first thing that goes wrong: event time against ingest time, time zones, clock skew between hosts, missing zones in the original text. Store UTC with the original text kept, and carry the skew you measured.
- Identity is the second: a user may be a name, a domain-qualified name, a security identifier or a principal name; a host a short name, a fully qualified name or an address that changed with a lease. Resolve with a time-bounded mapping and say how a failed lookup is recorded.
- Enrichment adds context (which role a host has, which tier an account belongs to) and adds failure modes: stale data, wrong joins, lookups that slow the pipeline, personal data copied into new places.

### Guided lab (120 min)
1. Write the field dictionary for the schema you chose, limited to the fields your rules will need.
2. Write a normalizer in Python for three sources: Windows events (exported), authentication and system logs from the Linux guest, and web or Zeek logs. Emit JSON lines with typed fields, UTC time, the original time text and a source tag. Reuse your parser rules from week 1.
3. Test it with fixtures: valid events, a missing field, a time zone you did not expect, a duplicated event, an out-of-order batch.
4. Measure the clock skew of each guest against the collector and record it.

### Independent lab (120 min)
Add a fourth source (DNS or firewall logs) and a set of chaos fixtures: wrong time zone, the same event twice, half-written lines, a user in three spellings, a host renamed during the run. For each, state what the pipeline should do and what it does. Measure how many events each defect loses or mislabels.

### Defence and verification (60 min)
Add data-quality monitors: events per minute for each source, the share of events missing a required field, the distribution of skew, and a heartbeat that alerts when a source goes quiet. Stop a source and move a guest's clock by ten minutes and show the monitors alert; show they are silent otherwise. State what a monitor cannot see: a source that is complete but wrong.

### Evidence and report (60 min)
Hand in the dictionary, the normalizer and its tests, the skew measurements, the chaos results, the enrichment costs and the monitor output. It is accepted when every dictionary field has a test and every chaos defect has a recorded outcome.

### Resources
- https://www.elastic.co/guide/en/ecs/current/index.html | External | Elastic Common Schema, current version; note the version you use | open schema, licence to be checked, ASSUMED
- https://ocsf.io/ | External | Open Cybersecurity Schema Framework, current version; note the version you use | open schema, licence to be checked, ASSUMED
- https://vector.dev/docs/ | External | current documentation; pin the Vector version you run | open source project, ASSUMED

### Watch for
Parsing a time with no zone as UTC. Joining on a short host name that two hosts share. Enrichment that silently turns a failed lookup into "internal".

## D39 — Sigma rules, hypotheses and fixtures

**Hand in.** Sigma test package — the hypotheses, the rules, the positive and negative fixtures, the conversions, and the result of comparing the converted query with a reference evaluation.

### Objectives
1. Understand Sigma schema — read and write rules in the Sigma format (log source, selections, conditions, modifiers, fields, false positives, level, tags) and explain how a rule is converted for a search system [det.sigma-rules]
2. Draft detection hypothesis — write a hypothesis (behaviour, data, expected normal activity) for a technique from week 5 or 6 and turn it into a rule [det.sigma-rules]
3. Test positive/negative fixtures — build events that must match and events that must not, run the rule through a backend and a reference evaluator you wrote, and show that both agree [det.sigma-rules, det.sigma-backend-fidelity]

### Recall (15 min)
Without notes: name the stages of the detection pipeline in order. What are the two ways a time field goes wrong? Which enrichment in your pipeline can fail silently?

### Theory (45 min)
- A Sigma rule is a portable description of a detection: where the events come from, what a match looks like (selections over fields with modifiers such as contains, starts with, regular expression, address range), how the selections combine, and metadata (level, false positives, tags). A converter turns the rule into a query for a particular system through a mapping of field names and a pipeline of transformations.
- Conversion can change meaning: case sensitivity, how wildcards and escapes are handled, regular-expression dialects, field name mappings, how a missing field is treated. A rule that passes in one system and silently differs in another is a fidelity failure; the only protection is a test that runs the converted query.
- A hypothesis comes first: what behaviour, which event shows it, what normal looks like. The rule is the hypothesis in the rule format; the fixtures are its tests.
- Read the specification and the field-mapping documentation of the converter library.

### Guided lab (120 min)
1. Take three of last week's pseudocode detections and the three persistence checks of week 5 and write them as rules, each with a stated hypothesis.
2. Write fixtures: for each rule at least four events that must match (including variants in case and spacing) and six that must not (near misses and ordinary activity).
3. Convert each rule with the backend for your store and run the query over the fixtures. Separately write a small Python evaluator for the subset of the format you used and run the rules through it.
4. Compare the two result sets. Every disagreement is a finding: decide whether the rule, the conversion or the evaluator is wrong, and fix it.

### Independent lab (120 min)
Choose five rules at random from the public rule repository and convert them with your pipeline. For each decide what its data requirement is, whether your lab can satisfy it, and whether the conversion kept its meaning on your fixtures. Write at least two fixtures per rule and record any rule whose conversion differs from your evaluator.

### Defence and verification (60 min)
Make one command (`make detect-test`) that runs every rule's fixtures through the backend and the evaluator and fails on any mismatch. Add a mutation test: change a modifier in one rule (for example contains to starts with) and show the suite fails. List the kinds of change the suite would not catch.

### Evidence and report (60 min)
Hand in the hypotheses, the rules, the fixtures, the conversion outputs, the evaluator, the comparison with its findings, and the suite output including the mutation. It is accepted when every rule has both kinds of fixture, backend and evaluator agree on all of them, and the mutation is caught.

### Resources
- https://sigmahq.io/docs/ | External | Sigma specification and guides, current version; note the specification version | open specification, licence to be checked, ASSUMED
- https://github.com/SigmaHQ/pySigma | External | current release; pin the version you install | open source project, ASSUMED
- https://github.com/SigmaHQ/sigma | External | rule repository; note the commit you use | rules licensed under the repository's detection rule licence, to be checked, ASSUMED

### Watch for
Writing the rule from the fixture you hold. Trusting a conversion because it ran without errors. Fixtures that only contain the exact string in the rule.

## D40 — Correlation and investigation timelines

**Hand in.** Investigation timeline — a timeline of a scripted benign scenario built from several sources, each entry tied to its event, with every statement marked as observed, inferred or assumed.

### Objectives
1. Correlate sources — join events across sources by host, user, session and time window, handling identity and clock differences, and say what each join proves [det.correlation-timeline]
2. Reconstruct investigation timeline — build a timeline of a scripted scenario in your lab from multi-source logs, with each entry tied to its event and source [det.correlation-timeline]
3. Separate fact from inference — mark each statement of the timeline as observed, inferred or assumed, give its evidence and say what would change it [det.correlation-timeline]

### Recall (15 min)
Without notes: what is a fixture of the negative kind and why does a rule need one? Name three ways a conversion can change a rule's meaning. What does a mutation test show?

### Theory (45 min)
- Correlation links events that belong together. The strongest links are identifiers that the systems share (a logon session identifier that appears in the logon event and the process-creation event, the audit user id and session in the Linux audit records, the connection identifier that ties the network sensor's connection, name lookup and web records). Weaker links are time and place, which need a window and a measured skew.
- A timeline is a hypothesis with evidence. Every entry carries its source event; every statement is labelled as observed (an event shows it), inferred (a chain of reasoning from events) or assumed (no event, taken for granted). Confidence language should be consistent and should say what would raise or lower it.
- Competing explanations stay on the page until evidence removes them; a table of hypotheses against evidence (consistent, inconsistent, not informative) keeps the work honest.

### Guided lab (120 min)
1. Write a scenario script that a clone runs while you do not watch: a user logs on, creates a scheduled task, looks up an unusual name, downloads a file from the lab web server, installs a service. Everything is benign and writes a marker.
2. Collect all sources for the period. Normalise them with last week's tool.
3. Build the timeline: join the logon to the process start by session identifier, the process to the network connection, the name lookup to the connection. Note every join's key and its weakness.
4. Label each statement observed, inferred or assumed with the evidence, and list what you would ask the owner of each system.

### Independent lab (120 min)
Run a second scenario drawn at random from three you prepared and describe within ninety minutes what happened, from the logs only. Write the competing hypotheses and the evidence against each. Then read the script and mark where you went beyond the evidence and what you missed.

### Defence and verification (60 min)
Express one correlation as a detection: the sequence of a logon, a scheduled task creation and a lookup of a name not seen in the baseline within ten minutes. Implement it as a correlation rule or as code over your normalised store. Test it with the scenario, with three benign variants (each missing one step or exceeding the window) and with the baseline's ordinary activity, and record the false positives.

### Evidence and report (60 min)
Hand in the scenario script, the timeline with sources and keys, the labels with evidence, the hypothesis table, the questions for owners, and the sequence detection with its tests. It is accepted when every timeline entry cites an event and no statement lacks a label.

### Resources
- https://attack.mitre.org/ | External | ATT&CK knowledge base; v20 (October 2026) reported current, pin the release you read | MITRE terms of use; CHECKED 2026-11-14 by search result only, page not opened
- https://www.sans.org/white-papers/ | External | public white papers of the SANS Institute; note the paper you use | publisher terms apply, ASSUMED
- https://plaso.readthedocs.io/ | External | current documentation of the timeline tool; pin the version you run | open source project, ASSUMED

### Watch for
Joining on time alone when an identifier exists. A timeline entry with no event behind it. Writing "the attacker" where the evidence says "a process".

## D41 — Detection criteria and false-positive estimation

**Hand in.** Detection validation report — for each rule the acceptance criteria, the measured false-positive rate with its interval, the refinements and their trade-offs, and the evasion fixtures it misses.

### Objectives
1. Define detection criteria — state for a rule what it must detect, the data it needs, its severity, the response it triggers and its acceptance criteria (the fixtures it must match and a false-positive budget per day) [det.criteria-fp]
2. Estimate false positives — measure a rule against a baseline of normal activity, estimate its false-positive rate with an interval, and say what the estimate does not cover [det.criteria-fp]
3. Refine rule — refine the rule to meet its budget without losing the fixtures it must catch, and record the trade-off of each change [det.criteria-fp, det.sigma-backend-fidelity]

### Recall (15 min)
Without notes: list the join keys you used yesterday and how each could break. Write one sentence for each of observed, inferred and assumed. Why does the sequence detection need benign variants?

### Theory (45 min)
- A detection is a classifier: it has precision (how many alerts are real) and recall (how much of the real activity it catches). When real events are rare, even a low false-positive rate buries them: a rule that fires wrongly on one event in a hundred, on a source that produces a hundred thousand events a day, produces a thousand false alerts a day. Budget the false positives first.
- Estimate rates from counts over a stated period with an interval, not from a single day's number. If no false positive was observed in N independent trials, the upper bound at about ninety-five percent confidence is roughly three divided by N.
- Tuning by exclusion removes alerts and creates blind spots; record each exclusion with its reason and the evasion it permits. Keep fixtures for the evasions you know (a renamed binary, different case, encoded arguments) and track which ones the rule misses.
- A rule has a lifecycle: hypothesis, rule, tests, deployment, monitoring, retirement. Its severity and response must be decided before it fires.

### Guided lab (120 min)
1. Build a baseline: generate three days of ordinary lab activity (a generator that logs users in, runs jobs, browses, patches) plus the noise of your guests.
2. Write acceptance criteria for three rules: the fixtures to match, the data required, the severity, the response and the budget.
3. Run each rule over the baseline and count alerts. Classify them as true, false or unknown. Estimate the false-positive rate per day with an interval.
4. Refine each rule to meet its budget. After every change rerun the fixtures; record the trade-off.

### Independent lab (120 min)
Choose three public rules at random, convert them and measure them against your baseline. For each decide whether it is deployable under your budget, refine it, and record the exclusions with their blind spots. Write three evasion fixtures per rule and record which the refined rule misses.

### Defence and verification (60 min)
Automate the acceptance: a test that fails if any must-match fixture is missed, if the false-positive upper bound on the baseline exceeds the budget, or if a known evasion that previously matched stops matching. Run it before and after a deliberate change that over-tunes a rule, and show it fails. List what the baseline does not represent (a different season, a new application).

### Evidence and report (60 min)
Hand in the criteria, the baseline description, the counts and intervals, the refinements and their trade-offs, the evasion table and the acceptance test output. It is accepted when every rule has a measured rate with an interval and every exclusion names the evasion it permits.

### Resources
- https://sigmahq.io/docs/ | External | Sigma specification and guides, current version; note the specification version | open specification, licence to be checked, ASSUMED
- https://attack.mitre.org/techniques/enterprise/ | External | ATT&CK Enterprise techniques; pin the release you read | MITRE terms of use, ASSUMED

### Watch for
A rate from one quiet day. Excluding a process name that an adversary can also choose. A budget set after seeing the alerts.

## D42 — Week 7 assessment (blind practical)

**Hand in.** Week 7 assessment — the report of the blind practical: the investigation with its timeline, two detections with their tests and false-positive estimates, and the limitations.

### Objectives
1. Investigate blind telemetry — investigate an unseen set of multi-source telemetry from a stated hypothesis and reconstruct what happened [det.blind-telemetry, hunt.hypothesis]
2. Implement evidence-based detection — write two rules for behaviours you found, with positive and negative fixtures and a false-positive estimate [det.sigma-rules, det.criteria-fp]
3. Report limitations — state what the telemetry could not show, which conclusions are observed, inferred or assumed, and what would change them [det.blind-telemetry]

### Blind practical (120 min)
Open the assessment in the application, start the attempt and read the task statement. Take the telemetry from the source you fixed before the week and do not read its description before the clock starts. Stop at 120 minutes.

### Remediation (60 min)
Grade yourself against the rubric before reading any answer key. For every criterion below your target find the cause in your method (a source you did not query, a join you did not verify, a rule with no negative fixture) and fix it with a short exercise on your own data. Then redo the failed part on different telemetry.

### Report (60 min)
Write the report in the structure of the task statement: hypothesis, timeline with labels, findings, the two rules with fixtures and estimates, limitations. Attach the evidence with SHA-256 values.

### Performance review (60 min)
Compare the grade with the week's targets: independent execution, a stated hypothesis before searching, every statement labelled, detections with negative fixtures and a measured rate, honest limits. Record the three habits that cost you most and one change for next week.

### Assessment
- Code: W07-BLIND
- Title: Week 7 blind practical: investigate unseen telemetry and implement two tested detections
- Kind: BLIND_PRACTICAL
- Competency: det.blind-telemetry
- Difficulty: 4

Task statement:
You have 120 minutes. You are given a set of telemetry you have not seen from a small environment, with a short description of its hosts and sources, and the statement that something of interest happened in it. Work only inside the lab and on the data named in your scope statement.

Scenario source (fixed before the week, not inspected): telemetry prepared by another person from a scenario of the catalogue below, or, working alone, an unseen public dataset or challenge of an investigation training platform chosen at random, or a recording of a scenario script run by a clone with a script you have not read. Scenarios are drawn from: a new account followed by a scheduled task or service; abnormal authentication across several hosts; a web request pattern followed by a file write on the server; a name-lookup pattern that does not match the baseline; a privileged group change outside a window; a benign administrator activity that looks like an attack.

Deliverables, in this order. First, a stated hypothesis before you search, and how you will test it. Second, an investigation record: queries, joins and the evidence behind each. Third, a timeline with every statement labelled observed, inferred or assumed. Fourth, two detection rules for behaviours you found, with at least four positive and six negative fixtures each, converted for your store and checked against your evaluator, and a false-positive estimate with an interval on whatever baseline the data allows. Fifth, the limitations: what the telemetry could not show and what would change your conclusions. Sixth, a report of at most four pages with the evidence hashes.

You are assessed on: independent execution during the block; a hypothesis stated before searching; every statement labelled; detections with negative fixtures and a measured rate; honest limits. The pass rule is non-compensatory: every criterion must reach the pass level.

### Resources
- https://github.com/OTRF/Security-Datasets | External | public datasets repository; note the dataset and commit you use | open datasets, licence to be checked per dataset, ASSUMED
- https://github.com/sbousseaden/EVTX-ATTACK-SAMPLES | External | public event log samples; note the file and commit you use | licence to be checked, ASSUMED

### Watch for
Searching before stating a hypothesis. A rule with no negative fixture. Calling an inference an observation.
