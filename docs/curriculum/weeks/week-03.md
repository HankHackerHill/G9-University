# Week 3 — Web application fundamentals

**Purpose.** Understand how a web application decides who you are and what you may do, and how that decision fails. You build a small application of your own, find its defects the way a tester would, fix them the way an engineer would, and prove each fix with a test that failed before it. The same four ideas carry through the week: the browser is a hostile environment for secrets, identity is state, every object needs its own authorization check, and data must never be allowed to become code.

**Lab stack.** The week 1 lab plus a web guest. A small Python web application of your own (Flask or FastAPI with SQLite), written by you on day 13 and kept in git with one branch per planted defect; a browser with developer tools; `curl`; `mitmproxy` or another intercepting proxy; `pytest`. For independent practice a free account on the PortSwigger Web Security Academy and the OWASP Juice Shop run locally in a container. All of it stays on the isolated lab network, with synthetic data.

**Competencies.** `web.http-semantics`, `web.session-auth`, `web.authz-idor`, `web.injection`, `web.xss-contexts` (pack `web-api-appsec`).

**Accelerator.** An application security engineer can build the lab application in half the time and spend the saved time on the independent labs; the Saturday assessment is taken unchanged.

## D13 — HTTP semantics and browser trust boundaries

**Hand in.** HTTP analysis — requests and responses of your own server annotated with what each header allows or blocks in a browser, the origin and trust-boundary diagram, and the header audit output.

### Objectives
1. Explain HTTP request semantics — for given requests predict method safety and idempotency, status code, caching and redirect behaviour, and verify each prediction against your own server with `curl -v` [web.http-semantics]
2. Analyze cookies, origins and headers — read the cookie attributes, CORS headers and security headers of a response and state exactly what each allows or blocks in a browser, with a test for each [web.http-semantics]
3. Map browser trust boundaries — draw the origins, frames, cookies and storage of a small multi-origin application and mark where the same-origin policy, CORS and a content security policy apply [web.http-semantics]

### Recall (15 min)
Without notes: what are the two things a routing table lookup decides? Why is default allow with exceptions untestable as segmentation? Write the three parts of the scope statement you would need before scanning a web server.

### Theory (45 min)
- A request has a method, a target, headers and an optional body; a response has a status, headers and a body. Safe methods do not change state, idempotent ones can be repeated with the same effect, and a server that changes state on GET breaks both promises. Read the sections of RFC 9110 on methods and status codes.
- An origin is scheme, host and port. The same-origin policy stops a page from reading another origin's responses; CORS is the server's way of relaxing that for named origins, and a wildcard origin with credentials is refused by browsers. A content security policy restricts what a page may load or run.
- A cookie is sent to every request that matches its domain and path unless attributes restrict it: `Secure` (HTTPS only), `HttpOnly` (not readable by page script), `SameSite` (cross-site sending). Read RFC 6265 and the cookie chapter of the OWASP testing guide.

### Guided lab (120 min)
1. Write the lab application's first version: a login page, a notes list, a note page and an API route. Run it on two hostnames under `lab.test` (for example `a.lab.test` and `b.lab.test`).
2. With `curl -v` look at methods, status codes, caching headers and redirects. For five requests write the prediction first, then compare.
3. Set cookies with different attributes and show in the browser's developer tools which requests carry them. Test `SameSite` by loading a page from `b.lab.test` that submits a form to `a.lab.test`.
4. Make a page on `b.lab.test` read a response of `a.lab.test` with `fetch`: show the block, add CORS headers, show the preflight request for a non-simple request, then show that credentials need an explicit origin.
5. Add a content security policy that blocks inline script and show the console report.

### Independent lab (120 min)
Prepare six response header sets (cookies, CORS, CSP, framing) and six browser scenarios that each ask "will this work?". Write your predictions, then run all thirty-six combinations or a random dozen, and score your predictions. For every wrong one, find which rule you mis-remembered.

### Defence and verification (60 min)
Write a header audit in Python that fetches a list of URLs and reports cookies missing `Secure`, `HttpOnly` or `SameSite`, missing or weak CSP, framing allowed without a decision, a CORS wildcard with credentials, and missing `X-Content-Type-Options`. Run it on your application before and after you fix each finding and show it fails on the weak configuration and passes on the fixed one.

### Evidence and report (60 min)
Hand in the annotated requests and responses with a predicted-versus-observed table, the origin and trust-boundary diagram, and the audit output before and after. It is accepted when each header on the diagram has a test that shows its effect in a browser.

### Resources
- https://www.rfc-editor.org/rfc/rfc9110 | External | RFC 9110 (June 2022) | IETF document, freely available
- https://www.rfc-editor.org/rfc/rfc6265 | External | RFC 6265 (April 2011) | IETF document, freely available
- https://owasp.org/www-project-web-security-testing-guide/ | External | Web Security Testing Guide, stable release to be confirmed when you read it | OWASP project, open licence stated by the project, ASSUMED
- https://developer.mozilla.org/en-US/docs/Web/HTTP | External | current MDN web documentation | open content licence stated by MDN, ASSUMED

### Watch for
Treating CORS as an access control for the server: it only changes what a browser lets a page read. Testing cookies over HTTP and wondering why `Secure` blocks them. Confusing site and origin.

## D14 — Authentication and session state

**Hand in.** Session review — the session lifecycle model, the defects found with evidence, and the mitigations with the tests that prove them.

### Objectives
1. Model authentication/session state — draw the lifecycle of a session from login through use to logout and expiry, naming where each piece of state lives and what every transition must check [web.session-auth]
2. Analyze controlled session defects — find the planted defects in your own application (a predictable session identifier, no rotation at login, logout that does not invalidate, a session identifier in a URL, a weak password reset) and show evidence for each [web.session-auth]
3. Test session mitigations — implement and prove identifier rotation, secure cookie attributes, idle and absolute timeouts, server-side invalidation and rate limiting of login attempts [web.session-auth]

### Recall (15 min)
Without notes: which of `Secure`, `HttpOnly` and `SameSite` stops script from reading a cookie? What is an origin? Predict what a browser does with a response that sets `Access-Control-Allow-Origin: *` and `Access-Control-Allow-Credentials: true`.

### Theory (45 min)
- Authentication proves who is asking; a session is the server's memory of that proof, carried by an identifier the client presents on each request. The identifier must be unguessable, bound to the server-side record, rotated when privilege changes (login, step-up) and destroyed on logout and expiry.
- Common defects: fixation (the attacker chooses the identifier and the server keeps it after login), identifiers in URLs, no server-side invalidation, long-lived tokens, passwords stored with a fast hash. Passwords are stored with a slow, salted, memory-hard scheme (Argon2id, scrypt or bcrypt), never with a general-purpose hash.
- Login must resist guessing without letting an attacker lock out a victim: throttle per account and per source, with delays that grow.
- Read the OWASP cheat sheets on session management, authentication and password storage.

### Guided lab (120 min)
1. Add login to the lab application with three planted defects on a branch: sequential session identifiers, no rotation at login, logout that only clears the cookie.
2. Show each defect with evidence: predict the next identifier from two observed ones; set an identifier before login and show it is still valid afterwards (fixation); replay a logged-out cookie and show it still works.
3. Fix each on its own commit and add a test per defect that fails before the fix.
4. Store passwords with a slow salted scheme and show with a timing measurement that verification takes the intended time.

### Independent lab (120 min)
On the PortSwigger Web Security Academy, choose three authentication labs at random from the list without reading their titles first (a die and the list order will do). Solve them, noting for each the underlying flaw in one sentence, the tool you used and the time. Then write for each one the control that would have prevented it.

### Defence and verification (60 min)
Implement login throttling: count failures per account and per source address, delay growing with the count, and a message that does not reveal whether the account exists. Test it with a script from `lin-aux` that tries a small list of synthetic passwords, show the delay engaging, show a legitimate login still works after the window, and discuss how an attacker could use your lockout against a victim and what you chose to do about it.

### Evidence and report (60 min)
Hand in the lifecycle diagram, the evidence of each planted defect, the commit and test that closed it, the timing measurement and the throttling test log. It is accepted when every defect shows a failing test before and a passing test after.

### Resources
- https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html | External | current cheat sheet; note the revision date you read | OWASP project, open licence stated by the project, ASSUMED
- https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html | External | current cheat sheet; note the revision date you read | OWASP project, open licence stated by the project, ASSUMED
- https://portswigger.net/web-security | External | Web Security Academy, content as of the day you use it | free account required; terms to be checked, ASSUMED

### Watch for
Rotating the identifier but keeping the old one valid. Locking accounts so that anyone can lock anyone. Testing only the happy path of logout.

## D15 — Authorization and object-level access

**Hand in.** Access-control matrix — users by endpoints with the expected and observed result in every cell, the missing check named for every difference, and the regression tests.

### Objectives
1. Distinguish authentication/authorization — for ten endpoints state what the server must check (who is asking, may they do this, on this object) and where it checks today [web.session-auth, web.authz-idor]
2. Examine object access failure — find and demonstrate in the lab application an insecure direct object reference and a missing function-level check, and name the exact missing check [web.authz-idor]
3. Build regression cases — write automated tests that fail while the flaw exists and pass after the fix, covering access to another user's objects and to administrative functions [web.authz-idor]

### Recall (15 min)
Without notes: draw the session lifecycle and mark the two moments the identifier must change. Why does `HttpOnly` not stop session fixation? State the rule for storing a password.

### Theory (45 min)
- Authentication answers "who"; authorization answers "may this principal do this action on this object". The second question must be asked on every request, on the server, for every object, not once at login and not by hiding links.
- Horizontal failures reach another user's objects at the same privilege level (object-level, often called insecure direct object reference or broken object level authorization); vertical failures reach functions of a higher role (function-level). An unguessable identifier is not an authorization check.
- Defensible design: one central policy check that every route calls, deny by default, ownership derived from the session rather than from the request, and tests with at least two ordinary users and one administrator.
- Read the OWASP testing guide on authorization testing and the API security list's entries on object-level and function-level authorization.

### Guided lab (120 min)
1. Add notes owned by users, a note page `/notes/ID`, an edit endpoint and an administrative page. Plant two defects on a branch: the note page checks login but not ownership, the administrative page is only linked for admins.
2. Create three accounts (two users, one admin). Demonstrate both defects with two sessions and `curl`.
3. Write the matrix before the fix (rows are roles and anonymous, columns are endpoints, cells are allowed or denied), then fill the observed column.
4. Fix with one central check, rerun the matrix, and commit the tests.

### Independent lab (120 min)
Choose three access-control labs at random from the Web Security Academy list and two access-control challenges from the OWASP Juice Shop. For each, state the missing check in one sentence and sketch the central policy that would make the class of flaw impossible.

### Defence and verification (60 min)
Write a deny-by-default test: enumerate every route of your application, call each as anonymous, as user A, as user B and as admin, and compare with a policy table. The test must fail when a route has no entry in the table. Add a route without a policy and show the test fails; add the entry and show it passes.

### Evidence and report (60 min)
Hand in the matrix with expected and observed results, a paragraph per difference naming the missing check, the central policy design, and the deny-by-default test output. It is accepted when every route appears in the matrix and the test fails on an unlisted route.

### Resources
- https://owasp.org/API-Security/editions/2023/en/0x00-header/ | External | OWASP API Security Top 10, 2023 edition | OWASP project, open licence stated by the project, ASSUMED
- https://owasp.org/www-project-juice-shop/ | External | current release; pin the container image tag you run | open source project, ASSUMED
- https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html | External | current cheat sheet; note the revision date you read | OWASP project, open licence stated by the project, ASSUMED

### Watch for
Checking the role but not the owner. Reading the owner from a request parameter. A test that only calls routes as the user who owns the data.

## D16 — Injection root causes

**Hand in.** Injection remediation — the root-cause explanation, the demonstration against your own fixture, the parameterized fix and the regression tests.

### Objectives
1. Explain injection root causes — explain how building a query or command from strings lets data become code, and where the parser boundary is, for SQL, shell and template contexts [web.injection]
2. Analyze vulnerable controlled fixture — find the injection point in your own application, demonstrate the behaviour with synthetic data and show the exact statement that results [web.injection]
3. Validate prepared statements — replace concatenation with parameters, prove the earlier inputs are inert, and state what parameters do not cover (identifiers, sort order, wildcards in `LIKE`) [web.injection]

### Recall (15 min)
Without notes: which of the two checks, who and may they, does a central policy answer? Name two ways an access-control test suite can miss a route. What is the difference between a horizontal and a vertical failure?

### Theory (45 min)
- Injection exists wherever untrusted text is concatenated into something another parser will interpret: a SQL statement, a shell command line, a template, an LDAP filter. The fix is to keep code and data separate (parameters, argument lists, safe template contexts), not to try to clean the data.
- SQL classes: in-band (the result shows in the page), blind (the effect is inferred from behaviour or timing) and out-of-band. You need to recognize them; you do not need exploit code to defend against them.
- Parameters cannot carry identifiers or keywords. A column or sort order from a user must be matched against an allow-list.
- The database account should hold only the privileges the endpoint needs, so a flaw does less. ORMs still allow raw fragments; read their escape hatches.

### Guided lab (120 min)
1. On a branch, add a search endpoint that builds its SQL by string formatting. Log every statement the database receives.
2. With synthetic data only, show three behaviours that prove the input reaches the parser: a quote that causes a syntax error, a condition that changes the number of rows returned, a boolean difference you can observe from outside. Record the statement that results each time.
3. Replace the concatenation with parameters. Re-run the same inputs and show they are treated as plain text.
4. Add a sort option and fix it with an allow-list.

### Independent lab (120 min)
Choose three SQL injection labs at random from the Web Security Academy list. For each, write the statement the application builds, the input that changes its meaning and the one-line fix. Then find one place in your own application that builds a shell command or a template from user input, and write the safe form.

### Defence and verification (60 min)
Give the endpoint a database connection that can only read the table it needs (for SQLite, open the file read-only for read paths) and show that even with the vulnerable branch an injected write fails. Run a static analyser (Bandit or Semgrep with a rule for string-built SQL) on both branches, compare its findings with your tests, and record what it found that you missed and what it missed.

### Evidence and report (60 min)
Hand in the explanation, the logged statements for each behaviour, the diff to parameters, the allow-list for sorting, the read-only evidence and the analyser comparison. It is accepted when each behaviour has a logged statement and each fix has a failing test before it.

### Resources
- https://cheatsheetseries.owasp.org/cheatsheets/SQL_Injection_Prevention_Cheat_Sheet.html | External | current cheat sheet; note the revision date you read | OWASP project, open licence stated by the project, ASSUMED
- https://cheatsheetseries.owasp.org/cheatsheets/Query_Parameterization_Cheat_Sheet.html | External | current cheat sheet; note the revision date you read | OWASP project, open licence stated by the project, ASSUMED
- https://bandit.readthedocs.io/ | External | current documentation; pin the Bandit version you install | open source project, ASSUMED

### Watch for
Escaping quotes by hand. Parameterizing the value and concatenating the table name. A test whose "attack" string is mistyped and therefore proves nothing.

## D17 — Cross-site scripting contexts

**Hand in.** XSS report — the context table, the three reproductions in your own application, the correct encoding per context and the policy that limits the impact.

### Objectives
1. Model XSS execution contexts — classify where untrusted data lands (HTML body, attribute, script string, URL, style, DOM sink) and which encoding or safe API each needs [web.xss-contexts]
2. Inspect controlled XSS — reproduce reflected, stored and DOM-based script execution in your own application with harmless markers and show where each runs [web.xss-contexts]
3. Verify context-sensitive output encoding — fix each context with the right encoding or a safe API, add a content security policy, and prove the earlier inputs are inert while legitimate input still renders [web.xss-contexts]

### Recall (15 min)
Without notes: name two things a parameter cannot carry. What privilege should a web application's database account have? Write one sentence explaining blind injection, and one reason string escaping by hand fails.

### Theory (45 min)
- The same text is safe in one context and dangerous in another. In HTML body text the dangerous characters are `<` and `&`; in a quoted attribute the quote; in a JavaScript string the quote and backslash and the closing script tag; in a URL the scheme. Encoding must match the context in which the output lands, at the point of output.
- Reflected, stored and DOM-based differ in where the data comes from and where it is written. DOM sinks such as `innerHTML` and `document.write` parse HTML; `textContent` does not.
- Frameworks escape by default and offer escape hatches (marking a value safe); the hatches are where the defects are. A strict content security policy with nonces limits what an injected script can do; `HttpOnly` limits what it can steal. Neither makes the output safe.
- Read the OWASP cheat sheets on cross-site scripting prevention and DOM-based XSS prevention.

### Guided lab (120 min)
1. Add three features on a branch: a search page that echoes the query, a comment list that stores and shows comments, a page that writes a URL fragment into the document with `innerHTML`.
2. Use a harmless marker (a script that changes the page title) to show each runs. Record in which context the marker landed in each.
3. Fix each with the right tool: context-aware template escaping, `textContent` for the DOM case, an allow-list sanitizer for the one place where limited HTML must be allowed.
4. Re-run the markers and also a set of legitimate inputs (names with apostrophes, angle brackets in code samples) to show nothing legitimate broke.

### Independent lab (120 min)
Choose three cross-site scripting labs at random from the Web Security Academy list. For each, write the context, why the first attempt failed, and the encoding that would have stopped it. Then review your own application's templates and list every place that disables escaping.

### Defence and verification (60 min)
Add a content security policy that forbids inline script and allows scripts only by nonce. Deploy it first in report-only mode, collect the violations, fix them, then enforce it. Show that the earlier stored marker is blocked even on the vulnerable branch, and list what the policy does not stop (for example injection into an allowed script's data).

### Evidence and report (60 min)
Hand in the context table with one example per context, the three reproductions, the fixes and the legitimate-input tests, and the policy with its violation log. It is accepted when each fix has a failing test before it and the legitimate inputs still render.

### Resources
- https://cheatsheetseries.owasp.org/cheatsheets/Cross_Site_Scripting_Prevention_Cheat_Sheet.html | External | current cheat sheet; note the revision date you read | OWASP project, open licence stated by the project, ASSUMED
- https://developer.mozilla.org/en-US/docs/Web/HTTP/CSP | External | current MDN web documentation | open content licence stated by MDN, ASSUMED
- https://portswigger.net/web-security/cross-site-scripting | External | Web Security Academy, content as of the day you use it | free account required; terms to be checked, ASSUMED

### Watch for
Encoding for the wrong context. Sanitizing on input and trusting the stored value everywhere. Judging success by a popup instead of by the context.

## D18 — Week 3 assessment (blind practical)

**Hand in.** Week 3 assessment — the report of the blind practical: the defects found with proof, their root causes, the fixes with regression tests and the verification.

### Objectives
1. Analyze blind web task — review an unfamiliar web application for the week's classes of defect and report each with a reproduction [web.authz-idor, web.injection, web.xss-contexts]
2. Diagnose security failure — separate symptom from cause for each finding, naming the missing check, the unsafe construction or the wrong output context [web.session-auth, web.authz-idor]
3. Verify remediation and document evidence — fix each defect with the smallest change, prove it with a test that failed before, and record the evidence with hashes [web.http-semantics, web.authz-idor]

### Blind practical (120 min)
Open the assessment in the application, start the attempt and read the task statement. Take the application from the source you fixed before the week and do not read its code or its list of defects before the clock starts. Stop at 120 minutes.

### Remediation (60 min)
Grade yourself against the rubric before reading any answer key. For every criterion below your target, find the cause in your method (which class of defect you did not test for, which request you did not replay as a second user) and fix the gap with a short targeted exercise on your own lab application. Then redo the part that failed on a different instance.

### Report (60 min)
Write the report in the structure of the task statement: findings ordered by impact, each with reproduction, root cause, fix, regression test and verification; what you did not test. Attach the evidence with SHA-256 values.

### Performance review (60 min)
Compare the grade with the week's targets: independent execution, a second user used for every access test, root cause rather than symptom, regression tests that fail before the fix, evidence quality, honest limits. Record the three habits that cost you most and one change for next week.

### Assessment
- Code: W03-BLIND
- Title: Week 3 blind practical: review and repair an unfamiliar web application
- Kind: BLIND_PRACTICAL
- Competency: web.authz-idor
- Difficulty: 3

Task statement:
You have 120 minutes. You are given a small web application you have not read, running on your isolated lab network, with synthetic accounts and data. Work only inside the lab and on the application named in your scope statement.

Scenario source (fixed before the week, not inspected): an application prepared by another person who has planted defects from the catalogue below, or, working alone, one of the following chosen at random and never opened before: an unseen intermediate-level lab of a training platform, an unseen challenge of the OWASP Juice Shop, or a variant of your own lab application produced by a script you wrote earlier but have not read since. Defects are drawn from: an object reference with no ownership check; a function reachable without the required role; session fixation or a session that survives logout; string-built SQL; a stored or reflected script injection in a specific output context; cookie, CORS or content security policy settings that undo another control.

Deliverables, in this order. First, a list of findings, each with a safe reproduction using synthetic data and the request or input that shows it. Second, for each finding the root cause (the missing check, the unsafe construction or the wrong context), not the symptom. Third, for each finding a fix and a regression test that fails before the fix and passes after. Fourth, what you did not test and why. Fifth, a report of at most three pages with the evidence hashes.

You are assessed on: independent execution during the block; a second user used for every access test; root cause stated rather than symptom; a regression test that fails before the fix; evidence quality; honest limits. The pass rule is non-compensatory: every criterion must reach the pass level.

### Resources
- https://owasp.org/www-project-web-security-testing-guide/ | External | Web Security Testing Guide, stable release to be confirmed when you read it | OWASP project, open licence stated by the project, ASSUMED

### Watch for
Testing every endpoint as the same user. Stopping at the first finding. A fix with no test that failed before it.
