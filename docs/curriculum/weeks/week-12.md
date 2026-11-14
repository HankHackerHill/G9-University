# Week 12 — Integrated G9/CT9 capstone

**Purpose.** Eleven weeks of separate skills now have to work together on one system the way a real engagement works: a written scope, a threat model across layers, an assessment that follows an access chain from the outside to something that matters, detection and response reasoning, an assessment of the AI components, and a report that defends every claim with evidence. The week is a rehearsal of an engagement from authorization to report, ending with a blind integrated practical and the portfolio that records what you can do and what you cannot yet.

**Lab stack.** Everything from weeks 1 to 11, joined into one integrated environment, "the estate": two network zones behind a router, the web application and API of weeks 3 and 4, a Linux host and a Windows domain of weeks 5 and 6, the log pipeline of week 7, and the assistant, agent and one MCP server of weeks 9 and 10, with a canary document standing in for the crown jewel. A second person, or a script you have not read, plants faults across the layers and a benign incident in the telemetry, and seals the list. Isolation, authorization, synthetic data and harmless goals apply exactly as before. The evidence of the capstone is kept in one repository with an index and hashes.

**Competencies.** All eight packs: this week revisits `lab.authorization`, `recon.scope-definition`, `web.threat-model-logic`, `ai.threat-model-redteam`, `net.failure-diagnosis`, `web.authz-idor`, `api.token-authz`, `linux.privilege-boundaries`, `ad.delegation-permissions`, `ad.kerberos`, `det.sigma-rules`, `dfir.host-artifacts`, `ir.recovery-recommendation`, `agent.workflow-authority`, `mcp.transport-auth`, `llm.prompt-injection` and others, each in the setting where it matters most: together.

**Accelerator.** None: the capstone is the accelerator's end point. An experienced practitioner takes it unchanged and is graded on the same rubric.

## D67 — Capstone charter: scope, threat model and acceptance rules

**Hand in.** Capstone charter — the scope and rules of engagement for the estate, the integrated threat model across layers, the acceptance rules that say when the assessment is complete and how findings are rated and evidenced, and the baseline of the estate before assessment.

### Objectives
1. Define scope and authorization — write the scope statement and rules of engagement for the estate: owner, systems and addresses in scope, permitted and forbidden actions, rate, window, data handling, escalation and stop condition, with the authorization of the owner recorded [lab.authorization, recon.scope-definition]
2. Build integrated threat model — draw the estate as one system (zones, hosts, identities, applications, data stores, pipelines, AI components) and derive the threats that cross layers, ranked with reasons [web.threat-model-logic, ai.threat-model-redteam]
3. Establish acceptance rules — define the severity rubric, the evidence standard, the coverage required of each layer, the reproducibility rule and the completion criteria of the assessment before starting it [linux.evidence-completeness]

### Recall (15 min)
Without notes: write the scope statement you wrote on day 1 and name what you would now add. List the elements of a pre-registered design. What did your portfolio index for weeks 1 to 4 lack?

### Theory (45 min)
- An engagement starts with permission and limits, not with tools. The rules of engagement turn authorization into operating rules: what may be touched, how hard, when, with what data, who to call if something breaks, and what ends the work. The record of who authorized what, and when, is part of the evidence.
- A cross-layer threat model follows an attacker's route, not an organisation chart: from an exposed web feature to a service account, to a host, to a domain privilege, to data, and the same route for an assistant that holds credentials. Defensive findings are strongest where they cut a route at its cheapest point.
- Acceptance rules prevent two failures: stopping when you are tired and reporting only what was easy. Decide before the work how findings are rated (a severity rubric with stated criteria, for example a scoring system's base metrics adjusted by your environment's context), what counts as evidence for a finding, which layers must be covered and to what depth, and when the work is complete.
- Read the planning and reporting guidance of the national technical guide to security testing, and the severity scoring specification you will use.

### Guided lab (120 min)
1. Write the scope statement and rules of engagement for the estate and have the owner of the estate (you, as owner, in a separate signed note) authorize it. Include the stop condition and the escalation path for something unexpected.
2. Record the pre-assessment baseline: snapshots, hashes of configuration, the topology, the list of accounts, the telemetry sources, the model and tool list. This is what you will compare against when you verify fixes.
3. Draw the estate as one data-flow diagram with trust boundaries and mark the five routes of greatest concern from outside to the canary.
4. Write the acceptance rules: severity rubric, evidence standard, coverage per layer, the reproducibility rule (each finding reproduced twice, by a second method where possible) and the completion criteria.

### Independent lab (120 min)
Without opening the sealed list, write an assessment plan for the estate: the order of work, the time box for each layer, what you will do if a layer is larger than expected, and which of the five routes you will test first and why. Then review your plan against the rules of engagement and strike anything that is out of scope.

### Defence and verification (60 min)
Verify the baseline is usable: from the recorded baseline, restore the estate in a clean copy and show that the baseline's hashes match and the topology is identical. Write the check that detects drift from the baseline and run it after a deliberate small change. State what the baseline does not cover (state that lives only in memory).

### Evidence and report (60 min)
Hand in the scope statement and authorization note, the baseline with hashes and restore proof, the integrated diagram with the five routes, the acceptance rules and the assessment plan. It is accepted when a stranger could tell from the charter what may be tested, how a finding is rated and when the work is done.

### Resources
- https://csrc.nist.gov/pubs/sp/800/115/final | External | SP 800-115 (September 2008) | NIST publication, freely available
- https://www.first.org/cvss/ | External | the common vulnerability scoring system, current version; note the version you use (4.0 was published in November 2023) | specification published by FIRST, terms to be checked, ASSUMED
- https://attack.mitre.org/ | External | ATT&CK knowledge base; pin the release you read (version 20 was reported current on 2026-11-14) | MITRE terms of use; CHECKED 2026-11-14 by search result only, page not opened

### Watch for
A scope written after the first scan. Acceptance rules written after seeing what is easy. A baseline taken after the assessment started.

## D68 — Network and application assessment

**Hand in.** Network/app assessment — the findings for the network and for the web application and API, each with evidence, severity, root cause and remediation, and the verification that each remediation works.

### Objectives
1. Assess isolated network — map the estate's zones and test its segmentation and name resolution against its intended policy within scope, interpreting discovery results with confidence [net.failure-diagnosis, recon.discovery-interpretation]
2. Evaluate controlled app weaknesses — test the web application and API for access control, token, request-forgery, upload and logic weaknesses, with two tenants and several roles, and prove each finding safely [web.authz-idor, api.token-authz, web.ssrf-upload-deserialization]
3. Document remediation — for each finding write the root cause, the smallest remediation and a regression test, and apply and verify the ones within your remit [web.threat-model-logic]

### Recall (15 min)
Without notes: list the five routes of your charter and the layer each starts in. What was the stop condition you wrote? Which acceptance rule would stop you from reporting only easy findings?

### Theory (45 min)
- An assessment works through layers against a policy: what the network is meant to allow, what the application is meant to do for whom. Findings are differences between intent and reality, each with a cause, an effect and a cost to fix.
- Chain findings with care: a low-severity weakness that completes a route to the canary may matter more than a medium one that leads nowhere. Rate each finding alone and its place in a chain.
- Reproduce every finding twice and by a second method where you can, using synthetic data and harmless markers. A finding that cannot be reproduced is a lead, not a finding; say so.
- Remediation is part of the job: the smallest change that removes the cause, with a test that failed before and passes after.

### Guided lab (120 min)
1. Within the scope, map the zones and compare the observed flows with the intended policy using the matrix of week 2. Interpret discovery results with confidence labels and note every difference.
2. Test name resolution and the router's rules for the differences that matter to your routes.
3. Test the web application and API as two tenants and the roles of week 4, following the checklist of classes: access control, tokens, request forgery, uploads and import, business logic, injection and output encoding.
4. For each finding record the evidence, a second reproduction, the severity under your rubric, and the route it belongs to.

### Independent lab (120 min)
Take the layer or class you tested least thoroughly in the guided block and test it again with a different method (a different tool, or by reading code and configuration instead of probing). Compare the findings and record what the second method added.

### Defence and verification (60 min)
Fix what is in your remit: apply the smallest remediation for each finding, write the regression test, and show that the test fails on the baseline and passes after. Re-run the segmentation matrix and the access-control matrix and update the drift check. State which findings you could not fix and why.

### Evidence and report (60 min)
Hand in the findings table (evidence, second reproduction, severity, route, root cause, remediation, test), the matrices before and after, and the list of unfixed findings. It is accepted when every finding was reproduced twice and every fixed finding has a test that fails on the baseline.

### Resources
- https://owasp.org/www-project-web-security-testing-guide/ | External | Web Security Testing Guide, stable release to be confirmed when you read it | OWASP project, open licence stated by the project, ASSUMED
- https://owasp.org/API-Security/editions/2023/en/0x00-header/ | External | OWASP API Security Top 10, 2023 edition | OWASP project, open licence stated by the project, ASSUMED

### Watch for
Testing as one tenant. A finding reported from a single run. Fixing the symptom and not the cause.

## D69 — Host and identity assessment

**Hand in.** Host/identity assessment — the privilege paths on the hosts and in the domain, the access chain from an ordinary account to the surrogate privileged group, the controls that cut it and the proof that they hold.

### Objectives
1. Assess host/identity boundaries — enumerate the Linux and Windows hosts and the domain as a low-privilege principal and find the privilege paths and the trust problems [linux.privilege-boundaries, win.token-service-controls]
2. Explain access chain — explain the chain from the application foothold of the previous day to host and domain privilege as one path, with the right, trust or misconfiguration behind each step [ad.delegation-permissions, ad.kerberos]
3. Verify controls — cut the chain at its cheapest points, re-enumerate to show the chain is gone, and confirm the hardening baseline holds on every host in the chain [linux.hardening-baseline, ad.identity-detection]

### Recall (15 min)
Without notes: list the severity criteria of your rubric. Which of your findings yesterday belongs to more than one route? How would you show a stranger that a fix works?

### Theory (45 min)
- A chain is a sequence of steps each of which gives the next its starting point: an application account that can read a configuration file, a credential in it that is valid on a host, a service on that host that runs with a privilege, a delegation or a write right in the directory. The weakest link is not the most severe finding but the one whose removal cuts the most chains.
- Explain every step in terms of a boundary: whose authority, which check, why it did not hold. A step that you cannot explain is one you have not understood and cannot fix with confidence.
- Cutting a chain means choosing the cheapest effective edge, and verifying means collecting again. A verification that reuses the original step's method proves the step is closed; a different method proves the chain is.
- Hardening verification is behavioural, as in week 5.

### Guided lab (120 min)
1. Enumerate the Linux hosts, the Windows hosts and the domain as a low-privilege principal: sudo and capabilities, writable privileged files, services and their paths, persistence, delegation, rights on privileged objects, tokens and groups.
2. Join the application foothold of day 68 to what you find: which credential, which host, which service, which right. Draw the chain and mark the boundary at each step.
3. Reproduce the chain twice with harmless markers. State for every step why it works.
4. Rate the chain and each step under your rubric.

### Independent lab (120 min)
Search for a second chain that does not use the first foothold (a different entry point or a different privilege route). If you find one, document it; if not, document what you tried and what coverage you had, so that the null result can be read.

### Defence and verification (60 min)
Cut each chain at its cheapest edge and re-enumerate with a different method from the one that found it. Run the behavioural hardening tests of week 5 on the hosts in the chain and the identity checks of week 6 on the domain. Update the drift check. Record what remains and why.

### Evidence and report (60 min)
Hand in the enumeration records, the chain diagram with the boundary for each step, the reproductions, the cuts with the re-enumeration, and the hardening test output. It is accepted when every step of every chain is explained as a boundary and the chains are shown gone by a different method.

### Resources
- https://github.com/SpecterOps/BloodHound | External | Community Edition, current release; pin the version you run | open source project, licence to be checked, ASSUMED
- https://attack.mitre.org/tactics/TA0004/ | External | ATT&CK privilege escalation tactic; pin the release you read | MITRE terms of use, ASSUMED

### Watch for
Reporting a tool's path without the right behind each edge. Verifying a fix with the same step that found the flaw. Ignoring the hosts that are not on the first chain.

## D70 — Detection, investigation and response reasoning

**Hand in.** Detection+IR package — the detections you built for the chains with their tests and false-positive estimates, the investigation of the incident in the telemetry with its timeline, and the incident response reasoning with scoping, severity and recovery recommendation.

### Objectives
1. Build telemetry/detection — build detections for the steps of the chains you found, with fixtures, conversions and a false-positive estimate against the estate's baseline [det.sigma-rules, det.criteria-fp]
2. Investigate incident scenario — investigate the benign incident planted in the estate's telemetry from a stated hypothesis, correlating host, network and identity evidence into a labelled timeline [det.correlation-timeline, dfir.host-artifacts, dfir.network-evidence]
3. Demonstrate IR reasoning — scope and rate the incident, choose and justify containment, eradication and recovery steps with their risks, and state what you could not determine [ir.scoping-triage, ir.recovery-recommendation]

### Recall (15 min)
Without notes: list the steps of your chain on the host side and the telemetry each would leave. What is the difference between observed and inferred? Write one sentence for each of containment, eradication and recovery.

### Theory (45 min)
- The detections of the capstone follow the chains: a detection is valuable where it sits on several chains, early, and with a cost you can bear. Write the hypothesis, the rule, the fixtures and the estimate as in week 7; ask of each how an adversary could avoid it and what it would take.
- An investigation needs a hypothesis before the search, a record of queries and joins, and a timeline whose statements are labelled. Incident response reasoning then asks what is affected, how bad, what to do first, and what each action risks, including what it destroys.
- A recommendation is a decision aid: who decides, what the options are, what each costs and risks, what evidence supports it, what is still unknown, and when to review.
- Combine the evidence kinds: a name lookup, a connection, a process start and a logon become one story only when joined on identifiers you can name.

### Guided lab (120 min)
1. For the steps of your chains write six detections with hypotheses, positive and negative fixtures, a conversion check and an estimate of false positives per day over the estate's baseline. Choose where each runs.
2. State a hypothesis for the planted incident before you search. Investigate it across host, network and identity telemetry with a record of queries.
3. Build the timeline with every statement labelled observed, inferred or assumed.
4. Write the scoping and severity decision, with criteria, and the containment, eradication and recovery recommendation with risks.

### Independent lab (120 min)
Hunt in the same telemetry for something your detections did not target, using a hypothesis drawn at random from the techniques list. Record the coverage and the result, including a null result. Then compare what your detections would have caught of the planted incident with what the investigation found by hand, and list the gap.

### Defence and verification (60 min)
Run the detections against replays of your own chain reproductions and against the baseline: they must fire on the first and stay within budget on the second. Measure time from the chain's step to the alert. Record the evasions you know and a plan to close the cheapest. Rehearse the incident decision in ten minutes as if speaking to the owner of the estate and note what you could not say with evidence.

### Evidence and report (60 min)
Hand in the detections with fixtures and estimates, the replay results, the investigation record and labelled timeline, the response recommendation with its risks, and the evasion list. It is accepted when each detection has negative fixtures and a measured rate, each timeline statement is labelled, and each recommendation states its risk.

### Resources
- https://sigmahq.io/docs/ | External | Sigma specification and guides, current version; note the specification version | open specification, licence to be checked, ASSUMED
- https://csrc.nist.gov/pubs/sp/800/61/r3/final | External | SP 800-61, the current revision; note which revision you read | NIST publication, freely available, ASSUMED

### Watch for
Rules with no negative fixture. A timeline that mixes inference into observation. A containment recommendation that destroys the evidence.

## D71 — AI and agent assessment

**Hand in.** AI security assessment — the map of the assistant, agent and MCP server in the estate, the findings on authorization and trust boundaries with harmless reproductions, the mitigations with deterministic tests, and the measured rates where the model matters.

### Objectives
1. Assess an AI/agent workflow — map the assistant's retrieval, the agent's loop, tools and the MCP server, with identities and authority crossings, and connect them to the chains of the estate [agent.workflow-authority, llm.app-mapping]
2. Test authorized trust boundaries — test, with harmless goals, the retrieval permissions, the tools' authority, the token handling of the MCP server and the injection paths, within the authorization of the charter [agent.identity-scopes, mcp.transport-auth, llm.prompt-injection]
3. Verify mitigations — apply and verify the structural mitigations (typed enforcement, delegated authority, memory write controls) with deterministic tests and a pre-registered measurement for the model-dependent ones [llm.output-schema-tools, agent.memory-integrity]

### Recall (15 min)
Without notes: list the three questions to ask of every tool. What does a scripted adversarial model prove? State the chain-of-evidence rule for a model-dependent finding.

### Theory (45 min)
- In an estate the AI components are not a separate subject: the assistant holds credentials, reads documents and calls tools that reach the same hosts and domain as the rest. A finding in the assistant is a route into the estate or from it, and belongs in the same threat model and rating as any other.
- The tests are those of weeks 9 and 10, applied to the estate's components: retrieval permissions checked in code, tool authority derived from the requester, tokens bound to their audience, tool definitions pinned, memory writes gated, limits enforced outside the model.
- Separate what the application guarantees (tested with a stub, expected to pass) from what the model does (measured, reported with intervals, with the model version and settings).

### Guided lab (120 min)
1. Map the assistant, agent and MCP server of the estate with the identity and authority at each crossing, and connect each crossing to a route in the integrated diagram.
2. Test the boundaries with harmless fixtures: a canary in a restricted document, a tool call carrying a marker, a token for another server, a changed tool description, a stored instruction in memory.
3. Record each finding with its reproduction and the boundary it crosses; run the model-dependent ones twenty times and report intervals.
4. Rate each finding in the estate's context: what it gives an attacker on the chains.

### Independent lab (120 min)
Write a pre-registered mini experiment for the one model-dependent control that matters most to the estate, run it, and report the result with the design date shown in version control. Add the attacking input family you held out.

### Defence and verification (60 min)
Apply the structural fixes and verify them: stub tests for retrieval, authority, tokens, definition pinning and memory, and the measured rates before and after for the model-dependent control. Update the estate's drift check to include the AI components. State what remains and the owner who must decide on it.

### Evidence and report (60 min)
Hand in the map with crossings, the findings with boundaries, the stub tests, the measured rates with intervals and settings, the pre-registered design and the fixes. It is accepted when every guarantee has a stub test and every statistic has an interval and a settings record.

### Resources
- https://genai.owasp.org/llm-top-10/ | External | the project's Top 10 for LLM Applications; the current edition | OWASP project, open licence stated by the project; CHECKED 2026-11-14 by search result only, page not opened
- https://modelcontextprotocol.io/specification/ | External | protocol specification; pin the revision you test against and read its change log | open specification, licence to be checked; CHECKED 2026-11-14 by search result only, page not opened

### Watch for
Treating the assistant as outside the estate's threat model. A model-dependent finding reported without a rate. Fixing the prompt and calling it a control.

## D72 — Capstone: blind integrated practical, defence of findings and portfolio

**Hand in.** Capstone report — the report of the blind integrated practical with every claim defended by evidence, and the final portfolio with its index, the self-assessment against all competencies and the plan for the next twelve weeks.

### Objectives
1. Complete blind integrated practical — assess an unfamiliar estate across layers within the time box, following an access chain to the canary and recommending controls and detections [ai.threat-model-redteam, web.threat-model-logic, dfir.blind-incident]
2. Defend findings with evidence — for each claim present the evidence, the reproduction, the alternatives excluded and the limits, and answer questions on it as in a review with the owner of the estate [linux.evidence-completeness, ir.scoping-triage]
3. Deliver final CT9 portfolio — assemble the portfolio of all twelve weeks with an index, hashes and a self-assessment, and record what you can do, what you cannot yet, and what you will study next [linux.evidence-completeness, ir.recovery-recommendation]

### Blind practical (120 min)
Open the assessment in the application, start the attempt and read the task statement. Take the estate from the source you fixed before the week and do not read its notes or its list of faults before the clock starts. Stop at 120 minutes.

### Remediation (60 min)
Grade yourself against the rubric before reading any answer key. For every criterion below your target find the cause in your method across the twelve weeks (the habit that first appeared in week 1 or 3 and still costs you) and repair it with a targeted exercise from the week that taught it. Redo the part of the practical that failed on a fresh copy of the estate where you can.

### Report (60 min)
Write the capstone report: an executive summary for the owner (what is wrong, what matters most, what to do first, what you could not determine) and a technical part (scope, method, findings with severity, evidence and reproduction, chains, detections, response reasoning, remediation and verification, limits). Then defend it: record yourself answering the ten hardest questions a reviewer could ask, and state the evidence for each answer.

### Performance review (60 min)
Assemble the portfolio: an index of the twelve weeks' hand-ins with hashes, the capstone report, a self-assessment for each of the 64 competencies (what evidence supports it, what is missing), the gap list ranked by consequence, and a plan for the next twelve weeks. Then review the program: which habits changed, which did not, which week taught the most, and what you would change in this curriculum.

### Assessment
- Code: W12-CAPSTONE
- Title: Week 12 capstone: blind integrated assessment of an unfamiliar estate
- Kind: CAPSTONE
- Competency: ai.threat-model-redteam
- Difficulty: 5

Task statement:
You have 120 minutes for the practical, then the report and defence blocks of the day. You are given an estate you have not seen: two network zones behind a router, a web application and API, Linux hosts, a Windows domain with a surrogate privileged group, a log pipeline holding a benign incident, and an AI assistant with an agent and one tool server, with a canary document standing in for the crown jewel. Work only inside the lab, on the estate named in your scope statement and its rules of engagement. Use synthetic data and harmless markers only.

Scenario source (fixed before the week, not inspected): an estate prepared by another person who has planted faults across the layers, a chain to the canary and a benign incident in the telemetry, and sealed the list, or, working alone, a variant of your own estate produced by a script you wrote earlier but have not read since. Planted faults are drawn from the catalogues of weeks 1 to 11: a segmentation or name-resolution fault; a web or API authorization, request-forgery, upload or logic fault; a host privilege path; a directory delegation or permission path; a detection gap with a benign incident in the logs; an authorization or trust-boundary failure in the assistant, agent or tool server; a supply-chain or data problem.

Deliverables, in this order. First, within the first ten minutes, the scope check against your rules of engagement and the plan with time boxes. Second, an access chain from outside to the canary with every step explained as a boundary and reproduced with harmless markers, or, if you did not reach it, the furthest point reached and the evidence that stops you. Third, the other findings that matter, rated with your rubric and each reproduced twice. Fourth, the controls that cut the chain at its cheapest points, applied where within your remit and verified by a method different from the one that found the weakness. Fifth, at least two detections for steps of the chain with negative fixtures and a false-positive estimate, and a labelled timeline of the benign incident. Sixth, the response recommendation with risks. Seventh, the limits: what you did not cover and why. Eighth, the capstone report and the defence of its claims, and the portfolio.

You are assessed on: independent execution during the block; scope respected; a chain explained as boundaries and reproduced; findings rated and evidenced; controls verified by a different method; detections with negative fixtures and a measured rate; timeline labelled; honest limits; the defence of the evidence. The pass rule is non-compensatory: every criterion must reach the pass level.

### Resources
- https://csrc.nist.gov/pubs/sp/800/115/final | External | SP 800-115 (September 2008) | NIST publication, freely available

### Watch for
Leaving the scope check until later. Reporting breadth with no chain. Writing the executive summary last, in a hurry, and for yourself.
