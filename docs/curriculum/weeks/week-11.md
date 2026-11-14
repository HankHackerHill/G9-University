# Week 11 — AI red-team, evaluation and supply chain

**Purpose.** The last two weeks secured one application and one agent. This week widens the view to the AI system as a whole: its assets (data, models, indexes, prompts, tools, pipelines), the threats against each across the life cycle (collection, training, packaging, serving, use), how to test those threats honestly, where a model or a dependency comes from and whether you should trust it, what training and evaluation data can leak, and how to design measures that mean what they claim. A red team here is a bounded, pre-registered, harmless experiment, not a performance. The Saturday assessment hands you an unfamiliar AI system and asks for a threat model, bounded tests and findings across all of it.

**Lab stack.** The week 10 lab plus a small machine-learning workbench: Python with scikit-learn and a small synthetic text or tabular dataset for classical models, a model registry directory of your own with hashes and licences, a package index mirror or a hash-pinned requirements file, and `picklescan` or an equivalent. Everything stays on the isolated network. Adversarial work targets your own models and applications with harmless goals; no harmful content is generated, collected or stored.

**Competencies.** `ai.threat-model-redteam`, `ai.supply-chain`, `ai.data-governance-eval` (pack `agentic-mcp-supplychain`), with `agent.redblue-experiment`, `llm.independent-eval`, `llm.data-leakage`, `llm.eval-leakage` and `python.testing-fixtures` revisited.

**Accelerator.** A machine-learning engineer who can name the assets and life-cycle stages of an AI system and the attacks of each from memory in the Monday recall block can treat day 61 as review; days 62 to 65 and the Saturday assessment stay.

## D61 — AI assets, threats and test objectives

**Hand in.** AI threat model — the assets and life-cycle stages of an AI system, the threats against each with ranking and evidence, and the test objectives with success criteria.

### Objectives
1. Identify AI assets/threats — list the assets of an AI system across its life cycle (data, labels, features, models, weights, indexes, prompts, tools, credentials, pipelines, logs, evaluation sets) and the classes of threat against each [ai.threat-model-redteam]
2. Prioritize failure modes — rank the failure modes by impact and likelihood with evidence from your own systems and from public taxonomies, and justify the order [ai.threat-model-redteam]
3. Define test objectives — turn the top failure modes into test objectives with a harmless goal, a measure, a sample size and a stop rule, tied to your pre-registered design method [ai.threat-model-redteam, agent.redblue-experiment]

### Recall (15 min)
Without notes: list the elements of a pre-registered design. What did your held-out attack family test? Name two ways a memory can be deleted incompletely.

### Theory (45 min)
- An AI system has a life cycle: data is collected and labelled, a model is trained or chosen and packaged, a service is built around it, and people and other systems use its outputs. Each stage has assets and attackers. The national taxonomy of attacks on machine learning organises them for predictive systems (evasion at inference time, poisoning at training time, privacy attacks that extract data or infer membership) and for generative systems (misuse, direct and indirect prompt injection, supply chain attacks, and the security of agents).
- The public knowledge base of adversary behaviour against AI systems (ATLAS) extends the vocabulary of the enterprise knowledge base with tactics and techniques seen against machine learning systems; the enterprise knowledge base itself has added techniques for attacks on AI features. Use both for naming and coverage, as in week 5.
- Ranking needs evidence: what an attacker needs (access, knowledge, effort), what they gain, what you have observed in your own tests. A ranking with no evidence is an opinion; say which parts are opinion.
- A test objective names the failure mode, the harmless goal that demonstrates it, the measure, how many trials, and when to stop. Without a stop rule an experiment keeps going until it finds something.

### Guided lab (120 min)
1. Choose the system under study: your week 10 agent plus a small classifier you will train on day 62 (for example a spam filter over synthetic messages). List every asset at each life-cycle stage.
2. For each asset write the threats that apply using the taxonomy's classes, and the controls that exist today.
3. Rank the failure modes with a stated scale, citing the evidence: your week 9 and 10 results, public reports you have read, what an attacker would need.
4. Write eight test objectives for the top failure modes in the pre-registration format of day 59, each with a harmless goal, a measure, a sample size and a stop rule.

### Independent lab (120 min)
Threat-model a system you did not build: a public description of a retrieval assistant with fine-tuning and tools, for example a vendor's reference architecture. In ninety minutes produce its asset list, ten threats, a ranking and three test objectives, marking every statement you could not verify as an assumption.

### Defence and verification (60 min)
Turn the model into something checked: for each of the top five failure modes write the control, the automated check (a test or a monitor) that would show it failing, and where in the pipeline it runs. Implement two. Remove a control and show the check fails. List the failure modes of your list that have no automated check and why.

### Evidence and report (60 min)
Hand in the asset and threat tables, the ranking with evidence marked as opinion where it is, the eight test objectives, the controls and checks, and the independent-lab model. It is accepted when each ranked threat cites evidence or says it is an opinion and each objective has a measure and a stop rule.

### Resources
- https://csrc.nist.gov/pubs/ai/100/2/e2025/final | External | NIST AI 100-2 E2025 (March 2025) | NIST publication, freely available; CHECKED 2026-11-14 by search result only, page not opened
- https://atlas.mitre.org/ | External | MITRE ATLAS knowledge base; release 2026.05 is reported, pin the release you read | MITRE terms of use; CHECKED 2026-11-14 by search result only, page not opened
- https://attack.mitre.org/ | External | ATT&CK, version 20 (October 2026) reported current; pin the release you read | MITRE terms of use; version 20 is reported to add techniques for attacks on AI features; CHECKED 2026-11-14 by search result only, page not opened

### Watch for
A threat list copied from a taxonomy without your system's assets. A ranking with no evidence column. A test objective with no stop rule.

## D62 — Adversarial fixtures, observed output and uncertainty

**Hand in.** Red-team fixture set — the fixtures for a classifier and for the agent with their harmless goals, the evaluation of observed outputs with a validated rubric, and the results with their uncertainty.

### Objectives
1. Build controlled adversarial fixtures — build fixtures that probe a trained classifier (evasion by small input changes) and the agent (harmless-goal inputs), each with a stated goal, budget and stop rule [ai.threat-model-redteam]
2. Evaluate observed output — grade the outputs with a written rubric, check the grading against labels you made by hand (and by yourself again a week later where you can), and say how reliable the grading is [ai.threat-model-redteam, llm.independent-eval]
3. Report uncertainty — report each result with the number of trials, an interval, the sensitivity to settings and the limits of the fixture set [ai.threat-model-redteam]

### Recall (15 min)
Without notes: name the life-cycle stages and one asset of each. Which of your test objectives would you run first and why? What is a stop rule for?

### Theory (45 min)
- Fixtures are the red team's reusable inputs, built to be harmless and repeatable. For a classifier, evasion means changing an input a little so that its output changes (words replaced, spacing, character swaps in a message) while a human reads it the same. For an agent the fixtures are the injection and memory cases of weeks 9 and 10.
- Grading generated output is a measurement problem. A rubric states what counts as success in words a second grader could apply. Another model can act as judge, but it is a model: measure its agreement with your hand labels, look for its biases (length, position, style) and never use it on the cases it was tuned on.
- Agreement is measured, not assumed: compare two graders (you and the judge, or you now and you later) with a chance-corrected statistic such as Cohen's kappa and read the disagreements.
- Uncertainty has several sources: the sampling of cases, the variation between runs, the sensitivity to prompt and settings, the reliability of the grader. Report the ones you measured and name the ones you did not.

### Guided lab (120 min)
1. Train a small spam classifier on a synthetic dataset. Write twenty evasion fixtures by editing messages with small changes a reader would ignore. Measure how often each changes the classifier's decision, over a set of messages with an interval.
2. Write the agent fixtures for three failure modes from yesterday's list (reveal a canary, call a tool with a marker, store a marker in memory). Run each twenty times.
3. Write the rubric for the agent fixtures. Grade eighty outputs by hand. Grade them with a judge model, and compute agreement and the disagreements.
4. Report each result with trials, interval, settings, and the grader's measured reliability.

### Independent lab (120 min)
Regrade forty of your outputs without looking at your earlier grades, and compute your agreement with yourself. Change the judge's prompt and the model's temperature and record how much each result moves. Decide which results are stable enough to report and which are not.

### Defence and verification (60 min)
Add defences for the classifier (normalising inputs, ensembling or adversarial training on the fixtures) and for the agent (the controls of weeks 9 and 10). Rerun the fixtures on a held-out set you did not use to design the defences. Report what improved, what did not, and what a better-resourced attacker could still do.

### Evidence and report (60 min)
Hand in the fixture sets, the rubric, the hand labels, the judge's agreement statistics, the results with intervals and settings, the sensitivity analysis, the defences and the held-out results. It is accepted when every result has its trials, interval and settings, and the grader's reliability is reported.

### Resources
- https://scikit-learn.org/stable/ | External | current documentation; pin the version you install | open source project, ASSUMED
- https://www.promptfoo.dev/docs/ | External | current documentation; pin the version you run | open source project, ASSUMED
- https://github.com/microsoft/PyRIT | External | current release; pin the version you install; the repository location may have changed | open source project, ASSUMED

### Watch for
Using the judge model on the cases it was tuned on. A fixture that a human would read differently. Reporting a rate from a single run.

## D63 — Model and dependency provenance

**Hand in.** Supply-chain review — the provenance record for each model and dependency of the lab, the integrity and licence assessment, a demonstration of a loading risk in the sandbox, and the acquisition controls with their tests.

### Objectives
1. Trace model/dependency provenance — for each model, dataset, package and container image in the lab record where it came from, who published it, which version, which hash and under which licence [ai.supply-chain]
2. Assess licensing/integrity — check integrity (hashes, signatures, file format risks) and licence terms (including use restrictions of open-weight models) for each, and record what you could not verify [ai.supply-chain]
3. Define acquisition controls — define and implement controls for bringing new models and packages into the lab: allow-listed sources, hash pinning, scanning, quarantine, licence review and an approval record [ai.supply-chain]

### Recall (15 min)
Without notes: write the interval method you use for rates and one case where it fails. Which of your fixtures is most sensitive to settings? State the rubric rule you were least sure of.

### Theory (45 min)
- A model is code you did not write, in the form of data. Where it comes from, who made it, whether it is the file you think it is and what you may do with it are supply-chain questions. So are the packages it needs, the container it runs in and the datasets behind it.
- Formats matter: some checkpoint formats are executed when loaded (a serialised object format can run arbitrary code), others hold only tensors and cannot. Prefer formats that cannot execute code, scan the rest, and load unknown files only in a sandbox with no network and no credentials.
- Integrity: compare the hash with the publisher's, prefer signed artefacts (signature and identity of the signer), pin by hash and not by name or tag, and watch for lookalike names (a package or a model with a one-letter difference).
- Licence: open weights are not necessarily open source. Models, datasets and code each carry terms that may restrict use, field, scale or redistribution. Record the terms and who approved the use.
- Records: a bill of materials for software and a counterpart for machine learning components lists what the system is made of; provenance attestations say how an artefact was built.

### Guided lab (120 min)
1. Inventory every model, dataset, package and image in the lab and fill the provenance table: name, source, publisher, version, hash, licence, date acquired, who approved.
2. For the model files check the format; scan with a tool of your choice; compare each hash with the publisher's. Mark what you could not verify.
3. In a disposable guest with no network, show with a harmless marker that loading a file in an executable checkpoint format runs code on load, and that a tensors-only format does not. Do not load files you do not control.
4. Read the licence of each model and dataset and write the permitted and forbidden uses for your lab.

### Independent lab (120 min)
Review a plausible dependency list for a machine-learning project (twenty packages and three models, written by a colleague or generated for the exercise) and find the problems: unpinned versions, a lookalike name, a model from an unverified account, an unrestricted licence assumption, an image with `latest`. Rank them and write the fix for each.

### Defence and verification (60 min)
Implement the acquisition gate: a script that, for a candidate model or package, requires an allow-listed source, a pinned hash, a scan result and a licence entry before copying it to the registry. Test it with a good candidate, a hash mismatch, a lookalike name and an unlicensed model, and show each is refused for the right reason. State what the gate does not know (a malicious but correctly hashed artefact).

### Evidence and report (60 min)
Hand in the provenance table, the integrity and licence findings with the unverified items marked, the sandbox demonstration, the review of the dependency list and the gate's test output. It is accepted when every artefact has a pinned hash and a recorded licence or is marked unapproved.

### Resources
- https://slsa.dev/ | External | supply-chain levels for software artefacts, current specification; note the version you read | open specification, ASSUMED
- https://huggingface.co/docs/safetensors | External | tensors-only format documentation; pin the version you install | open source project, ASSUMED
- https://pip.pypa.io/en/stable/topics/secure-installs/ | External | current pip documentation on hash-checking mode; pin the pip version | open source project, ASSUMED
- https://opensource.org/ai/open-source-ai-definition | External | the Open Source AI Definition, version to be confirmed on the page | open definition, ASSUMED

### Watch for
Trusting a model because its page looks official. Pinning a tag instead of a hash. Loading an unknown checkpoint on the workstation.

## D64 — Data poisoning, privacy exposure and governance

**Hand in.** Data security report — poisoning and privacy demonstrations on a small model in the lab, the data inventory with its lawful-use and retention decisions, and the governance controls with the checks that prove them.

### Objectives
1. Evaluate data poisoning — poison the training data of your small classifier in two ways (flipped labels, a trigger pattern) and measure the effect on accuracy and on triggered inputs, and test detection by data validation [ai.data-governance-eval]
2. Assess privacy exposure — show membership inference on an overfitted model, scan your corpus and logs for personal data, and list the places where data about a person persists [ai.data-governance-eval, llm.data-leakage]
3. Define governance controls — specify and implement controls for data collection, provenance, access, retention and deletion, with checks that fail when a control is missing [ai.data-governance-eval, ai.supply-chain]

### Recall (15 min)
Without notes: what makes a checkpoint format dangerous to load? Why pin by hash and not by tag? Name three terms in a model licence that could restrict your use.

### Theory (45 min)
- Poisoning corrupts what a model learns. Availability attacks degrade it (flip labels at random); targeted attacks teach it a hidden behaviour (a trigger pattern that forces an output) while leaving ordinary accuracy almost unchanged, which is why accuracy alone does not detect them. Defences are provenance (know who wrote the data), validation (outliers, label consistency, duplicate and near-duplicate checks), and tests with candidate triggers. Retrieval corpora are poisoned the same way.
- Privacy: models can reveal facts about their training data. An overfitted model is more confident on examples it saw than on ones it did not, which lets an observer infer membership. Generative models can reproduce memorised strings. Prompts, logs, caches, embeddings and evaluation sets hold personal data too.
- Governance is the set of decisions you can show: what data you may collect and why, from whom, under which terms, how long you keep it, who can see it, how a person's data is found and deleted, how a dataset's licence is honoured. It needs checks, not only a document.
- This is engineering guidance, not legal advice: where a law or an agreement applies, ask the person who owns that question.

### Guided lab (120 min)
1. On the synthetic classifier, flip a growing fraction of training labels and plot accuracy. Then add a trigger (a harmless token inserted into a few examples labelled as one class) and measure accuracy on clean data and the rate at which triggered inputs flip.
2. Write validation that would catch each: label-consistency checks, near-duplicate detection, a test that searches for tokens that flip decisions. Run it on the clean and poisoned sets.
3. Overfit a model on a small synthetic dataset and show a membership inference that uses the confidence gap, with its accuracy. Then regularise and show the gap shrink.
4. Scan your lab corpus, prompts and logs for personal data patterns (synthetic ones you planted) and list where each persists.

### Independent lab (120 min)
Write a data inventory for the whole lab: each dataset and store, its origin, licence, contents, who can access it, how long it is kept, how it is deleted, and whether personal data is present. Mark the rows you could not fill in. Choose the three riskiest rows and propose the control for each.

### Defence and verification (60 min)
Implement three governance checks as code: every dataset has a licence and an owner, nothing older than its retention period remains, and a person's identifier can be found and removed across the corpus, index and logs. Plant a violation of each and show the check fails; show a deletion reaching the index and the logs. State what the checks do not know.

### Evidence and report (60 min)
Hand in the poisoning curves and trigger results, the validation outcomes, the membership inference result before and after regularisation, the personal-data map, the inventory with unknowns marked, and the three checks. It is accepted when every poisoning claim has a measurement and every check has a planted violation that it catches.

### Resources
- https://csrc.nist.gov/pubs/ai/100/2/e2025/final | External | NIST AI 100-2 E2025 (March 2025), on poisoning and privacy attacks | NIST publication, freely available; CHECKED 2026-11-14 by search result only, page not opened
- https://www.nist.gov/itl/ai-risk-management-framework | External | AI Risk Management Framework, current version; note the version you read | NIST publication, freely available, ASSUMED

### Watch for
Judging poisoning by accuracy alone. A deletion that leaves the embeddings. A governance document with no check behind it.

## D65 — Metrics, benchmark leakage and regression criteria

**Hand in.** Evaluation protocol — the metrics and why they are valid for the claim, the leakage checks and their results, and the regression criteria with the gate that enforces them.

### Objectives
1. Define valid metrics — for a stated claim choose metrics that measure it, say what each does not measure, and fix scoring rules, sample sizes and the analysis before running [ai.data-governance-eval, llm.independent-eval]
2. Detect benchmark leakage — test whether evaluation items appear in training, tuning or retrieval data (exact and near matches, canaries) and measure the effect on the score [llm.eval-leakage]
3. Create regression criteria — define thresholds that use intervals, a held-out set that stays held out, and a gate that blocks a change when a measure worsens beyond its interval [llm.eval-leakage, ai.data-governance-eval]

### Recall (15 min)
Without notes: name two checks that would catch a poisoned dataset and one thing neither can see. How does an overfitted model reveal membership? Where in the lab does personal data persist after a deletion?

### Theory (45 min)
- A metric is valid for a claim if its value would change when the claim stops being true and would not change otherwise. Accuracy of a classifier says little about how often it is fooled; the share of fixtures that succeed says nothing about how often real traffic contains them. Write the claim first ("the assistant does not disclose documents the requester may not read"), then the measure, then what the measure cannot show.
- Leakage means evaluation data influenced the system under test: items present in a training or tuning set, in a retrieval corpus, in a system prompt's examples, or in the developer's memory. It inflates scores. Check by exact and near-duplicate search, by planting canary strings in held-out items, and by looking for impossible performance.
- Goodhart's law applies: a measure that becomes a target stops measuring. Keep a held-out set that you run rarely, rotate cases, and report the measures you did not optimise.
- A regression criterion compares a new result with a reference using intervals, so that noise does not block changes and real losses do.

### Guided lab (120 min)
1. Write three claims for your application and classifier. For each choose a metric, a scoring rule, the sample size and the analysis, and write what the metric does not show.
2. Test for leakage: search your fine-tuning, retrieval and prompt example sets for exact and near matches of your evaluation items; plant canary strings in held-out items and see whether any appears in outputs or in the corpus.
3. Deliberately leak ten evaluation items into a training or retrieval set and measure how much the score rises. Record the inflation.
4. Write the regression criteria: thresholds on the interval, the rotation of the held-out set, and the stored reference results.

### Independent lab (120 min)
Take a public benchmark description and design a leakage test for it that you could run against a model or corpus you control. List what the test would show if it passed and if it failed, and what it could not show. Draft the paragraph of a report that states the limits honestly.

### Defence and verification (60 min)
Implement the gate: a command that runs the evaluation, compares with the stored reference using intervals, checks that no held-out item appears in any corpus, and fails the change otherwise. Show it blocks a regression, a leaked item and an unrecorded change of settings. Record the cost in time and what it cannot guard (a metric that stopped meaning anything).

### Evidence and report (60 min)
Hand in the claims and metrics with their limits, the leakage test results and the inflation measurement, the regression criteria and the gate's output on a clean change, a regression and a leak. It is accepted when each metric states what it does not measure and the gate is shown to block a leak.

### Resources
- https://github.com/EleutherAI/lm-evaluation-harness | External | current release; pin the version you run | open source project, ASSUMED
- https://www.promptfoo.dev/docs/ | External | current documentation; pin the version you run | open source project, ASSUMED

### Watch for
A metric chosen because it is easy to compute. Held-out data used to pick between two designs. A score reported without the settings and the version.

## D66 — Week 11 assessment (blind practical)

**Hand in.** Week 11 assessment — the report of the blind practical: the threat model of an unfamiliar AI system, the bounded tests with their results and uncertainty, the supply-chain and data findings, and the controls with the evidence and limits.

### Objectives
1. Assess unfamiliar AI scenario — build a threat model of an unseen AI system from its description and artefacts and run the bounded tests it allows [ai.threat-model-redteam]
2. Produce evidence-linked findings — link every finding to an artefact, a test or a measurement, with uncertainty, and separate what you measured from what you assume [ai.data-governance-eval]
3. Verify control/limits — recommend and verify controls for the supply chain and data findings, and state the limits of every conclusion [ai.supply-chain]

### Blind practical (120 min)
Open the assessment in the application, start the attempt and read the task statement. Take the scenario from the source you fixed before the week and do not read its notes or its list of problems before the clock starts. Stop at 120 minutes.

### Remediation (60 min)
Grade yourself against the rubric before reading any answer key. For every criterion below your target find the cause in your method (the asset you did not list, the test with no stop rule, the licence you did not read) and repair it with a short exercise. Then redo the failed part on a different scenario.

### Report (60 min)
Write the report in the structure of the task statement: assets and threats, tests and results with uncertainty, supply-chain findings, data findings, controls, limits. Attach the evidence with SHA-256 values.

### Performance review (60 min)
Compare the grade with the week's targets: independent execution, every finding linked to evidence, results reported with uncertainty, supply chain and data covered, honest limits. Record the three habits that cost you most and one change for the capstone.

### Assessment
- Code: W11-BLIND
- Title: Week 11 blind practical: threat model, bounded tests and supply-chain review of an unfamiliar AI system
- Kind: BLIND_PRACTICAL
- Competency: ai.threat-model-redteam
- Difficulty: 4

Task statement:
You have 120 minutes. You are given an AI system you have not seen: an architecture description, a dependency and model list with hashes and licences, a data description, sample logs, results of an earlier evaluation, and a running lab instance with a local model and synthetic data. Work only inside the lab and on the system named in your scope statement. Every adversarial goal must be harmless.

Scenario source (fixed before the week, not inspected): a scenario prepared by another person from the catalogue below, or, working alone, a variant of your own system with a scenario pack produced by a script you wrote earlier but have not read since. Problems are drawn from: a model whose file format executes on load or whose hash does not match; a lookalike or unpinned dependency; a licence that forbids the stated use; training or retrieval data that contains the evaluation items; a poisoned subset; personal data persisting in logs or an index after a deletion; an evaluation whose metric does not support its claim; an agent or application failure of the kind of weeks 9 and 10.

Deliverables, in this order. First, the assets by life-cycle stage and the threats ranked with evidence, opinion marked as opinion. Second, three test objectives in pre-registered form and the results of running them within the bounds, with trials, intervals and settings. Third, the supply-chain findings (provenance, integrity, licence) with what you could not verify. Fourth, the data findings (poisoning, privacy, retention, leakage into evaluation). Fifth, for each finding the control and the check that would show it failing. Sixth, the limits of every conclusion. Seventh, a report of at most five pages with the evidence hashes.

You are assessed on: independent execution during the block; every finding linked to an artefact, a test or a measurement; results with their uncertainty; supply chain and data both covered; honest limits. The pass rule is non-compensatory: every criterion must reach the pass level.

### Resources
- https://csrc.nist.gov/pubs/ai/100/2/e2025/final | External | NIST AI 100-2 E2025 (March 2025) | NIST publication, freely available; CHECKED 2026-11-14 by search result only, page not opened

### Watch for
Running a test with no stop rule. A finding with no artefact behind it. Treating a signed artefact as a safe one.
