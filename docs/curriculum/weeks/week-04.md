# Week 4 — Advanced web and API security

**Purpose.** Move from single-page defects to the failures that live in how a system is put together: identity carried by tokens across services, servers that fetch what users tell them to fetch, files and objects that arrive from outside, business rules that only hold when requests arrive one at a time, and the habit of writing down what could go wrong before it does. The week ends with a threat model of your own application, built from evidence you collected, and with the blind API assessment.

**Lab stack.** The week 3 lab. A small REST API of your own (FastAPI with a JWT library and SQLite) with two tenants and three roles; an internal-only service on a second subnet that plays the part of an administrative or metadata endpoint; the lab DNS server of week 2; `pytest` and `httpx`; the OWASP crAPI running locally from its containers for independent practice. All traffic stays on the isolated lab network, with synthetic data.

**Competencies.** `api.token-authz`, `web.ssrf-upload-deserialization`, `web.threat-model-logic`, with `web.authz-idor`, `web.injection`, `net.routing-segmentation` and `python.testing-fixtures` revisited.

**Accelerator.** A penetration tester can build the API in half the time and use the saved hours on the independent labs; the threat model of day 23 and the Saturday assessment are taken unchanged.

## D19 — API identity, tokens and object-level authorization

**Hand in.** API security matrix — every route by authentication, token validation, object-level, property-level and function-level checks, with the test that proves each cell.

### Objectives
1. Map API token/identity flow — draw how an API authenticates callers (API key, session cookie, OAuth 2.0 access token, signed token), what each credential asserts, who validates it and how it is revoked [api.token-authz]
2. Check object-level authorization — test every object-bearing route of a small REST API with two tenants and several roles and record which checks are missing at object, property and function level [api.token-authz, web.authz-idor]
3. Define API tests — write an automated suite for authentication failure modes (no token, expired, wrong audience, wrong signing algorithm) and for authorization (cross-tenant access, mass assignment, excessive data in responses) [api.token-authz]

### Recall (15 min)
Without notes: list the classes of defect you fixed last week and name the test that guards each. What does `HttpOnly` protect and what does it not? Name the three cookie attributes.

### Theory (45 min)
- An API key identifies an application, not a person. An OAuth 2.0 access token is a bearer credential: whoever holds it can use it, so audience, scope, lifetime and storage matter. A signed token (JWT) is verified by checking its signature, the algorithm you expect (never the one the token names), issuer, audience and expiry. Read RFC 6749, RFC 6750, RFC 7519 and the best-practice RFC 8725.
- The API security list names the recurring failures: object-level authorization, broken authentication, object-property-level authorization (which covers both returning too much and accepting writes to fields the caller must not set, called mass assignment) and function-level authorization.
- Authorization lives on the server per request, per object, per property. Schemas (an OpenAPI description, response models) make the allowed shape explicit and testable.

### Guided lab (120 min)
1. Build the lab API with `POST /login` returning a signed token, `GET /accounts/ID`, `PATCH /accounts/ID` and an administrative route. Two tenants, users with roles.
2. Write the matrix first, then the tests: no token gives 401; expired, wrong audience, wrong signature and a token whose header asks for no signature are all refused; a user of tenant A reading tenant B's account is refused; a `PATCH` that includes a `role` field does not change the role; the response never contains the password hash.
3. Run the tests against a deliberately weak first version (it should fail several), then fix one defect per commit.
4. Fill in the observed column of the matrix from the test results.

### Independent lab (120 min)
Run the OWASP crAPI in its containers on the lab network. Choose three of its challenges at random from the list without reading the titles first. For each, record the request that shows the flaw, the class from the API security list, and the check that was missing. Write the test you would add to its code base.

### Defence and verification (60 min)
Make the response shape explicit: serializers with an allow-list of fields per role, input models that reject unknown fields, and a per-token rate limit. Prove with tests that a client cannot read a field it should not see, cannot write a field it should not set, and is throttled after a burst. Show that hiding a field in the client would not have passed the same tests.

### Evidence and report (60 min)
Hand in the matrix with the name of the test for every cell, the three crAPI notes, and the diff that introduced the allow-lists. It is accepted when every route appears in the matrix and every cell is backed by a test that fails on the weak version.

### Resources
- https://www.rfc-editor.org/rfc/rfc6749 | External | RFC 6749 (October 2012) | IETF document, freely available
- https://www.rfc-editor.org/rfc/rfc7519 | External | RFC 7519 (May 2015) | IETF document, freely available
- https://www.rfc-editor.org/rfc/rfc8725 | External | RFC 8725 (February 2020) | IETF document, freely available
- https://owasp.org/API-Security/editions/2023/en/0x00-header/ | External | OWASP API Security Top 10, 2023 edition | OWASP project, open licence stated by the project, ASSUMED
- https://github.com/OWASP/crAPI | External | current release; pin the tag or image digest you run | open source project, ASSUMED

### Watch for
Accepting the algorithm named in the token. Checking the tenant on read and not on update. Filtering a field in the client and calling it protected.

## D20 — Server-side request forgery

**Hand in.** SSRF threat model — the diagram of what a fetching feature can reach, the demonstration against your internal service, and the layered restrictions with the tests that refuse each bypass class.

### Objectives
1. Explain SSRF trust boundary — explain how a server that fetches a user-supplied address can be made to reach internal services and metadata endpoints, and which network and application trust assumptions this breaks [web.ssrf-upload-deserialization]
2. Analyze isolated SSRF scenario — add a preview-by-URL feature to the lab application, show it reaching an internal-only service you host, and map what a caller learns at each step [web.ssrf-upload-deserialization]
3. Verify destination restrictions — implement and test allow-listing after name resolution, redirect handling, scheme and port limits and network egress rules, and show the bypass classes you tried are refused [web.ssrf-upload-deserialization, net.routing-segmentation]

### Recall (15 min)
Without notes: what is the difference between a token's audience and its issuer? Why is a client-side filter not a control? List the four checks of a validated signed token.

### Theory (45 min)
- A server that makes requests on behalf of a user acts with the server's network position, which usually sees more than the user does: internal admin pages, databases, and on cloud hosts a metadata service on a link-local address. SSRF is the abuse of that position.
- Validation by string is weak. Addresses have many spellings, hostnames resolve to different addresses at different times (a hostname can resolve to a public address when checked and to an internal one when used), redirects change the destination after the check, and IPv6 and mapped addresses reach the same machines.
- Layered defence: an allow-list of destinations, resolve once and connect to the checked address, no redirects or revalidation on each, only `http` and `https` on stated ports, refusal of loopback, private, link-local and unique-local ranges after resolution, the response not returned raw, and egress rules so the web host cannot reach internal services it does not need.

### Guided lab (120 min)
1. Add `/preview?url=` to the lab application using a plain HTTP client. Put an internal service on the other subnet that returns a recognisable banner and is reachable only from the web host.
2. Show the application fetching the internal banner for the caller. Map what the caller can learn: whether a port is open, how long it took, what the response says.
3. Write a validator that resolves the name, checks the address against the refused ranges and connects to that address. Write tests for alternate spellings of the loopback and private addresses (decimal, octal, short forms, IPv6 and mapped forms), for a redirect to an internal address, and for a name that you make resolve differently on a second lookup in your lab DNS.
4. Run the tests against the naive and the fixed validator and keep the failing output of the first.

### Independent lab (120 min)
Choose three server-side request forgery labs at random from the Web Security Academy list. For each, record the sink, the check that existed and why it failed, and the one change that would have stopped it. Then list every feature of your own application that makes an outbound request and rate the destination control of each.

### Defence and verification (60 min)
Add the network layer: an egress rule on the web guest that denies private destinations except an explicit list. Prove that an SSRF attempt which gets past your validator on the naive branch is still stopped at the network, and that the application's legitimate outbound call still works. State what the layers together still do not stop.

### Evidence and report (60 min)
Hand in the diagram (the application, the internal service, the allowed and refused flows), the demonstration, the test list by bypass class with failing and passing output, and the egress rule with its test. It is accepted when every bypass class has a test and the network rule is shown to work independently of the validator.

### Resources
- https://owasp.org/Top10/ | External | OWASP Top 10, 2025 edition exists (SSRF is inside A01 Broken Access Control); read the current edition | OWASP project, open licence stated by the project; CHECKED 2026-11-14 by search result only, page not opened
- https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html | External | current cheat sheet; note the revision date you read | OWASP project, open licence stated by the project, ASSUMED
- https://portswigger.net/web-security/ssrf | External | Web Security Academy, content as of the day you use it | free account required; terms to be checked, ASSUMED

### Watch for
Validating the hostname and then letting the library resolve it again. Blocking `127.0.0.1` and forgetting `::1`. A redirect that moves the request after your check.

## D21 — Uploads and unsafe deserialization

**Hand in.** Processing security review — the upload and import features reviewed, the demonstrations in the sandbox, the safe replacements and the hostile-fixture tests.

### Objectives
1. Evaluate upload validation — test an upload feature for trust in the declared type and the file name, path traversal in names, oversized and compressed-bomb inputs and the risk of stored files being executed, and decide what to allow [web.ssrf-upload-deserialization]
2. Explain unsafe deserialization — explain why reading untrusted data with a format that can create arbitrary objects lets the data choose the code that runs, and show a harmless proof in a sandboxed guest [web.ssrf-upload-deserialization]
3. Build input-processing tests — write tests with hostile fixtures for the upload and the importer that fail on the unsafe version and pass on the safe one [web.ssrf-upload-deserialization, python.testing-fixtures]

### Recall (15 min)
Without notes: list three bypass classes for an SSRF filter and say which layer stops each. Why does connecting to the checked address matter? What did your egress rule block?

### Theory (45 min)
- Uploads: the browser's declared type and the file name are attacker-controlled. Decide by content (the leading bytes), allow only the types you need, generate the stored name yourself, store outside the web root on a mount that cannot execute, set a size limit while streaming, re-encode images, and serve user content with a download disposition and `nosniff`. Archives add path traversal in member names and expansion bombs.
- Deserialization: some formats can rebuild arbitrary objects and call code while doing so (Python `pickle`, Java native serialization, YAML tags that construct objects). Untrusted input must use a data-only format (JSON with a schema). A signature does not make an unsafe format safe if the key can leak.
- Read the cheat sheets on file upload and on deserialization; read the page of your language's documentation that warns about the unsafe module.

### Guided lab (120 min)
1. Add an avatar upload that trusts the declared type and uses the supplied file name, and a settings import that reads a serialized object.
2. In the sandbox guest show the file name traversal writing a harmless marker file outside the upload directory, and a file with a lying type accepted. Do not upload anything that runs.
3. For the importer, show with a harmless marker (an object that makes the process create a file named `marker`) that data selected the code. Then replace it with JSON and a schema.
4. Fix the upload: content check, allow-list, generated name, size limit while streaming.

### Independent lab (120 min)
Choose three labs at random from the Web Security Academy's file upload and insecure deserialization topics. For each, write the sink, the missing control and the fix. Then write the hostile fixture set for your own features: a file with a double extension, a path in the name, a huge file, a nested archive and a deeply nested document.

### Defence and verification (60 min)
Contain the damage: mount the upload directory with `noexec,nosuid,nodev`, run the service under a systemd unit that limits where it can write, and show a script placed in the upload directory cannot be executed through the service. Add the fixtures as tests that fail on the naive code and pass on yours.

### Evidence and report (60 min)
Hand in the review of each feature (what is accepted, what is refused and why), the sandbox demonstrations, the fixtures with failing and passing output and the containment evidence. It is accepted when every refused input has a test and the containment is shown with a command and its output.

### Resources
- https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html | External | current cheat sheet; note the revision date you read | OWASP project, open licence stated by the project, ASSUMED
- https://cheatsheetseries.owasp.org/cheatsheets/Deserialization_Cheat_Sheet.html | External | current cheat sheet; note the revision date you read | OWASP project, open licence stated by the project, ASSUMED
- https://docs.python.org/3/library/pickle.html | External | Python 3.11 or newer documentation; use the version you run | open documentation, ASSUMED

### Watch for
Trusting the extension, the declared type or both. A "safe" loader that still allows object tags. Testing the validator with one file instead of a set that tries to break it.

## D22 — Business logic and concurrency

**Hand in.** Business-logic report — the invariants of the workflow, the failures found, the concurrency demonstration and the fix with a repeatable regression test.

### Objectives
1. Assess business logic invariants — list the invariants of a small shop workflow (a balance never below zero, a coupon used once, stock never negative, only legal state transitions) and find where the code relies on the client or on call order to keep them [web.threat-model-logic]
2. Examine controlled concurrency issue — reproduce a race (double redemption of a coupon, overdrawing a balance) with parallel requests against your application and show the interleaving that breaks the invariant [web.threat-model-logic]
3. Create race-condition regression — fix it with a database constraint, an atomic update or a transaction at a suitable isolation level, and add a test that runs the parallel scenario repeatedly and fails on the unsafe version [web.threat-model-logic, python.testing-fixtures]

### Recall (15 min)
Without notes: why is a content type not evidence of what a file is? Name the two mount options that stop execution and the setuid effect. What does the safe replacement for a pickle import look like?

### Theory (45 min)
- A business rule is an invariant, a statement that must hold after every operation. Validation at the edge is not enforcement: the rule must be enforced where the state lives, by constraints and atomic operations, because requests interleave.
- Check-then-act is the classic race: read the balance, test it, write the new one. Two requests can both read before either writes. Fixes: a single atomic statement (`UPDATE ... SET balance = balance - x WHERE balance >= x`), a uniqueness constraint, row locking, a version column checked on update, or serializable transactions with a retry.
- Logic abuse also includes tampered prices and quantities (negative values), skipped steps in a workflow, and replaying a request; an idempotency key defends the last.

### Guided lab (120 min)
1. Add a wallet, a coupon that may be redeemed once, and a checkout. Write the invariants first as a list with a query that checks each.
2. Plant check-then-act in coupon redemption. Fire twenty parallel redemptions from a script and show more than one succeeds; show the log of the interleaving.
3. Fix it three ways on three commits (unique constraint, atomic update, transaction with retry) and compare the behaviour under the same load.
4. Add tests for a negative quantity, a price sent by the client, a skipped payment step and a replayed order.

### Independent lab (120 min)
Choose three labs at random from the Web Security Academy's business logic and race condition topics. For each, state the invariant that was violated and where it should have been enforced. Then review your own application for any rule that depends on the order in which the client calls endpoints.

### Defence and verification (60 min)
Write an invariant monitor that runs the queries from step 1 against the database and alerts on any violation. Run it after hammering the unsafe version (it must flag the damage) and the safe version (it must be silent). Add a detection for the attack pattern itself, many near-simultaneous redemptions of one coupon from one session, and say what it cannot see.

### Evidence and report (60 min)
Hand in the invariants with their checking queries, the interleaving evidence, the three fixes with their results under load, and the monitor output on both versions. It is accepted when each invariant has a query and the race test fails on the unsafe version more often than not.

### Resources
- https://portswigger.net/web-security/race-conditions | External | Web Security Academy, content as of the day you use it | free account required; terms to be checked, ASSUMED
- https://www.postgresql.org/docs/current/transaction-iso.html | External | current PostgreSQL documentation; note the major version you read | open documentation, ASSUMED
- https://www.sqlite.org/lang_transaction.html | External | current SQLite documentation; note the version you run | public domain documentation, ASSUMED

### Watch for
A test that sends requests one after another and calls it concurrency. Fixing the symptom with a lock that does not span processes. A constraint added in the application but not in the database.

## D23 — Application threat model

**Hand in.** Application threat model — the data-flow diagram, the ranked threats with evidence, and the top five mitigations with the tests that prove them and the risk that remains.

### Objectives
1. Draft application data-flow model — draw the data-flow diagram of the lab application with trust boundaries, assets, entry points and data stores, derived from the code and the running system rather than from memory [web.threat-model-logic]
2. Rank vulnerabilities — enumerate threats per element, score each by likelihood and impact on a stated scale, and justify the ordering with evidence from weeks 3 and 4 [web.threat-model-logic]
3. Verify mitigations — for the five highest threats state the mitigation, the test that shows it works and the residual risk, and run the tests [web.threat-model-logic, web.injection]

### Recall (15 min)
Without notes: list the invariants of your wallet workflow and the statement that enforces each. Name three fixes for check-then-act. Which of them survives two application processes?

### Theory (45 min)
- Threat modelling answers four questions: what are we building, what can go wrong, what will we do about it, and did we do a good enough job. The artefact is a diagram, a list of threats and a list of actions with owners, and it is updated when the system changes.
- A data-flow diagram has external entities, processes, data stores, flows and trust boundaries; threats are found where a flow crosses a boundary. STRIDE (spoofing, tampering, repudiation, information disclosure, denial of service, elevation of privilege) is a checklist for each element.
- Rank with a stated scale for likelihood and for impact and write the reasoning; a score with no evidence is an opinion. Tie each threat to a requirement and to a test, so the model is checked by the build.
- Read the OWASP threat modelling cheat sheet and the application security verification standard's structure.

### Guided lab (120 min)
1. Derive the diagram from the system: list every route, every outbound call and every data store from the code and from `ss`, and place the trust boundaries (browser, web host, internal service, database).
2. For each element and boundary crossing, write the STRIDE threats that apply. Strike the ones the system already stops, citing the control and test.
3. Score the rest with your stated scale and rank them. For the top five write mitigation, test and residual risk.
4. Keep the model in git as text (a diagram in a text format and a table).

### Independent lab (120 min)
Build a threat model of a different application, the OWASP Juice Shop or crAPI, in ninety minutes using only its running behaviour and documentation. Compare its top five with the list of its known challenges afterwards and score your recall: what did you find, what did you miss, and which element of your method would have found it.

### Defence and verification (60 min)
Turn the top five mitigations into automated checks in your repository (a `make security` target running `pytest` tests and the header audit of day 13). Run it. Remove one mitigation and show the target fails. State which threats in the model remain without an automated check and why.

### Evidence and report (60 min)
Hand in the threat model: diagram, threat table with scores and reasoning, the five mitigations with their tests, residual risks, the independent-lab comparison and the check target's output with and without the removed mitigation. It is accepted when every ranked threat cites evidence and every top-five mitigation has a test that fails without it.

### Resources
- https://cheatsheetseries.owasp.org/cheatsheets/Threat_Modeling_Cheat_Sheet.html | External | current cheat sheet; note the revision date you read | OWASP project, open licence stated by the project, ASSUMED
- https://owasp.org/www-project-threat-dragon/ | External | current release; pin the version you install | open source project, ASSUMED
- https://owasp.org/www-project-application-security-verification-standard/ | External | ASVS 5.0.0 (30 May 2025); a 5.0.1 patch release is planned | OWASP project, open licence stated by the project; the standard is also published at asvs.owasp.org (reported); CHECKED 2026-11-14 by search result only, page not opened

### Watch for
A diagram drawn from memory. Scores with no reasoning. A model nobody updates after the next change.

## D24 — Week 4 assessment (blind practical)

**Hand in.** Week 4 assessment — the report of the blind practical: the API defects found with proof, their root causes, the fixes with tests and the evidence that each control works.

### Objectives
1. Investigate blind API scenario — review an unfamiliar API for the week's classes of defect and report each with a safe reproduction [api.token-authz, web.ssrf-upload-deserialization]
2. Explain root cause — for each finding name the missing check, the trusted input or the broken invariant rather than the symptom [web.authz-idor, web.threat-model-logic]
3. Verify and report control effectiveness — fix each defect, prove the fix with a test that failed before it, and say what the controls still do not cover [api.token-authz]

### Blind practical (120 min)
Open the assessment in the application, start the attempt and read the task statement. Take the API from the source you fixed before the week and do not read its code, its description of known issues or its tests before the clock starts. Stop at 120 minutes.

### Remediation (60 min)
Grade yourself against the rubric before reading any answer key. For every criterion below your target find the cause in your method (the role you did not test, the field you did not try to set, the feature that fetches or imports) and close the gap with a short exercise on your own lab API. Then redo the failed part on a different instance.

### Report (60 min)
Write the report in the structure of the task statement: findings by impact with reproduction, root cause, fix, regression test and verification; residual risk; what you did not test. Attach the evidence with SHA-256 values.

### Performance review (60 min)
Compare the grade with the week's targets: independent execution, two tenants and three roles used for every access test, root cause rather than symptom, tests that fail before the fix, honest statement of residual risk. Record the three habits that cost you most and one change for next week.

### Assessment
- Code: W04-BLIND
- Title: Week 4 blind practical: review and repair an unfamiliar API
- Kind: BLIND_PRACTICAL
- Competency: api.token-authz
- Difficulty: 4

Task statement:
You have 120 minutes. You are given an API you have not read, with documentation, two tenants and users of several roles, running on your isolated lab network with synthetic data, and a statement of what it is for. Work only inside the lab and on the API named in your scope statement.

Scenario source (fixed before the week, not inspected): an API prepared by another person with defects planted from the catalogue below, or, working alone, an unseen challenge set of a deliberately vulnerable API project, or a variant of your own lab API produced by a script you wrote earlier but have not read since. Defects are drawn from: a token accepted with the wrong audience, algorithm or no signature; an object returned or modified across tenants; a function reachable by a lower role; a writable field that grants privilege; a response that exposes fields the caller must not see; a feature that fetches a caller-supplied address; an upload or import that trusts its input; a limited resource that can be spent twice by parallel requests; no rate limit on an expensive or sensitive route.

Deliverables, in this order. First, a threat sketch of the API drawn from its running behaviour: entry points, trust boundaries, assets. Second, findings, each with a safe reproduction. Third, for each finding the root cause and the fix. Fourth, regression tests that fail before the fix and pass after. Fifth, the residual risk and what you did not test. Sixth, a report of at most four pages with the evidence hashes.

You are assessed on: independent execution during the block; tenants and roles used for every access test; root cause stated rather than symptom; tests that fail before the fix; honest residual risk and limits. The pass rule is non-compensatory: every criterion must reach the pass level.

### Resources
- https://owasp.org/API-Security/editions/2023/en/0x00-header/ | External | OWASP API Security Top 10, 2023 edition | OWASP project, open licence stated by the project, ASSUMED

### Watch for
Testing as one tenant only. A finding with a reproduction that harms real data. Stopping after the first class of defect.
