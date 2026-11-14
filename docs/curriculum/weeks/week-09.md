# Week 9 — LLM application and RAG security

**Purpose.** A language model is a component that turns text into text and cannot tell instructions from data. An application built around one inherits that property: whatever reaches the model, from the user, a document, a web page or a tool, can try to steer it. This week you build a small retrieval-augmented application of your own, map its trust boundaries, attack it with harmless goals, and learn which defences are guarantees (deterministic code outside the model) and which are statistics (behaviour of the model, measured with intervals). The rule that runs through the week is the one this program's own application follows: model output is data, never an instruction and never authoritative, and authority is decided in code.

**Lab stack.** The week 8 lab plus an application guest with no network access except to a local model server. A local model runtime (llama.cpp's server or Ollama) with a small open-weight instruction model whose licence permits your use (record the name, version, file hash and quantisation), a local embedding model, a vector store you can inspect (SQLite-based or a Python library), and a small web or command-line front end of your own. Evaluation tools: `pytest`, `promptfoo`, `garak` or an equivalent scanner. A synthetic corpus of company-style documents for three tenants and three roles, with canary strings (unique, meaningless tokens) planted in restricted documents. No real data, no network egress, and no attempt to make the model produce harmful content: every adversarial goal in the week is harmless (reveal a canary, call a tool with a marker).

**Competencies.** `llm.app-mapping`, `rag.ingestion-provenance`, `llm.prompt-injection`, `llm.output-schema-tools`, `llm.independent-eval`, `llm.trust-failure-diagnosis`, `llm.data-leakage`, `llm.eval-leakage` (pack `llm-rag-security`), with `web.authz-idor`, `web.injection` and `api.token-authz` revisited.

**Accelerator.** An engineer who has built retrieval applications can write the lab application in half the time and spend the saved hours on the independent labs; the evaluation design of day 53 and the Saturday assessment are taken unchanged.

## D49 — Mapping an LLM application and its threat model

**Hand in.** LLM threat model — the data-flow diagram of your application with trust boundaries, assets and entry points, the threats ranked with reasons, and the security requirements each threat gives rise to.

### Objectives
1. Map LLM application — draw the components of the application (front end, orchestration, system prompt, retrieval, model, tools, memory, logs) and the data that flows between them, from the code and the running system [llm.app-mapping]
2. Identify data/tool boundaries — mark who can write each input (users, document authors, tool providers), which inputs reach the model, which outputs reach tools or other users, and what authority each tool carries [llm.app-mapping]
3. Threat-model application — enumerate threats for the model, the retrieval path, the tools and the logs, rank them with evidence, and write each as a testable security requirement [llm.app-mapping]

### Recall (15 min)
Without notes: which controls of week 8 would you apply before touching a memory image? Name the two kinds of evidence a timeline mixes. What did your hunts report as their coverage?

### Theory (45 min)
- An LLM application is a pipeline around a model: input from a user, a system prompt from the developer, context retrieved from stores, tool definitions and tool results, and the model's output, which is shown to a user or passed to code. The model sees all of it as one stream of text. That is the root of prompt injection: data and instructions share a channel.
- The model is not a security boundary and cannot enforce an authorization rule. Authorization, isolation and validation belong in code before the model (what it may see) and after it (what its output may do). Assume the system prompt can be disclosed and put no secret in it.
- A threat model of such an application asks the usual questions of each element (who can write this, who can read it, what can it cause) and adds: what happens if the model follows an instruction that was inside data; what happens if its output is wrong, malformed or hostile; what happens if two users share a cache, a memory or an index.
- Read the project's list of the most serious risks for applications built on language models (the current edition) and the national taxonomy of attacks on machine learning systems for vocabulary.

### Guided lab (120 min)
1. Build version 0 of the application: a question-answering tool over a synthetic corpus for three tenants and three roles, with retrieval by similarity, a system prompt, and three tools (search documents, create a ticket in a sandbox, send a message to a sandbox address). Keep it small enough to read in an hour.
2. From the code and by running it, list every route by which text reaches the model and every route by which model output reaches a person, a tool or a store.
3. Draw the data-flow diagram with trust boundaries and mark the assets (system prompt, restricted documents, tool credentials, logs).
4. Write threats for each element and boundary, rank them, and write twelve security requirements that a test could check (for example "retrieval returns only documents the requesting user may read, decided in code before the prompt is built").

### Independent lab (120 min)
Run a different open-source retrieval application from a public repository in your lab, offline, and map it in ninety minutes from its code and behaviour alone. Produce the diagram, the five threats you rank highest and one test that would show the most serious. Then compare your map with its documentation and note what it did not tell you.

### Defence and verification (60 min)
Turn the twelve requirements into a skeleton test file: for each a test with a name, a setup and an expected outcome, most of which fail or are skipped today. Implement the three that need no model at all (the retrieval filter, the tool allow-list, the output size limit) and run them against the application. Record which requirements can be guaranteed by code and which can only be measured.

### Evidence and report (60 min)
Hand in the diagram, the entry-point and authority table, the ranked threats with reasoning, the twelve requirements and the test skeleton with the results of the three implemented tests. It is accepted when every threat points to a requirement and every requirement is marked "guaranteed by code" or "measured".

### Resources
- https://genai.owasp.org/llm-top-10/ | External | 2025 edition (November 2024); read the current edition | OWASP project, open licence stated by the project; a 2026 edition is listed by the project; CHECKED 2026-11-14 by search result only, page not opened
- https://csrc.nist.gov/pubs/ai/100/2/e2025/final | External | NIST AI 100-2 E2025 (March 2025) | NIST publication, freely available; CHECKED 2026-11-14 by search result only, page not opened
- https://simonwillison.net/series/prompt-injection/ | External | series of articles, read the ones dated when you study | personal publication, free to read, ASSUMED

### Watch for
Putting a secret in the system prompt. Treating the model's refusal as access control. A diagram that omits the logs and the cache.

## D50 — RAG ingestion, retrieval and source trust

**Hand in.** RAG review — the ingestion pipeline with provenance recorded for every document, the retrieval controls, the tests with planted instructions and canaries, and the measured difference between a prompt-level and a code-level defence.

### Objectives
1. Explain RAG ingestion/retrieval — explain how documents are parsed, split, embedded, indexed and retrieved, and what is lost or added at each step (metadata, access control, hidden text) [rag.ingestion-provenance]
2. Analyze source trust — classify the sources of the corpus by who can write them and how much they are trusted, and show how an untrusted document can steer the answer to another user's question [rag.ingestion-provenance]
3. Test provenance controls — implement and test source labels, quarantine of untrusted sources, retrieval-time access control and hidden-content stripping, and show which ones hold regardless of the model [rag.ingestion-provenance, llm.data-leakage]

### Recall (15 min)
Without notes: name five assets of your application. Which two of your requirements need no model to test? Why may a system prompt not hold a secret?

### Theory (45 min)
- Retrieval brings text from stores into the prompt. Each stage can change its meaning: parsers drop or add text (hidden text in a web page or document, text in comments or metadata), chunking cuts context, embedding discards access control unless you carry it as metadata, similarity search returns what is near and not what is permitted.
- A document that a user can write, or that is fetched from somewhere they can influence, is untrusted input to every other user's question. It can contain instructions (indirect injection) or false facts. Source trust is a property you record at ingestion, with the document's hash, origin, author class and time.
- Access control must be enforced at retrieval, in code, with the requester's permissions, before text is placed in the prompt. A prompt that says "do not reveal documents the user may not see" is a request, not a control. A shared index across tenants needs a filter the application sets, never one the model or the user supplies.
- Embeddings and indexes are data stores that need the same protection, retention and deletion as the documents they came from.

### Guided lab (120 min)
1. Extend the application's ingestion with metadata per chunk: source, document hash, author class, tenant, readers, trust level, ingest time.
2. Plant three test documents: one that contains a harmless instruction (when asked about the product, also print a marker), one restricted document with a canary string, one with text hidden by formatting.
3. Show the failures: the marker appears in another user's answer, the canary is returned to a user who may not read it when the question is close enough, hidden text reaches the prompt.
4. Fix them in code: filter at retrieval by the requester's permissions, quarantine untrusted sources and label their text as untrusted data in the prompt, strip hidden content at ingestion. Rerun with twenty trials each and record the rates.

### Independent lab (120 min)
Ingest a set of web pages and PDFs you did not prepare for the test (a public documentation set, a set of newsletters). Examine what the parsers do with hidden text, comments, metadata, alternative text and tables. Write an ingestion check that reports content present in the file but invisible to a reader, run it on the set, and count what it finds.

### Defence and verification (60 min)
Add monitoring: log for every answer the identifiers of the chunks used, their trust levels and the requester; alert when a chunk of a lower trust level than the request's policy allows is used, or when a canary appears in any output. Test with the three planted documents. State what the logging cannot show (the model's reasoning) and what it would cost to keep.

### Evidence and report (60 min)
Hand in the pipeline description with the metadata schema, the three planted documents and the before and after rates, the hidden-content check and its counts, and the monitoring tests. It is accepted when the retrieval filter has a test that fails without it and the prompt-level mitigation is reported with its measured rate rather than as a guarantee.

### Resources
- https://genai.owasp.org/llm-top-10/ | External | current edition; note which you read | OWASP project, open licence stated by the project; read the entries on retrieval and embedding weaknesses and on sensitive information disclosure; CHECKED 2026-11-14 by search result only, page not opened
- https://github.com/ggml-org/llama.cpp | External | current release; pin the build and the model file hash you run | open source project, repository location to be confirmed, ASSUMED

### Watch for
Filtering after generation instead of before. Letting the query supply its own tenant filter. A hidden-text check that only looks at one parser's output.

## D51 — Prompt injection entry points and the authorization boundary

**Hand in.** Injection evaluation — the entry-point taxonomy, the fixture set with its results over repeated trials and intervals, and the authorization tests that hold regardless of the model.

### Objectives
1. Classify prompt-injection entry — classify the ways untrusted text reaches the model (direct input, retrieved documents, tool outputs, file and field names, metadata, rendered content) and the goals an injected instruction can have (reveal, act, mislead, persist) [llm.prompt-injection]
2. Run controlled fixtures — build and run a set of harmless injection fixtures against your application over repeated trials, and report the success rate of each with an interval [llm.prompt-injection]
3. Test authorization boundary — show that an injected instruction asking for a tool or a document the requester may not use is refused by code, whatever the model does [llm.prompt-injection, llm.data-leakage]

### Recall (15 min)
Without notes: list the four stages of your ingestion where text can change meaning. What is the difference between a filter at retrieval and a warning in the prompt? Name the trust levels you recorded.

### Theory (45 min)
- Direct injection is the user's own text trying to override the developer's instructions; indirect injection arrives in content the application fetches on the user's behalf. The second is the serious one because the victim never typed it. Goals include disclosing context, causing an action through a tool, producing misleading output, and storing an instruction for later.
- Defences differ in kind. Statistical: instructions to the model, delimiters, filters on input and output, classifiers; they lower the rate and never reach zero, and attackers adapt. Structural: remove the capability (a tool that cannot send data out cannot exfiltrate), limit authority to the requester's own, require approval for sensitive actions, block outbound channels such as rendered links and images, isolate untrusted content from control flow.
- Because model behaviour varies between runs, a single success or failure means nothing. Repeat each fixture many times at fixed settings, report the success rate with an interval, and keep the model version and settings with the result.
- Read the project's description of the risk and at least one practitioner's account of attacks through documents and tools.

### Guided lab (120 min)
1. Write thirty fixtures with harmless goals: print a marker, reveal a canary, call a tool with a marker argument, try to call a tool the requester cannot use, try to send retrieved text to an address. Spread them across entry points: the user's message, a retrieved document, a tool result, a file name, a field of metadata.
2. Run each twenty times at fixed settings. Record the success rate and a Wilson interval, with model, quantisation and settings.
3. For every fixture that tried to use a tool or document beyond the requester's authority, show whether the attempt was refused by code. Write the test: even with a scripted model that always makes the attempt, the call is denied.
4. Rank the fixtures by rate and by impact.

### Independent lab (120 min)
Run a scanner (`garak` or the red-team features of `promptfoo`) against your application. Triage its findings: which are true failures of the application, which are model behaviours with no impact because of structural controls, which are noise. Compare with your fixtures' results and list what each approach found that the other did not.

### Defence and verification (60 min)
Apply structural defences: authority of every tool call derived from the requester (not from the model's arguments), approval required for the sensitive tool with the exact arguments shown, outbound content stripped of links and images to other origins. Rerun the thirty fixtures and show the impact-bearing ones drop to zero by construction while the statistical ones keep a measured rate. Record what the defences cost in usability.

### Evidence and report (60 min)
Hand in the taxonomy, the fixtures with rates and intervals, the scripted-model authorization tests, the scanner triage and the before and after table. It is accepted when every impact-bearing fixture is stopped by a deterministic test and every rate carries its interval and settings.

### Resources
- https://www.promptfoo.dev/docs/ | External | current documentation; pin the version you run | open source project, ASSUMED
- https://github.com/NVIDIA/garak | External | current release; pin the version you run | open source project, ASSUMED
- https://genai.owasp.org/llm-top-10/ | External | the project's Top 10 for LLM Applications; read the entry on prompt injection in the current edition | OWASP project, open licence stated by the project; CHECKED 2026-11-14 by search result only, page not opened

### Watch for
Concluding "it works" from three runs. A scripted-model test that never tries the forbidden call. Counting a refusal by the model as a refusal by the application.

## D52 — Output handling, schemas and unsafe tools

**Hand in.** Tool safety matrix — every tool by authority, input validation, side effects, approval, logging, limits and sandbox, with the test for each cell, and the typed enforcement at the boundary between model output and code.

### Objectives
1. Analyze output schema failures — show how malformed, extra, mistyped or hostile model output breaks code that parses or uses it, and where the output becomes input to another interpreter (SQL, shell, HTML, a URL) [llm.output-schema-tools]
2. Examine unsafe tools — review each tool of the application for authority, inputs, side effects and reachable resources, and find the unsafe patterns (arbitrary path or address, command construction, broad credentials, no limits) [llm.output-schema-tools]
3. Define typed enforcement — specify and implement validation of every model-proposed call against a schema and a policy in code before anything executes [llm.output-schema-tools]

### Recall (15 min)
Without notes: write the confidence interval method you used for success rates. Which of your fixtures changed behaviour with the model's settings? Name two structural defences and what each removes.

### Theory (45 min)
- Output from a model is untrusted input for whatever consumes it. If it is placed in a page it is a script-injection risk; if it is placed in a query, a command, a path or a URL it is an injection or request-forgery risk; if it is parsed it may be malformed. All of week 3 and week 4 applies to it.
- A tool is an authority grant: it carries credentials, can have side effects and can reach resources. The questions to ask of every tool are who it acts for, what it can touch, what happens on a wrong call, and how a wrong call is noticed.
- Typed enforcement means the model proposes and code decides: the proposed call is parsed against a schema (fields, types, ranges, lengths, allowed values), checked against policy (is this requester allowed to do this to this object), limited (rate, size, time) and recorded. Anything else is refused and the refusal is returned to the model as data.
- Dry-run modes, idempotency keys and approvals for destructive actions bound the cost of the cases that slip through.

### Guided lab (120 min)
1. Add to the application three tools in unsafe form: a query tool that builds SQL from the model's text, a fetch tool that retrieves any address, a file tool that takes a path.
2. With harmless markers show each failure: a query that returns another tenant's row, a fetch of an internal address of your lab, a path outside the sandbox directory. Use a scripted model so the result is reproducible.
3. Replace each with a typed version: parameterised queries with an allow-list of columns, a destination allow-list with the checks of week 4, a path resolved inside the sandbox directory. Add schema validation of every call, a per-requester policy check and a size and rate limit.
4. Test every cell of the matrix with valid, boundary and hostile arguments.

### Independent lab (120 min)
Write five tool definitions that a colleague might plausibly add (read a file, run a command, fetch a web page, query a database, send an email) in their naive form. Audit each for authority, inputs, side effects and reachable resources, rank them by risk, and write the minimal safe form and the test that would catch the naive one.

### Defence and verification (60 min)
Fuzz the validation layer with hostile arguments (very long values, nested structures, type confusion, path and address tricks, encodings) generated by a script, and show every invalid call is refused with a recorded reason and no side effect. Add the approval path for the one destructive tool and test replay of an approval with changed arguments. State what the validation cannot know (whether a valid call is a wise one).

### Evidence and report (60 min)
Hand in the matrix with a test name for each cell, the naive and safe versions with their results, the fuzzing summary and the approval tests. It is accepted when no invalid call reaches a side effect and each cell is backed by a test that fails on the naive tool.

### Resources
- https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html | External | current cheat sheet; note the revision date you read | OWASP project, open licence stated by the project, ASSUMED
- https://json-schema.org/specification | External | current specification; note the draft you use | open specification, ASSUMED

### Watch for
Validating the shape and not the meaning of an argument. A tool that runs with the application's own credentials when the requester has fewer. An error message that returns internal detail to the model.

## D53 — Independent evaluation design

**Hand in.** LLM eval report — the evaluation protocol, the independent test set, the results with intervals for the model-dependent measures and the pass/fail of the deterministic controls, and the limitations.

### Objectives
1. Design independent eval cases — write evaluation cases that were not used to build the defences, partition them into development and held-out sets, and state the scoring rule for each before running [llm.independent-eval]
2. Separate model from app security — test the application's guarantees with an adversarial model stub so that they are shown to hold whatever the model does, and measure the real model's behaviour separately [llm.independent-eval, llm.app-mapping]
3. Report actual limitations — report the results with their intervals, the settings and versions, and what the evaluation cannot show, including the risk that the test data leaked into your tuning [llm.independent-eval, llm.eval-leakage]

### Recall (15 min)
Without notes: for each of your tools name the three questions to ask of it. What does a typed boundary do with a refused call? Name a defence that cannot be shown by a unit test.

### Theory (45 min)
- Two different claims need two different tests. "The application never returns a document the requester may not read" is a guarantee: test it with a stub that behaves as badly as an attacker could, and expect no failures. "The model usually refuses an injected instruction" is a statistic: repeat, report the rate with an interval, and keep the version and settings.
- Independent means the test cases were not used to build or tune the defences. Write them after the application is frozen, keep a held-out set you do not run until the end, and record what you changed after seeing results. Looking at the test set while tuning leaks it.
- Scoring rules are decided before the run. A model used as a judge is another model whose errors and biases must be measured against labels you made by hand.
- Report: system and model versions and hashes, settings, number of trials, intervals, failures with examples, and the limits of the data and of the cases.

### Guided lab (120 min)
1. Freeze version 1 of the application (all week's defences in). Write sixty new cases across entry points, goals and roles, with an expected outcome and a scoring rule for each. Split into a development set of thirty and a held-out set of thirty.
2. Build an adversarial model stub that always attempts the attack in each case. Run the stub against the whole set: every guarantee test must pass.
3. Run the real model twenty times on the development set. Compute rates and intervals. Fix anything you decide to fix, and write down each change.
4. Run the held-out set once. Report the rates and compare with the development results.

### Independent lab (120 min)
Take an evaluation someone else published (a benchmark description or a vendor's safety claim) and critique its validity in one page: what it measures, how the cases were made, the number of trials, nondeterminism, prompt sensitivity, leakage of test data, and what the reported number does not show. Redo one of its measurements on your application if you can.

### Defence and verification (60 min)
Make the evaluation a gate: a command that runs the guarantee tests with the stub (must be all green) and the statistical tests with a threshold on the upper bound of the failure rate, stores each run with model hash, settings and date, and fails the change if a rate worsens beyond its interval. Introduce a regression (loosen the retrieval filter) and show the gate stops it.

### Evidence and report (60 min)
Hand in the protocol, the cases and the scoring rules, the stub results, the intervals for the real model, the held-out comparison, the critique and the gate's output with and without the regression. It is accepted when the guarantee tests and the statistical tests are reported separately and the held-out set was run once.

### Resources
- https://csrc.nist.gov/pubs/ai/100/2/e2025/final | External | NIST AI 100-2 E2025 (March 2025) | NIST publication, freely available; CHECKED 2026-11-14 by search result only, page not opened
- https://www.nist.gov/itl/ai-risk-management-framework | External | AI Risk Management Framework, current version; note the version you read | NIST publication, freely available, ASSUMED

### Watch for
Tuning on the cases you will report. A single run reported as a rate. Calling a defence "tested" because it passed the cases you wrote it against.

## D54 — Week 9 assessment (blind practical)

**Hand in.** Week 9 assessment — the report of the blind practical: the trust failures found in the unfamiliar application, their root causes in terms of boundaries, the mitigations with deterministic tests, and the measured limits.

### Objectives
1. Evaluate blind AI application — map an unfamiliar LLM application, derive its security requirements and test them with harmless fixtures [llm.trust-failure-diagnosis]
2. Diagnose controlled trust failure — for each failure state which boundary it crosses and why the model was or was not the cause [llm.trust-failure-diagnosis, llm.prompt-injection]
3. Verify mitigation — apply a structural mitigation for each failure and show with a deterministic test (and a measured rate where the model matters) that it holds [llm.output-schema-tools]

### Blind practical (120 min)
Open the assessment in the application, start the attempt and read the task statement. Take the application from the source you fixed before the week and do not read its code, its configuration or its list of defects before the clock starts. Stop at 120 minutes.

### Remediation (60 min)
Grade yourself against the rubric before reading any answer key. For every criterion below your target find the cause in your method (the entry point you did not try, the guarantee you tested only with the real model) and repair it with a short exercise on your own application. Then redo the failed part on a different instance.

### Report (60 min)
Write the report in the structure of the task statement: map, requirements, findings with boundary and root cause, mitigations with tests, measured rates with intervals, limits. Attach the evidence with SHA-256 values.

### Performance review (60 min)
Compare the grade with the week's targets: independent execution, boundary named for every failure, guarantees tested with a stub, statistics reported with intervals, honest limits. Record the three habits that cost you most and one change for next week.

### Assessment
- Code: W09-BLIND
- Title: Week 9 blind practical: diagnose trust failures in an unfamiliar LLM application
- Kind: BLIND_PRACTICAL
- Competency: llm.trust-failure-diagnosis
- Difficulty: 4

Task statement:
You have 120 minutes. You are given a language-model application you have not seen, running on your isolated lab network with a local model, a synthetic corpus for several tenants and roles, and some tools that act only on a sandbox. Work only inside the lab and on the application named in your scope statement. Every adversarial goal must be harmless: reveal a canary, print a marker, call a tool with a marker argument.

Scenario source (fixed before the week, not inspected): an application prepared by another person with failures planted from the catalogue below, or, working alone, a variant of your own application produced by a script you wrote earlier but have not read since, or an unseen deliberately vulnerable application from a public training source. Failures are drawn from: retrieval that ignores the requester's permissions; an untrusted document whose instruction changes another user's answer; a tool whose authority comes from the model's arguments; model output used unsafely by a downstream component; a secret in the system prompt; a cache or memory shared between users; unlimited consumption (loops, very long outputs).

Deliverables, in this order. First, a map of the application with its trust boundaries and the requirements you derive. Second, the findings, each with a harmless reproduction and the boundary it crosses. Third, for each finding whether the model was the cause, the amplifier or irrelevant, with evidence. Fourth, the mitigation for each, a deterministic test (using a scripted adversarial model where the guarantee is about the application) and, where the model matters, a measured rate over repeated trials with an interval. Fifth, residual risk and what you did not test. Sixth, a report of at most four pages with the evidence hashes.

You are assessed on: independent execution during the block; the boundary named for every failure; guarantees tested with a stub rather than the real model; statistics reported with intervals and settings; honest limits. The pass rule is non-compensatory: every criterion must reach the pass level.

### Resources
- https://genai.owasp.org/llm-top-10/ | External | the project's Top 10 for LLM Applications; the current edition | OWASP project, open licence stated by the project; CHECKED 2026-11-14 by search result only, page not opened

### Watch for
Testing only with the real model. Reproductions that need harmful content. Stopping at the first failure.
