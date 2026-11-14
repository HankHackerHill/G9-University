# Week 10 — Agent security, MCP and tool authorization

**Purpose.** An agent is a language model in a loop with tools: it decides, calls, reads the result and decides again. Every tool is a grant of authority, every tool result is a new input to the model, and every step is a chance for text to change what happens next. This week you build a small agent of your own and take apart its authority: where authority crosses a boundary, whose identity each call carries, what a protocol such as the Model Context Protocol adds and what it leaves to you, what persistent memory does to trust, and how to test defences in a bounded experiment that you design before you see the result. The rule from week 9 gets stricter: the model proposes, code disposes, and the code that disposes holds the authority.

**Lab stack.** The week 9 lab plus an agent loop of your own (a few hundred lines of Python), a sandbox container for tools with no network by default and a mounted scratch directory, a local authorization server for tokens (a small OAuth-style service you write or a lightweight open-source one), an MCP server and client written with an official software development kit, a memory store you can inspect, and a scripted adversarial model stub from week 9. Pin the specification revision of the protocol you test against and read its change log; the protocol is changing quickly and the authorization rules in particular have been revised several times. Synthetic data only; every adversarial goal is harmless.

**Competencies.** `agent.workflow-authority`, `agent.identity-scopes`, `mcp.transport-auth`, `agent.memory-integrity`, `agent.redblue-experiment` (pack `agentic-mcp-supplychain`), with `llm.output-schema-tools`, `api.token-authz`, `python.testing-fixtures` and `rag.ingestion-provenance` revisited.

**Accelerator.** An engineer who has shipped agent features can build the lab agent in half the time and spend the saved hours on the independent labs and the pre-registered experiment of day 59; the Saturday assessment is taken unchanged.

## D55 — Agent workflow and authority crossings

**Hand in.** Agent boundary diagram — the workflow of the agent as a state machine with every authority crossing marked, the bounded-execution limits with the tests that enforce them, and the kill switch.

### Objectives
1. Map agent workflow — draw the loop of the agent (plan, propose a call, validate, execute, observe, decide) as a state machine with the data and the identity at each step [agent.workflow-authority]
2. Identify authority crossings — mark every point where authority moves (user to agent, agent to tool, tool to an external system, a tool result back into the model, a memory write) and say what constrains each [agent.workflow-authority]
3. Define bounded execution — specify and enforce limits on steps, time, tokens, spend, tools and side effects, with a way to stop the agent that does not depend on the model [agent.workflow-authority, llm.output-schema-tools]

### Recall (15 min)
Without notes: which two of last week's measures are guarantees and which are statistics? Name the property that makes an evaluation independent. What did your adversarial stub prove that the real model could not?

### Theory (45 min)
- An agent turns a model's text into actions. Its security depends less on the model than on the loop around it: which tools exist, with whose credentials they run, what is checked before a call, what limits apply, and what happens to results.
- Authority crossings are where defects live: a user's request delegates some of the user's authority to the agent; a tool exercises an authority that may be larger than the user's (a service account); a tool result carries untrusted text back into the model; a memory write stores a decision for later; a tool that fetches content gives an outsider a way to speak to the agent.
- Excess comes in three kinds: more functionality than the task needs, more permission than the functionality needs, and more autonomy than the consequences allow. Bounded execution addresses all three: a short tool list, narrow credentials, step and time limits, approval for consequential steps, and a stop that code controls.
- Read the agentic threat guidance of the application security community and the protocol-independent parts of the model context specification on tools and consent.

### Guided lab (120 min)
1. Build the agent: a loop in which the model proposes a tool call as structured data, code validates and executes it in the sandbox, and the result is returned as data. Give it four tools: read a file in the scratch directory, fetch a page through an allow-listed proxy, write a note, run a snippet in the sandbox without network.
2. Draw the loop as a state machine. Mark each crossing with its authority, the check applied and the data that crosses.
3. Implement the limits: maximum steps, wall-clock time, tokens, number of calls per tool, output size, and an allow-list of tools per task. Add a stop that an operator can trigger from outside the process.
4. Write the tests with the adversarial stub: a model that loops forever, one that calls a tool not on the list, one that asks for ten thousand files, one that ignores the stop. Each must be contained.

### Independent lab (120 min)
Take an open-source agent example from a public repository, run it in the sandbox, and map its workflow in ninety minutes. Produce the diagram, list five authority crossings with what constrains each, and write and run one test that shows the weakest.

### Defence and verification (60 min)
Prove the limits hold under pressure: run the adversarial stub against every limit and the stop, record what each did and the cost incurred before containment, and show the logs allow reconstruction of the run. Remove one limit and show the matching test fails. State which consequences the limits cannot bound (a single valid but harmful call).

### Evidence and report (60 min)
Hand in the state-machine diagram with crossings, the table of limits with the test for each, the stub results and the stop-switch evidence. It is accepted when every crossing names its constraint and every limit has a test that fails without it.

### Resources
- https://genai.owasp.org/resource/agentic-ai-threats-and-mitigations | External | Agentic AI Threats and Mitigations (February 2025) | OWASP project, open licence stated by the project; the project also publishes a later Top 10 for agentic applications, read the current versions; CHECKED 2026-11-14 by search result only, page not opened
- https://modelcontextprotocol.io/specification/ | External | pin the revision you test; 2025-11-25 exists, 2026-07-28 is reported | open specification, licence to be checked; revision dates are from third-party reports and search results; CHECKED 2026-11-14 by search result only, page not opened

### Watch for
A stop that asks the model to stop. A limit counted in model steps while the tool loops inside itself. A tool list longer than the task.

## D56 — Identities, scopes and excessive agency

**Hand in.** Authorization specification — the identities in the system, the scopes each tool needs and has, the excess found, and the backend-enforced approval protocol with its tests.

### Objectives
1. Examine identities/scopes — list the identities in the system (user, agent, each tool backend) and the credentials and scopes each tool call carries, and compare them with the minimum the task needs [agent.identity-scopes]
2. Identify excessive agency — find the tools that have more functionality, permission or autonomy than their task needs, and show the harm a wrong call could do with each [agent.identity-scopes]
3. Design backend approvals — design and implement approval for consequential actions in the backend, bound to the exact operation, resource and requester, single use and expiring, and test it against replay and tampering [agent.identity-scopes, api.token-authz]

### Recall (15 min)
Without notes: list the six limits you put on the agent. Which of them stops a loop inside a tool? Why must a kill switch not rely on the model?

### Theory (45 min)
- Whose authority does a call carry? Three answers exist and only one is safe by default: the agent's own broad service credential (the agent can do anything the credential can, for anyone who asks), a credential delegated from the user for this task and these objects (the call can do what the user could), or none (the tool is only a proposal). Prefer delegated, narrow, short-lived credentials.
- Excessive agency has three parts: tools with more functionality than needed (a tool that can delete when the task is to read), with more permission than their function needs (a database role that can write everywhere), and with more autonomy than the consequences allow (sending, paying, deleting without a person).
- Approval is a control only if the approver sees what will happen and the approval binds to exactly that: the operation, the resource, the parameters, the requester, an expiry and a single use, verified by the backend when the call is made and not by the agent. The approval prompt must be produced by trusted code from the stored request, never from text the model wrote.
- The approval design of this program's application is a worked example: a request records the operation hash, the resource and its revision; the grant is made through a confirmation the application itself shows; the grant is consumed once; a message authentication code binds it.

### Guided lab (120 min)
1. List the identities and for each tool the credential it runs with and the scopes that credential holds. Compare with the minimum each task needs and mark the excess.
2. Add a destructive tool (delete a ticket in the sandbox service) and a sending tool. Demonstrate with a scripted model the harm of an over-broad credential: the agent deletes a ticket the requester does not own.
3. Replace ambient credentials with delegated tokens: the backend issues a token per task bound to the requester, the objects and the scopes. Show the same scripted model's call is now refused.
4. Implement the approval: the agent proposes, the backend stores the request with the exact parameters, a person approves through a separate channel that displays the stored request, the backend verifies and consumes the approval when the call arrives. Test replay, changed parameters, a different resource, an expired approval and an approval of someone else's request.

### Independent lab (120 min)
Review a design in which an agent runs tools with credentials taken from environment variables of the tool process. Write the findings, refactor the design to scoped delegated tokens on paper and in code for one tool, and prove with a test that a token for one task cannot be used for another.

### Defence and verification (60 min)
Write a scope audit: a script that lists every tool, the credential and scopes it holds, the scopes it actually used in the last runs (from the logs) and the difference. Run it before and after the refactor. Alert on any tool holding a scope it has never used. State what usage-based review cannot tell you (a scope needed rarely but needed).

### Evidence and report (60 min)
Hand in the identity table with excess marked, the demonstration of the over-broad credential, the delegated-token design, the approval protocol and its five failure tests, and the scope audit output. It is accepted when every tool's credential is no wider than its task and every approval test fails the attack it models.

### Resources
- https://www.rfc-editor.org/rfc/rfc9700 | External | RFC 9700 (January 2025), best current practice for OAuth 2.0 security | IETF document, freely available; version to be confirmed on the page, ASSUMED
- https://www.rfc-editor.org/rfc/rfc8707 | External | RFC 8707 (February 2020), resource indicators for OAuth 2.0 | IETF document, freely available
- https://genai.owasp.org/llm-top-10/ | External | the project's Top 10 for LLM Applications; read the entry on excessive agency in the current edition | OWASP project, open licence stated by the project; CHECKED 2026-11-14 by search result only, page not opened

### Watch for
An approval prompt written by the model. A token valid for every tool. Approving the description of an action rather than the action.

## D57 — MCP transport, actors and tokens

**Hand in.** MCP security report — the actors and transports of your setup, the authorization checks with the tests for each, the attacks on tool descriptions and on tokens demonstrated in the lab, and the regression suite.

### Objectives
1. Analyze MCP transport and actors — identify the host, client, servers, authorization server and resource server in a working setup, the transport each uses, and what each trusts [mcp.transport-auth]
2. Examine token audience and exfil risk — show how a token for one server presented to another is or is not accepted, how a tool description or result can carry instructions, and how a tool with a destination argument can move data out [mcp.transport-auth, api.token-authz]
3. Define MCP regression cases — write tests that fail on the vulnerable server and pass on the fixed one for audience, expiry, tool-definition change, destination arguments and path arguments [mcp.transport-auth, python.testing-fixtures]

### Recall (15 min)
Without notes: draw the approval protocol and mark what is bound. Which three kinds of excess does a tool have? What does a scope audit based on usage miss?

### Theory (45 min)
- The protocol connects a host application, which runs one client per server, to servers that offer tools, resources and prompts over a transport (a local process over standard streams, or HTTP). A local server runs with the user's authority and often with their environment, so installing one is installing code. A remote server needs authorization: the specification treats it as a resource server of an authorization framework based on OAuth, with protected-resource metadata (RFC 9728), resource indicators that bind a token to the server it is for (RFC 8707), proof key for code exchange, and the explicit rule that a server must not pass a token it received on to another service (token passthrough).
- Named problems in the specification's own security guidance: the confused deputy (a server acts for a client with authority the client did not grant), token passthrough, session hijacking, and local-server compromise. Added by the research community: tool poisoning (instructions hidden in a tool's description, which the model reads and the user does not), changes to a tool's definition after it was approved, a tool that shadows another's name, and results that carry instructions.
- Defences are the ones you know: audience-bound short-lived tokens, per-tool consent that shows the real arguments, pinning of tool definitions by hash with an alert on change, sandboxing of local servers with no ambient credentials, egress allow-lists, argument validation, logging.
- The specification is revised often. Pin the revision; read its change log; do not assume a rule you learnt is still in force.

### Guided lab (120 min)
1. Write a small server with an official software development kit offering three tools (a note store, a URL fetcher, a file reader), and a client. Run it over standard streams first, then over HTTP behind your local authorization server.
2. Capture the messages and mark the actors, what each trusts and what each checks. Show the token's audience and what the server does with a token issued for another server.
3. Plant a harmless instruction in a tool description (a marker the model should print) and show the client passes it to the model. Change a tool's definition after the user approved it and show your client notices (pinned hash) or does not.
4. Give the fetcher a destination argument and show a model-proposed call moving a canary to an address in your lab. Add the defences: audience check, definition pinning, destination allow-list, display of the exact arguments for consent, path confinement for the file reader.

### Independent lab (120 min)
Read the code of three open-source servers for this protocol (do not run them outside the sandbox). For each list the authority it holds, the inputs it trusts, whether it handles tokens, and its install path (who can change it). Rate their risk and write the sandbox configuration you would run each in.

### Defence and verification (60 min)
Build the regression suite: audience mismatch, expired token, a token for the wrong resource, definition drift, a destination outside the allow-list, a path escaping the directory, a secret appearing in a log. Run it against the vulnerable and fixed servers. State what the suite cannot cover (a revision of the protocol that changes a rule).

### Evidence and report (60 min)
Hand in the actor diagram with trust annotations, the captured messages, the demonstrations, the defences and the suite results on both servers, and the three server audits. It is accepted when every test fails on the vulnerable server and passes on the fixed one, and the specification revision you tested against is recorded.

### Resources
- https://modelcontextprotocol.io/specification/ | External | pin the revision you test; 2025-11-25 exists, 2026-07-28 is reported | open specification and its security best practices page, read the change log; licence to be checked; CHECKED 2026-11-14 by search result only, page not opened
- https://www.rfc-editor.org/rfc/rfc9728 | External | RFC 9728, OAuth 2.0 protected resource metadata | IETF document, freely available; date to be confirmed on the page, ASSUMED
- https://www.rfc-editor.org/rfc/rfc8707 | External | RFC 8707 (February 2020), resource indicators for OAuth 2.0 | IETF document, freely available

### Watch for
Forwarding the client's token to a downstream service. A consent screen that shows the tool's name and not its arguments. A pinned hash of the description that ignores the schema.

## D58 — Persistent memory, poisoning and provenance

**Hand in.** Memory integrity plan — the write and read paths of the agent's memory with trust labels, the poisoning fixtures and their results before and after, and the correction, rollback and deletion procedures with their tests.

### Objectives
1. Explain persistent memory trust — explain how a stored memory carries an earlier input's influence into later sessions and other users, and why the memory's contents need provenance and a trust label [agent.memory-integrity]
2. Evaluate isolated poisoning fixtures — build harmless fixtures that plant a false or instructing memory through a document, a tool result and another user's session, and measure how often a later session follows it [agent.memory-integrity]
3. Design correction/provenance — design write gating, labelled reads, expiry, review, correction and deletion that reach every copy (cache, embedding, backup of the store), and test them [agent.memory-integrity, rag.ingestion-provenance]

### Recall (15 min)
Without notes: name the five parts of a specification revision pin you would record. What is the confused deputy? Why is "the user approved the tool" not a defence against a changed description?

### Theory (45 min)
- Memory turns a one-time input into a standing one. If an untrusted document can cause the agent to store "always send reports to this address", every later session inherits the instruction, perhaps for a different user. Poisoned memory is persistence for text.
- Controls on the write path: decide what may be stored (facts the user stated about themselves, not instructions from retrieved content), record the source session, the source type and the trust level, require confirmation for memories that carry preferences about authority, expire entries, and rate-limit writes.
- Controls on the read path: place memories in the prompt as labelled data with their provenance, never as system instructions, and never let a memory grant authority (authority comes from the backend, week 10 day 56).
- Correction and deletion must reach every copy: the store, the index of embeddings, caches, derived summaries, exports and backups. Test that a deleted memory is really not retrievable and not followed.
- Read the section of the agentic threat guidance on memory poisoning.

### Guided lab (120 min)
1. Add a memory store: after each session the agent proposes facts to remember, and a writer stores them with source, session, user and time.
2. Write three poisoning fixtures with harmless goals (a stored instruction to print a marker, a stored false preference about a recipient, a stored claim about a user's role): one planted through a retrieved document, one through a tool result, one by another user in a shared memory.
3. Run each twenty times in a later session and record how often the agent follows it, with an interval.
4. Add the controls: gated writes, labelled reads, no authority from memory, expiry, a review command. Rerun the fixtures. Then delete a poisoned memory and prove it is gone from the store, the embedding index and the cache.

### Independent lab (120 min)
Design and run three further fixtures: a delayed trigger (the memory acts only after a condition), a memory written by gradual drift over several sessions, and a poisoned summary of a long conversation. Evaluate your controls against them and record what they missed. Hold out one fixture family until the end to check you have not tuned to your own tests.

### Defence and verification (60 min)
Write the memory integrity tests: a write from untrusted content is refused or labelled, a read never produces a system-level instruction, deletion reaches every copy, a rollback to a snapshot restores a known-good state. Run them with the stub and with the real model. State what the plan does not protect (a poisoned fact that looks like a normal preference).

### Evidence and report (60 min)
Hand in the path diagram with trust labels, the fixtures with before and after rates and intervals, the deletion proof, the rollback procedure and the test output. It is accepted when each poisoning fixture has a measured rate before and after and deletion is shown across every copy.

### Resources
- https://genai.owasp.org/resource/agentic-ai-threats-and-mitigations | External | Agentic AI Threats and Mitigations (February 2025); read the sections on memory in the current version | OWASP project, open licence stated by the project; CHECKED 2026-11-14 by search result only, page not opened
- https://csrc.nist.gov/pubs/ai/100/2/e2025/final | External | NIST AI 100-2 E2025 (March 2025) | NIST publication, freely available; CHECKED 2026-11-14 by search result only, page not opened

### Watch for
Letting the memory writer see the untrusted content as instructions. Deleting the row and leaving the embedding. Tuning controls to the fixtures you will report.

## D59 — A bounded red/blue experiment

**Hand in.** Agent evaluation — the pre-registered design, the rounds with their measured attack success and task success, the ablations, and the report with limits.

### Objectives
1. Define bounded red/blue experiment — write, before running anything, a design with hypotheses, the attack set, the success metrics, the number of trials, the stopping rules, the controls to enable in each round and the bounds that contain the experiment [agent.redblue-experiment]
2. Observe behavior — run the rounds, capture the traces, and record attack success, task success and side effects with intervals and the model and settings used [agent.redblue-experiment]
3. Verify defensive controls — show with ablations that each control matters (remove it, the attack success rises) and that the controls do not destroy the task [agent.redblue-experiment]

### Recall (15 min)
Without notes: list three controls on the memory write path and two on the read path. What are the copies a deletion must reach? What made one of your fixtures delayed?

### Theory (45 min)
- An experiment answers a question you wrote down before you saw the data. Pre-registration means the hypotheses, the metrics, the sample sizes and the analysis are fixed first, so that you cannot look for a pattern after the fact. This program's own rules for single-learner experiments take the same view.
- Bounded means safe by construction: a sandbox, synthetic data, harmless goals, a budget, a kill switch, and a stated list of what will not be tried. The attacker may be a scripted set of inputs and, if you choose, an attacking agent inside the same bounds.
- Measure two things: attack success (the harmless goal reached) and task success (the legitimate task completed). A defence that stops every attack by stopping every task is not a defence. Report both with intervals.
- An ablation removes one control at a time to show what each contributes. A held-out attack family tests whether you tuned the defences to the attacks you knew.

### Guided lab (120 min)
1. Write the design: two hypotheses (the structural controls remove the impact-bearing attacks; the statistical controls lower but do not remove the rest), the attack set of forty inputs in four families, the metrics, twenty trials per input, the controls for rounds A (none), B (statistical), C (structural), D (all), the bounds and the stop.
2. Freeze it in git with a timestamp before running.
3. Run the four rounds. Capture traces. Compute attack success and task success with intervals.
4. Run the ablations: from D remove one control at a time and record the change.

### Independent lab (120 min)
Run a held-out fourth family of attacks that you wrote but did not use while designing the controls, and an attacking agent with a bounded goal. Compare the result with the earlier rounds and state whether the defences generalised. Record every deviation from your pre-registered plan and why.

### Defence and verification (60 min)
Turn the experiment into a repeatable check: one command that reruns the design, stores the results with model hash, settings and date, and compares them with the previous run using the intervals. Show it detects a weakened control. Write the one-page limits statement: what was not tried, where the sample is thin, what would change the conclusion.

### Evidence and report (60 min)
Hand in the frozen design, the rounds' results with intervals, the ablations, the held-out result, the deviations list and the limits statement. It is accepted when the design predates the data in version control and every claim carries an interval and the settings.

### Resources
- https://csrc.nist.gov/pubs/ai/100/2/e2025/final | External | NIST AI 100-2 E2025 (March 2025) | NIST publication, freely available; CHECKED 2026-11-14 by search result only, page not opened
- https://atlas.mitre.org/ | External | MITRE ATLAS knowledge base; release 2026.05 is reported, pin the release you read | MITRE terms of use; CHECKED 2026-11-14 by search result only, page not opened

### Watch for
Writing the hypotheses after seeing the table. A defence that wins by refusing the task. Omitting the runs that did not turn out as expected.

## D60 — Week 10 assessment (blind practical)

**Hand in.** Week 10 assessment — the report of the blind practical: the agent's workflow and authority crossings, the authorization failures found and explained, and the recommended controls with the evidence that they hold.

### Objectives
1. Analyze unfamiliar agent workflow — map the workflow of an unseen agent, its tools, identities and authority crossings, from its running behaviour and configuration [agent.workflow-authority, agent.identity-scopes]
2. Explain authorization failure — for each failure say whose authority the call carried, what should have bound it, and which control was missing, including any failure in the protocol layer [agent.identity-scopes, mcp.transport-auth]
3. Recommend control — recommend the control for each failure and test it in a bounded experiment, reporting success rates with intervals and the limits [agent.redblue-experiment]

### Blind practical (120 min)
Open the assessment in the application, start the attempt and read the task statement. Take the agent from the source you fixed before the week and do not read its code, its configuration or its list of defects before the clock starts. Stop at 120 minutes.

### Remediation (60 min)
Grade yourself against the rubric before reading any answer key. For every criterion below your target find the cause in your method (the crossing you did not mark, the identity you assumed) and repair it with a short exercise on your own agent. Then redo the failed part on a different instance.

### Report (60 min)
Write the report in the structure of the task statement: map, failures with identity and missing control, tests and experiment results, limits. Attach the evidence with SHA-256 values.

### Performance review (60 min)
Compare the grade with the week's targets: independent execution, every crossing marked, identity named for every call, controls tested with a stub and with measured rates, honest limits. Record the three habits that cost you most and one change for next week.

### Assessment
- Code: W10-BLIND
- Title: Week 10 blind practical: authority and authorization failures in an unfamiliar agent
- Kind: BLIND_PRACTICAL
- Competency: agent.workflow-authority
- Difficulty: 3

Task statement:
You have 120 minutes. You are given an agent you have not seen, with its tools, possibly an MCP server or two, a local model and a sandbox, on your isolated lab network with synthetic data. Work only inside the lab and on the agent named in your scope statement. Every adversarial goal must be harmless.

Scenario source (fixed before the week, not inspected): an agent prepared by another person with failures planted from the catalogue below, or, working alone, a variant of your own agent produced by a script you wrote earlier but have not read since. Failures are drawn from: tools running with the agent's broad credential instead of the requester's; an approval that is not bound to the parameters; a token accepted by a server it was not issued for; a tool description that carries an instruction or changes after approval; a tool with a destination or path argument that is not confined; memory that stores instructions from untrusted content; limits that exist in the model's prompt and not in code.

Deliverables, in this order. First, the workflow as a state machine with the identity and the check at each authority crossing. Second, the failures, each with a harmless reproduction and the identity the call carried. Third, for each failure the missing control and the smallest implementation of it. Fourth, tests: guarantees with a scripted adversarial model, and one pre-registered mini experiment (written before running) for a model-dependent control with success rates and intervals. Fifth, residual risk and what you did not test, including the protocol revision you tested. Sixth, a report of at most four pages with the evidence hashes.

You are assessed on: independent execution during the block; every authority crossing marked; the identity named for every call; controls tested with a stub and with measured rates where the model matters; honest limits. The pass rule is non-compensatory: every criterion must reach the pass level.

### Resources
- https://modelcontextprotocol.io/specification/ | External | protocol specification; pin the revision you test against and read its change log | open specification, licence to be checked; CHECKED 2026-11-14 by search result only, page not opened

### Watch for
Assuming the agent acts as the user. Testing the model's compliance instead of the code's enforcement. Writing the experiment's hypothesis after the run.
