# Week 6 — Active Directory and identity

**Purpose.** In most organisations identity is the control plane: whoever can act as a privileged principal can act as the organisation. This week you build a small Active Directory forest, learn to read it as a graph of trust and permission rather than as a directory of names, understand Kerberos well enough to diagnose it when it fails, find the delegation and permission paths that lead to privileged groups, close them, and write the detections that would show someone using them. The privileged group in your lab is a surrogate, so nothing you demonstrate can damage the domain.

**Lab stack.** The week 5 lab plus a domain: one domain controller, one member server and one client from Microsoft evaluation images (verify the terms and the evaluation period at download), forest `corp.lab.test` on the isolated network, 16 GB of RAM for the three guests at the least. Tools: the PowerShell Active Directory module, `dsacls`, `setspn`, `nltest`, `klist`, Wireshark's Kerberos dissector, BloodHound Community Edition for graph analysis of your own lab, and `Get-WinEvent`. A lighter fallback is a Samba domain controller on Linux; it differs from Windows in places you must note. The Game of Active Directory lab is an optional extra for week 12.

**Competencies.** `ad.forest-trust-model`, `ad.kerberos`, `ad.delegation-permissions`, `ad.identity-detection` (pack `windows-identity-ad`), with `win.principals-services` and `host.persistence-telemetry` revisited.

**Accelerator.** An identity engineer who can draw the Kerberos exchange with the ticket contents from memory in the Tuesday recall block can treat day 32 as review; days 33 to 35 and the Saturday assessment stay.

## D31 — Forest, domain and trust model

**Hand in.** Identity diagram — the forest, domains, trusts and tiers of the lab as a graph, with the direction of trust, the privileged groups and their effective members, and the boundaries marked.

### Objectives
1. Explain forest/domain/trust model — explain the roles of forest, domain, domain controller, global catalog, site and trust, and why the forest rather than the domain is the security boundary [ad.forest-trust-model]
2. Model principal relationships — model the users, groups (nesting and scope), computers and service accounts of the lab domain and their relationships as a graph [ad.forest-trust-model, win.principals-services]
3. Identify identity trust boundaries — mark where trust crosses (trusts, replication, delegation, administrative tiers), state which way trust flows and what security identifier filtering changes [ad.forest-trust-model]

### Recall (15 min)
Without notes: which mechanism in last week's visibility matrix had no telemetry, and what would you enable to record it? What did your baseline check report that surprised you? Why does a hardening setting need a behavioural test?

### Theory (45 min)
- A forest is the top of an Active Directory deployment: a shared schema, configuration and global catalog, with one or more domains in trees. Domain controllers replicate the directory within a domain and the configuration across the forest. A domain is an administrative and replication unit; the vendor's position is that the forest is the security boundary, because an administrator of any domain can reach the shared parts.
- A trust lets principals of one domain be authenticated for resources of another. It has a direction (the trusting domain trusts the trusted one), may be transitive, and can be filtered or limited (security identifier filtering, selective authentication).
- Groups have scopes (domain local, global, universal) and nest; effective membership is the transitive closure. The tiered administration model separates control of the domain controllers and identity systems (tier 0) from servers (tier 1) and workstations (tier 2) and forbids credentials of a higher tier on a lower one.
- Read the vendor documentation on the directory service, on trusts and on the enterprise access model.

### Guided lab (120 min)
1. Promote the first server to a domain controller of a new forest `corp.lab.test`. Create organisational units for the three tiers, ten users, six groups with nesting (including a group `Lab-Tier0` that stands in for domain administrators), two service accounts and the three computers. Join the member server and the client.
2. Enumerate with the Active Directory module and export users, groups, memberships and computers to files.
3. Write a Python script that reads the export, builds the membership graph, and reports the effective members of `Lab-Tier0` (the transitive closure) and every path by which a user is a member.
4. Draw the model with the trust and tier boundaries and state, for each boundary, what crosses it and who can cross.

### Independent lab (120 min)
Design, on paper and then in the lab as far as memory allows, a layout for a company with a head office, a subsidiary and an acquired company that must keep its own administrators. Decide forests, domains and trusts, the direction of each trust and whether to filter or limit it, and justify each as a statement about who is allowed to authenticate where. If the hardware allows, build a second small forest and a one-way trust and test authentication across it with selective authentication on and off.

### Defence and verification (60 min)
Add tier checks to your script and to the event logs: report any member of `Lab-Tier0` who has logged on to a tier 1 or tier 2 computer (from the logon events), and any privileged group member outside an approved list. Plant one violation of each kind in the lab and show both reports find it and stay silent on the clean domain.

### Evidence and report (60 min)
Hand in the diagram with boundaries and trust directions, the exports and the closure output, the tier checks with their before and after output, and the design of the three-company layout with its reasons. It is accepted when every boundary on the diagram states who can cross it and every effective member of `Lab-Tier0` is explained.

### Resources
- https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/get-started/virtual-dc/active-directory-domain-services-overview | External | current documentation; note the Windows Server release | vendor documentation, terms to be checked, ASSUMED
- https://learn.microsoft.com/en-us/security/privileged-access-workstations/privileged-access-access-model | External | enterprise access model, current page; note the date you read it | vendor documentation, terms to be checked, ASSUMED
- https://www.microsoft.com/en-us/evalcenter/ | External | evaluation images as of the day you download; note the product and build | vendor terms apply; evaluation period to be checked, ASSUMED

### Watch for
Treating a domain as a security boundary. Counting only direct members of a privileged group. Building the model from the names in the console instead of from the exported data.

## D32 — Kerberos issuance and validation

**Hand in.** Kerberos analysis — the exchange captured and annotated message by message, the events that correspond to each, and the diagnosis of the failure you planted.

### Objectives
1. Explain Kerberos issuance/validation — explain who issues and validates each ticket (ticket-granting ticket, service ticket), what is encrypted with whose key, what the privilege attribute data carries, and what pre-authentication prevents [ad.kerberos]
2. Interpret auth events — correlate the ticket and logon events on the domain controller and on the service host with the messages in a capture [ad.kerberos, win.principals-services]
3. Diagnose controlled ticket failure — from a symptom find the cause of a planted failure (clock skew, a missing or duplicate service principal name, a service account whose password changed, a wrong encryption type) [ad.kerberos]

### Recall (15 min)
Without notes: what makes a trust one-way and what does the arrow mean? List the three group scopes. Which tier may a tier 0 credential touch?

### Theory (45 min)
- The client proves its identity to the key distribution centre (on the domain controller) and receives a ticket-granting ticket encrypted with the domain's ticket-granting key. It presents that to request a service ticket for a named service, encrypted with the service account's key; the service decrypts it and learns who the client is and which groups, from the privilege attribute data inside. The client never sees the key of the service and the service never contacts the domain controller to verify.
- Pre-authentication makes the client prove knowledge of its key before a ticket is issued. Ticket lifetime, renewal, clock skew (five minutes by default) and encryption types (the AES types, and the older RC4) are configuration and are where most failures come from. Names that identify services (service principal names) must be unique and must be registered on the account that runs the service.
- Read RFC 4120 for the protocol and the vendor documentation of the protocol's extensions for the Windows behaviour.

### Guided lab (120 min)
1. Capture the traffic between the client, the domain controller and the member server while the client opens a file share. Identify the authentication service exchange, the ticket-granting exchange and the application exchange; annotate realm, client name, service name, encryption types and flags.
2. On the client inspect the ticket cache (`klist`). On the controller and the member server find the events for the ticket requests and the logon and match each to a message in the capture.
3. Register a service principal name for a test service and show the ticket request for it. Then break things one at a time: move the client's clock ten minutes, register a duplicate name, change the service account's password without updating the service, restrict the encryption types. For each, record the symptom, the error in the capture and the event, and the repair.

### Independent lab (120 min)
Prepare three failures on restored snapshots and draw one at random. Starting from "the file share asks for a password again", find the cause with the capture, the events and `setspn` only. Write the order in which you ruled hypotheses out and what finally settled it. Repeat with a second failure and a different tool.

### Defence and verification (60 min)
Harden the domain's Kerberos: require the AES types, give service accounts long random passwords or use managed service accounts, and confirm services keep working. Write a query over the ticket events that lists service tickets issued with the older encryption type, and show it finds a request you forced and is empty after the hardening.

### Evidence and report (60 min)
Hand in the annotated capture, the events matched to messages, the failure table (symptom, error, cause, repair) with your own captures, and the hardening evidence. It is accepted when each message of the exchange is matched to an event and each failure shows its error in a capture or log.

### Resources
- https://www.rfc-editor.org/rfc/rfc4120 | External | RFC 4120 (July 2005) | IETF document, freely available
- https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-kile/ | External | Kerberos protocol extensions specification, current revision; note the revision | vendor open specification, terms to be checked, ASSUMED
- https://www.wireshark.org/docs/wsug_html_chunked/ | External | current User's Guide; pin the Wireshark release you install | project documentation, ASSUMED

### Watch for
Diagnosing from the first error message without the capture. Duplicate service names on two accounts. Forgetting that a changed password changes the key the service tickets are encrypted with.

## D33 — Delegation types and excessive permissions

**Hand in.** Delegation review — every delegation in the lab domain classified with the risk of its type, the permissions on privileged objects reviewed, and the controls specified with the check that proves each.

### Objectives
1. Examine delegation types — explain unconstrained, constrained and resource-based constrained delegation, what each lets a service do with a client's identity, and how to find each in a directory [ad.delegation-permissions]
2. Analyze excessive permissions — review the access control lists of privileged users, groups and computers for rights that let a lower-privileged principal change them, and the group nesting that grants them indirectly [ad.delegation-permissions]
3. Specify controls — for each finding write the control (remove, restrict, replace, protect) and the check that proves it, in a form someone else can apply [ad.delegation-permissions]

### Recall (15 min)
Without notes: draw the Kerberos exchange and mark the key each message is encrypted with. What does clock skew do, and what is the default tolerance? Why must a service name be unique?

### Theory (45 min)
- Delegation lets a service act toward other services with a client's identity. Unconstrained delegation makes the service hold the forwardable ticket-granting tickets of every client that authenticates to it, so a compromised host becomes every one of those clients. Constrained delegation limits the target services (and may allow the service to obtain tickets for a user who never authenticated to it). Resource-based delegation moves the decision to the target, which names who may delegate to it.
- Accounts can be protected: marked as sensitive and not delegable, placed in the protected users group; the machine account quota controls who can create computer accounts, which matters for resource-based delegation.
- Rights on directory objects decide who can reset a password, add a member, write attributes or change permissions. The dangerous ones are generic-write, write-DACL, write-owner, all-extended-rights, and the right to add members to a group. Rights on a group also flow through nesting.
- Read the vendor documentation on delegation and on securing privileged accounts.

### Guided lab (120 min)
1. Configure three services on three computers: one with unconstrained delegation, one with constrained delegation (once with protocol transition, once without) and one with resource-based delegation. Find each with the directory module (the trusted-for-delegation flag, the allowed-to-delegate-to list, the allowed-to-act-on-behalf attribute).
2. For the unconstrained host show from the host's own ticket cache which clients' tickets it holds after a client connects.
3. Extract the access control lists of `Lab-Tier0`, its members' objects, the domain root and the domain controllers' organisational unit with `dsacls` or the directory module. List every non-default right granted to a principal that is not an administrator.
4. Write the finding table: what, why it matters, control, check.

### Independent lab (120 min)
Prepare three flawed configurations on restored snapshots (an unconstrained host, a helpdesk group with a write right over a group, a machine account quota that lets any user add computers). Draw one at random and find the weakness using only directory queries. Write the control and a query that proves it afterwards.

### Defence and verification (60 min)
Write an audit (PowerShell) that reports every delegation, every non-default right on privileged objects held by non-administrators, the machine account quota, and privileged accounts not marked sensitive. Run it on the flawed and the fixed domain. Apply the controls from the finding table and show each check passing.

### Evidence and report (60 min)
Hand in the delegation table, the permission review, the finding table with controls and checks, and the audit's before and after output. It is accepted when each finding has a control and a check that fails before and passes after.

### Resources
- https://learn.microsoft.com/en-us/windows-server/security/kerberos/kerberos-constrained-delegation-overview | External | current documentation; note the date you read it | vendor documentation, terms to be checked, ASSUMED
- https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/plan/security-best-practices/best-practices-for-securing-active-directory | External | current documentation; note the date you read it | vendor documentation, terms to be checked, ASSUMED

### Watch for
Reading the delegation tab in the console and missing the attributes the console does not show. Fixing the object and not the group nesting that grants the right. Granting a service more delegation to make a failure go away.

## D34 — Isolated AD misconfiguration and privilege paths

**Hand in.** Identity exposure report — the privilege paths from an ordinary account to the surrogate privileged group, the evidence for each step, the defensive fixes and proof that the paths are gone.

### Objectives
1. Analyze isolated AD misconfiguration — in a prepared copy of the lab domain find the planted misconfigurations (a delegated right over a group, nested membership, a weak service account password policy, an unconstrained host) from enumeration alone [ad.delegation-permissions]
2. Map privilege paths — turn the enumeration into a graph and find every path from a low-privilege user to `Lab-Tier0`, stating for each edge the right or membership that makes it [ad.delegation-permissions, ad.forest-trust-model]
3. Design defensive fix — choose for each path the edge to cut with the least collateral effect, apply it, and show by re-collecting that the path is gone [ad.delegation-permissions]

### Recall (15 min)
Without notes: which delegation type leaves other users' tickets on the service host? Which right on a group lets a user add themselves to it? Name two attributes the console does not show for delegation.

### Theory (45 min)
- Privilege in a directory is a graph: edges are memberships, rights (password reset, write members, write DACL, owner), sessions (who is logged on where), delegation and trusts. An attack path is a path in that graph; a defensive path is the same path, found first. Graph tools collect this data with ordinary read access and are used by defenders to find and cut the edges.
- Cutting a path means removing the cheapest edge that does not break a legitimate task: usually a right granted too widely, a nesting that should not exist, or a session that should not happen. Verify by collecting again.
- Collection is itself an event on the controllers (many reads); do it in a window and expect it in your own telemetry.

### Guided lab (120 min)
1. Restore the domain to a snapshot and plant four misconfigurations that you write down in a sealed note: a helpdesk group with a write-members right on a group nested in `Lab-Tier0`, a service account with a password in the description field, an unconstrained host, a user with a write right over another user who is a member of a privileged group.
2. Collect the domain data with BloodHound Community Edition's collector (or your own queries) and load it. Find all paths from a low-privilege user to `Lab-Tier0`.
3. For each path state every edge's cause. Verify one path by performing its first step in the lab on a test group only.
4. Cut each path with the cheapest edge, collect again, and show the paths are gone.

### Independent lab (120 min)
Restore the snapshot and apply a different set of misconfigurations chosen with a die from a list you wrote earlier. Without reading the list find all paths in the data, rank them by how much each edge would cost to cut, and propose the cuts. Then compare with the list and score what you missed and why.

### Defence and verification (60 min)
Turn "no path from an ordinary account to `Lab-Tier0`" into a repeatable check: collect, query for paths, fail when any exist. Run it on the planted domain (fails with the four paths) and on the fixed domain (passes). Record how long collection takes and what a path through a trust or an off-domain system would escape.

### Evidence and report (60 min)
Hand in the sealed note's planted list (after the exercise), the paths with per-edge causes, the verification of one step, the cuts and the second collection, and the repeatable check's output before and after. It is accepted when every path has a cause for every edge and the check fails before and passes after.

### Resources
- https://github.com/SpecterOps/BloodHound | External | Community Edition, current release; pin the version you run | open source project, licence to be checked, ASSUMED
- https://learn.microsoft.com/en-us/windows-server/identity/ad-ds/plan/security-best-practices/best-practices-for-securing-active-directory | External | current documentation; note the date you read it | vendor documentation, terms to be checked, ASSUMED

### Watch for
Planting a flaw on the real domain administrators group. Fixing the path the tool drew and missing a second one it cannot see. Trusting a graph from a collection that was missing a computer.

## D35 — Identity telemetry and detection

**Hand in.** Identity defense report — the identity telemetry inventory with its gaps, six detection hypotheses with expected baseline and false positives, and the results of testing four of them.

### Objectives
1. Identify identity telemetry — list the identity events and sources on the controllers and members, enable the auditing they need, and record what is still not collected [ad.identity-detection]
2. Build suspicious-behavior hypotheses — write six hypotheses that tie an abuse behaviour to the telemetry that would show it, with the normal baseline and the reasons for false positives [ad.identity-detection]
3. Test hardening/detection — for four hypotheses generate benign lab activity that mimics the behaviour, show whether the detection fires, record false positives and show the hardening that reduces the behaviour [ad.identity-detection, host.persistence-telemetry]

### Recall (15 min)
Without notes: what is a path in a privilege graph and what is a cut? Which edges did the collection find that you had not planted? How would you tell your own collection from someone else's in the logs?

### Theory (45 min)
- The sources: ticket and logon events (authentication service and ticket requests, failed pre-authentication, credential validation, logon with its type and source), special-privilege events, directory-service access and change events (object read, attribute written, member added to a group), account-management events, and endpoint events from Sysmon. Most directory events need auditing configured on the objects, not only the policy.
- A detection hypothesis has the form: if this behaviour happens, then this event with these fields appears; normal activity looks like this; these benign cases look the same. Write the baseline from measurements of your own lab, not from guesses.
- Examples to build from: service tickets with the older encryption type; one account requesting tickets for many different service names in a short time; a privileged group changing membership outside a window; directory replication rights exercised by something that is not a controller; a service principal name added to a user; a tier 0 account logging on to a lower tier.

### Guided lab (120 min)
1. Enable the audit policies and the object auditing (directory service changes and access) needed for your six hypotheses. Verify each source produces events.
2. Record a baseline: a script produces a day's normal activity (logons, file shares, a few administrative changes) and you note the rates.
3. Write each hypothesis in plain language with the event ids, the fields and a threshold.
4. Generate benign instances of four behaviours in the lab (use a test group and test accounts) and run the queries with `Get-WinEvent` and an XPath filter. Record which fired.

### Independent lab (120 min)
Restore the lab, run the baseline script and then three behaviours drawn at random from your list of six without noting which. Run your detections blind and name what happened and when. Record the missed ones, the false positives and the time to detection.

### Defence and verification (60 min)
Tune thresholds against the false positives you measured. For two hypotheses apply the hardening that removes the behaviour (for example the encryption types, a restricted right) and show the telemetry stops. Write each detection as pseudocode with the event fields so it can become a portable rule next week, and state what each detection cannot see.

### Evidence and report (60 min)
Hand in the telemetry inventory with gaps, the six hypotheses with baselines, the test results with false positives, the tuning, and the pseudocode. It is accepted when each hypothesis has a measured baseline and each tested one has a result, fired or missed, with the event.

### Resources
- https://learn.microsoft.com/en-us/windows/security/threat-protection/auditing/advanced-security-auditing | External | current auditing documentation; note the date you read it | vendor documentation, terms to be checked, ASSUMED
- https://attack.mitre.org/techniques/enterprise/ | External | ATT&CK Enterprise techniques; v20 (October 2026) reported current, pin the release you read | MITRE terms of use; CHECKED 2026-11-14 by search result only, page not opened

### Watch for
Writing thresholds before measuring a baseline. Auditing enabled in the policy but not on the objects. A detection tested only against the behaviour it was written for.

## D36 — Week 6 assessment (blind practical)

**Hand in.** Week 6 assessment — the report of the blind practical: the privilege paths and misconfigurations found, the root cause of each, the fixes with proof and the detections.

### Objectives
1. Analyze blind identity case — enumerate an unfamiliar domain and find its delegation, permission and Kerberos problems [ad.kerberos, ad.delegation-permissions]
2. Explain privilege/trust root cause — state for each finding the trust or permission that makes it possible, in terms of the graph [ad.forest-trust-model]
3. Recommend and verify fixes — choose the cheapest cut for each path, apply it, prove it by re-enumeration, and write the detection that would show abuse [ad.identity-detection]

### Blind practical (120 min)
Open the assessment in the application, start the attempt and read the task statement. Take the domain from the source you fixed before the week and do not read its setup notes before the clock starts. Stop at 120 minutes.

### Remediation (60 min)
Grade yourself against the rubric before reading any answer key. For every criterion below your target find the cause in your method (the enumeration you did not run, the edge you did not explain) and fix it with a short exercise on your lab domain. Then redo the failed part on a different restored copy.

### Report (60 min)
Write the report in the structure of the task statement: findings by impact with evidence, graph path and root cause; the cut and its proof; the detection; residual risk and what you did not enumerate. Attach the evidence with SHA-256 values.

### Performance review (60 min)
Compare the grade with the week's targets: independent execution, every path explained edge by edge, cuts proven by re-enumeration, detections tied to events, honest limits. Record the three habits that cost you most and one change for next week.

### Assessment
- Code: W06-BLIND
- Title: Week 6 blind practical: identity exposure in an unfamiliar domain
- Kind: BLIND_PRACTICAL
- Competency: ad.delegation-permissions
- Difficulty: 4

Task statement:
You have 120 minutes. You are given an Active Directory domain you have not seen, on your isolated lab network, with a low-privilege account to start from and a surrogate privileged group in place of the real one. Work only inside the lab and on the domain named in your scope statement. Use test objects for any demonstration.

Scenario source (fixed before the week, not inspected): a domain prepared by another person from the catalogue below, or, working alone, a restored snapshot of your lab domain faulted by a script you wrote earlier but have not read since, or an unseen variant of a deliberately vulnerable directory lab. Faults are drawn from: a delegated right over a group or user that reaches the surrogate group; nested group membership that grants more than it appears to; an unconstrained or over-constrained delegation; a service account with a weak or exposed password; a computer account quota that lets any user create machines; a trust without filtering or selective authentication; a Kerberos configuration that fails or is weakened (skew, duplicate service name, older encryption).

Deliverables, in this order. First, an enumeration record: commands, collection and the graph you built. Second, each finding with its path to the surrogate group, the right or membership behind every edge, and the root cause. Third, for each finding the cheapest cut, the change applied and the re-enumeration that proves it. Fourth, for each finding a detection hypothesis with the events and fields. Fifth, residual risk and what you did not enumerate. Sixth, a report of at most four pages with the evidence hashes.

You are assessed on: independent execution during the block; every path explained edge by edge; cuts proven by re-enumeration; detections tied to specific events; honest limits. The pass rule is non-compensatory: every criterion must reach the pass level.

### Resources
- https://learn.microsoft.com/en-us/security/privileged-access-workstations/privileged-access-access-model | External | enterprise access model, current page; note the date you read it | vendor documentation, terms to be checked, ASSUMED

### Watch for
Demonstrating on a real privileged group. Reporting a tool's finding without the edge that makes it true. Leaving a planted right in place after the exercise.
