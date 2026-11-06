#!/usr/bin/env python3
"""gen_competency_packs.py — generate the eight G9 knowledge packs (data/competency-packs/<pack>/pack.json + README.md)
from the structured table below. Re-run after editing the table; validate with
    python3 -I scripts/check_competency_packs.py data/competency-packs --expect-count 8
Status of every pack is PROPOSED. NICE v2.2.0 TKS ids are NOT pinned here: each nice_ref carries
confidence ASSUMED and a descriptive placeholder; pin real ids from the NICE v2.2.0 release artifact
(docs/evaluations and docs/curriculum) before promotion.
"""
from __future__ import annotations
import json, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "data" / "competency-packs"
RUBRIC = ["independent_execution", "root_cause_reasoning", "remediation_verification",
          "detection_visibility", "evidence_documentation", "scope_compliance_reporting"]
DIMS = ["RECALL", "UNDERSTANDING", "APPLICATION", "TRANSFER"]

# pack_id, title, owner module, seed weeks, isolation default, visibility, safety, sources, competencies
# competency tuple: (id, name, description, (min,max), [prereqs], validity_days, [(misconception, counterexample)x2], assessment focus)
PACKS = [
 ("linux-python-systems", "Linux, Python and systems engineering", "M12", "Week 1", "VM",
  "Host telemetry: auth/syslog/journald, process and file auditing (auditd), package and service state; every diagnostic leaves an evidence trail.",
  ["Work only on the isolated lab VM; never run diagnostics on the G9 host without an approved operation",
   "Preserve logs and baselines before changing state", "No credentials in notes or fixtures"],
  [("Linux Filesystem Hierarchy Standard", "Linux Foundation", "STANDARD", "freely available specification", "ASSUMED"),
   ("Python 3 documentation", "Python Software Foundation", "DOC", "PSF license, freely available", "ASSUMED"),
   ("OverTheWire Bandit", "OverTheWire", "LAB", "free public wargame; terms must be checked before embedding", "ASSUMED")],
  [("lab.authorization", "Lab authorization and boundaries", "Define scope, authorization and host/guest isolation for all practical work.", (1,2), [], 180,
    [("A home lab needs no written scope", "Unscoped testing against a shared VM host can touch other tenants; the capstone charter still requires scope"),
     ("Snapshots make destructive actions safe", "Snapshots do not protect data exfiltrated or leaked outside the VM")], "produce a lab charter with scope, isolation and evidence rules"),
   ("linux.filesystem", "Filesystem hierarchy, permissions and ownership", "Explain FHS, mode bits, ownership, special bits and ACLs; diagnose access failures.", (1,3), ["lab.authorization"], 180,
    [("chmod 777 fixes permission errors", "It removes the control; the failing component usually needs a group or ACL change"),
     ("The setuid bit makes any binary run as root", "Only when the owner is root and the filesystem allows suid")], "diagnose a controlled access failure and fix it with least privilege"),
   ("linux.processes-services", "Processes, services and logging", "Inspect processes, systemd units, journald and syslog; reproduce a troubleshooting workflow.", (2,3), ["linux.filesystem"], 180,
    [("A stopped service means the binary is broken", "Dependency ordering or a failed unit condition is more common; journalctl -u shows the reason"),
     ("Logs are complete by default", "Rate limiting and rotation drop lines; check journald limits")], "reproduce and document a service failure diagnosis"),
   ("python.secure-parsing", "Secure parsing of structured security records", "Parse JSON/CSV/log formats in Python with validation, malformed-input handling and fixtures.", (2,4), ["linux.processes-services"], 240,
    [("try/except around the whole parser is error handling", "It hides which record failed; validate per record and report counts"),
     ("eval() is acceptable for trusted files", "Trust is a property of provenance, not of the file name; use json/ast.literal_eval")], "write a parser with fixtures covering malformed input"),
   ("python.testing-fixtures", "Testing with fixtures and reproducible runbooks", "Write tests with fixtures, version-control the work and produce a reproducible runbook.", (2,4), ["python.secure-parsing"], 240,
    [("A passing test proves correctness", "It proves the fixture case; negative and malformed cases are needed"),
     ("Runbooks are documentation only", "A runbook is executable evidence; untested steps are assumptions")], "deliver a tested parser plus a runbook another engineer can replay"),
   ("linux.evidence-completeness", "Evidence completeness assessment", "Judge whether collected host evidence supports a conclusion; identify gaps.", (3,4), ["linux.processes-services"], 240,
    [("More logs always mean better evidence", "Unrelated volume hides gaps; evidence must map to the claim"),
     ("Timestamps are consistent across sources", "Clock skew and zones differ; normalize before correlating")], "report remediation and limitations from a blind troubleshooting case"),
   ("linux.hardening-baseline", "Hardened baseline and configuration verification", "Establish a baseline, prioritize host weaknesses and test configuration controls.", (3,5), ["linux.evidence-completeness"], 240,
    [("A benchmark score is a security posture", "Controls must be verified in effect, not only configured"),
     ("Disabling services is always safe", "Dependencies and monitoring agents may break; test and document")], "verify a hardening change with before/after evidence"),
   ("linux.blind-troubleshooting", "Blind Linux troubleshooting", "Diagnose an unfamiliar host fault end to end with evidence and a remediation report.", (3,5), ["linux.hardening-baseline", "python.testing-fixtures"], 180,
    [("Reboot first", "A reboot destroys volatile evidence; capture state first"),
     ("One cause explains all symptoms", "Multiple faults coexist; keep competing hypotheses")], "blind practical with rubric scoring on independent execution and root cause")]),
 ("networking-protocols-recon", "Networking, protocol analysis and scoped reconnaissance", "M12", "Week 2", "VM",
  "Flow records, packet captures, firewall and DNS logs; scoped discovery produces an auditable target list and timing.",
  ["Reconnaissance only against explicitly scoped lab targets", "No scanning from the G9 host; use the isolated lab network", "Capture files may contain credentials: classify PRIVATE"],
  [("RFC 793 / RFC 9293 TCP", "IETF", "STANDARD", "public RFC", "ASSUMED"), ("Wireshark User's Guide", "Wireshark Foundation", "DOC", "GPL documentation", "ASSUMED"),
   ("Nmap Reference Guide", "Nmap Project", "DOC", "freely available; tool license is NPSL", "ASSUMED")],
  [("net.layers-addressing", "Layers, addressing and subnetting", "Map OSI/TCP-IP layers, addressing and subnetting; diagram pathways.", (1,2), [], 240,
    [("Layers are strictly separated", "Encapsulation leaks: MTU and QoS cross layers"), ("A /24 always has 254 usable hosts", "Point-to-point and reserved addresses change the count")], "produce a network architecture map from captures"),
   ("net.tcp-states", "TCP states and failure interpretation", "Explain TCP state transitions; interpret retransmission and failure in PCAPs.", (2,3), ["net.layers-addressing"], 180,
    [("RST always means an attack", "Closed ports and application resets are normal"), ("Retransmissions prove packet loss on the link", "Receiver stalls and middleboxes also cause them")], "diagnose lab connectivity from a PCAP"),
   ("net.dns-resolution", "DNS resolution and caching", "Trace recursive/authoritative resolution; analyze cache and failure modes.", (2,3), ["net.layers-addressing"], 180,
    [("NXDOMAIN means the name never existed", "Negative caching and split horizon produce it"), ("Flushing cache fixes DNS", "Upstream TTLs and stale authoritative data persist")], "investigate a controlled DNS defect"),
   ("net.routing-segmentation", "Routing, NAT and segmentation", "Explain route selection, NAT and firewall policy; verify segmentation.", (2,4), ["net.tcp-states"], 240,
    [("Longest prefix always wins", "Administrative distance and policy routing override"), ("NAT is a security control", "It hides addresses, not vulnerabilities")], "verify segmentation controls with evidence"),
   ("recon.scope-definition", "Reconnaissance scope definition", "Define and document the scope, authorization and rules of engagement for discovery.", (2,3), ["lab.authorization"], 180,
    [("In-scope IP ranges imply in-scope services", "Third-party hosted services need separate authorization"), ("Passive recon needs no scope", "It still touches third-party data and terms")], "write a scoped recon plan"),
   ("recon.discovery-interpretation", "Discovery result interpretation", "Interpret discovery output and translate observations into risks.", (3,4), ["recon.scope-definition", "net.routing-segmentation"], 180,
    [("An open port is a vulnerability", "Exposure plus a weakness plus impact is a risk"), ("Version banners are accurate", "Banners are often faked or backported")], "scoped recon report with risk translation"),
   ("net.pcap-analysis", "Unfamiliar PCAP analysis", "Analyze an unfamiliar capture and write an evidence-backed conclusion.", (3,5), ["net.tcp-states", "net.dns-resolution"], 180,
    [("The first anomaly is the root cause", "Correlate timing across streams before concluding"), ("Encrypted traffic cannot be analyzed", "Metadata, SNI, timing and sizes carry evidence")], "blind PCAP analysis assessment"),
   ("net.failure-diagnosis", "Network failure diagnosis", "Diagnose a network failure end to end and document the evidence chain.", (3,5), ["net.pcap-analysis", "recon.discovery-interpretation"], 180,
    [("Ping success means the path works", "ICMP may be allowed while TCP is filtered"), ("MTU problems show as total loss", "They show as hangs after the handshake")], "week-2 blind assessment")]),
 ("web-api-appsec", "Web, API and application security", "M12", "Weeks 3-4", "CONTAINER",
  "Application logs, WAF/proxy logs, authentication events; each vulnerability class maps to a detection signal and a verified fix.",
  ["Only deliberately vulnerable lab applications (OWASP Juice Shop, PortSwigger labs); never production targets", "No credential reuse between lab and real accounts", "Keep exploit artifacts inside the lab evidence store, classified SECURITY_SENSITIVE"],
  [("OWASP Web Security Testing Guide", "OWASP", "STANDARD", "CC BY-SA 4.0", "ASSUMED"), ("OWASP ASVS", "OWASP", "STANDARD", "CC BY-SA 4.0", "ASSUMED"),
   ("PortSwigger Web Security Academy", "PortSwigger", "LAB", "free account; terms to verify before embedding", "ASSUMED"), ("OWASP Juice Shop", "OWASP", "LAB", "MIT", "ASSUMED")],
  [("web.http-semantics", "HTTP semantics and browser trust boundaries", "Explain requests, cookies, origins and headers; map browser trust boundaries.", (1,2), [], 240,
    [("SameSite cookies stop all CSRF", "Lax allows top-level GET navigations"), ("HTTPS makes an application secure", "Transport security says nothing about logic")], "HTTP analysis deliverable"),
   ("web.session-auth", "Authentication and session state", "Model authentication/session state and test mitigations for session defects.", (2,3), ["web.http-semantics"], 180,
    [("Long random session ids are enough", "Fixation, scope and rotation after login matter"), ("JWTs cannot be revoked so they are stateless by design", "Short lifetimes plus a denylist are required for logout")], "session defect review with regression cases"),
   ("web.authz-idor", "Authorization and object-level access control", "Distinguish authn/authz; find and fix object-level access failures; build regressions.", (2,4), ["web.session-auth"], 180,
    [("Unguessable ids prevent IDOR", "Ids leak; authorization must be checked server-side"), ("Role checks at the UI are sufficient", "APIs are called directly")], "access-control matrix and regression cases"),
   ("web.injection", "Injection root causes and prepared statements", "Explain injection root causes; validate prepared statements and encoding.", (2,4), ["web.http-semantics"], 180,
    [("Escaping quotes prevents SQL injection", "Numeric contexts and second-order injection bypass it"), ("ORMs are immune", "Raw query fragments and string concatenation persist")], "injection remediation with proof"),
   ("web.xss-contexts", "XSS execution contexts and output encoding", "Model execution contexts and verify context-sensitive encoding.", (2,4), ["web.http-semantics"], 180,
    [("HTML-encoding everything is enough", "Attribute, JS and URL contexts need different encoders"), ("CSP replaces encoding", "CSP is defense in depth, not a fix")], "XSS report with verified encoding"),
   ("api.token-authz", "API identity, tokens and object-level authorization", "Map token/identity flows; check object-level authorization; define API tests.", (3,4), ["web.authz-idor"], 180,
    [("Bearer tokens are bound to the client", "Without audience/resource binding they replay"), ("Rate limiting is an authorization control", "It limits volume, not access")], "API security matrix"),
   ("web.ssrf-upload-deserialization", "SSRF, uploads and unsafe deserialization", "Explain SSRF trust boundaries, upload validation and deserialization risks; verify restrictions.", (3,5), ["api.token-authz", "web.injection"], 180,
    [("Blocking 127.0.0.1 stops SSRF", "Alternate encodings, redirects and DNS rebinding bypass denylists"), ("Checking the file extension validates an upload", "Content type, magic bytes and processing path matter")], "processing security review"),
   ("web.threat-model-logic", "Application threat modeling and business-logic invariants", "Draft data-flow models, assess business-logic and race conditions, rank and verify mitigations.", (3,5), ["web.ssrf-upload-deserialization"], 240,
    [("Threat models are a one-time artifact", "They change with every trust boundary"), ("Race conditions are theoretical in web apps", "Coupon reuse and double-spend races are common")], "application threat model and week-4 blind API assessment")]),
 ("windows-identity-ad", "Windows, identity, Active Directory and access controls", "M12", "Weeks 5-6", "VM",
  "Windows event logs (4624/4625/4672/4768/4769/4776), Sysmon, service and task changes; identity misuse leaves ticket and logon artifacts.",
  ["Isolated AD lab forest only; never domain-joined production machines", "No real credentials or hashes in fixtures", "Privilege-path analysis stays within the lab trust boundary"],
  [("Microsoft Learn: Windows security auditing", "Microsoft", "DOC", "public documentation", "ASSUMED"), ("MITRE ATT&CK Enterprise", "MITRE", "STANDARD", "ATT&CK license (free use with attribution)", "ASSUMED")],
  [("win.principals-services", "Windows principals, services and event records", "Model principals and services; interpret event records; map privilege boundaries.", (1,3), [], 240,
    [("Administrators are all-powerful by default", "UAC split tokens and protected processes limit them"), ("Event 4624 alone proves interactive logon", "Logon type distinguishes network, batch and interactive")], "Windows architecture review"),
   ("linux.privilege-boundaries", "Linux privilege boundaries: sudo, capabilities, services", "Explain sudo, capabilities and service permissions; analyze misconfiguration; verify hardening.", (2,4), ["linux.hardening-baseline"], 240,
    [("sudo NOPASSWD for one binary is harmless", "Binaries with shell escapes escalate"), ("Capabilities are weaker than root", "CAP_SYS_ADMIN is effectively root")], "Linux privilege review"),
   ("win.token-service-controls", "Windows tokens and service controls", "Analyze token privileges and service permissions; investigate privilege failures; document detection.", (2,4), ["win.principals-services"], 240,
    [("SeImpersonate is a low-risk privilege", "It enables token theft paths"), ("Service accounts cannot log on interactively", "Depends on user rights assignments")], "Windows privilege review"),
   ("host.persistence-telemetry", "Persistence mechanisms and telemetry", "Map persistence mechanisms, identify telemetry and propose detection checks.", (2,4), ["win.token-service-controls"], 240,
    [("Autoruns covers all persistence", "WMI subscriptions and scheduled task hijacks need separate collection"), ("Persistence always survives reboot", "Session-scoped persistence exists")], "host visibility matrix"),
   ("ad.forest-trust-model", "Forest, domain and trust model", "Explain forests, domains and trusts; model principal relationships and trust boundaries.", (2,3), ["win.principals-services"], 240,
    [("A one-way trust is one-way for attackers", "Trust direction and SID filtering decide"), ("Domain admins are forest-bounded", "Enterprise admins and trust abuse cross boundaries")], "identity diagram"),
   ("ad.kerberos", "Kerberos issuance and validation", "Explain ticket issuance/validation; interpret auth events; diagnose ticket failures.", (3,4), ["ad.forest-trust-model"], 180,
    [("Kerberos tickets are bound to the client host", "Tickets are bearer artifacts"), ("Pre-auth failures always mean wrong passwords", "Clock skew and missing SPNs cause them")], "Kerberos analysis"),
   ("ad.delegation-permissions", "Delegation types and excessive permissions", "Examine delegation types, excessive permissions and privilege paths; design defensive fixes.", (3,5), ["ad.kerberos"], 180,
    [("Unconstrained delegation is fine on member servers", "It allows ticket harvesting"), ("ACL inheritance is safe", "Inherited write rights create privilege paths")], "identity exposure report"),
   ("ad.identity-detection", "Identity telemetry and detection", "Identify identity telemetry, build suspicious-behavior hypotheses and test hardening/detection.", (3,5), ["ad.delegation-permissions", "host.persistence-telemetry"], 180,
    [("Failed logons are the main signal", "Successful anomalous logons matter more"), ("Honey accounts generate noise", "Any touch is high-signal")], "week-6 blind identity case")]),
 ("detection-soc-hunting", "Detection engineering, SOC and threat hunting", "M12", "Week 7", "CONTAINER",
  "Coverage maps per data source, Sigma rule tests with positive and negative fixtures, false-positive estimates.",
  ["Use licensed sample datasets only (note license per dataset)", "Rules are tested on fixtures, never deployed to production SIEMs from the lab", "No customer data"],
  [("Sigma specification", "SigmaHQ", "STANDARD", "DRL/Apache-2.0 (verify)", "ASSUMED"), ("MITRE ATT&CK", "MITRE", "STANDARD", "free with attribution", "ASSUMED"),
   ("Splunk Boss of the SOC datasets", "Splunk", "DATASET", "free download; redistribution terms must be verified", "ASSUMED")],
  [("det.event-sources", "Event source inventory and coverage", "Inventory event sources, evaluate coverage gaps and design telemetry mapping.", (1,3), [], 240,
    [("More sources equal more coverage", "Coverage is per technique, not per source"), ("EDR replaces log collection", "EDR gaps exist for identity and network")], "coverage map"),
   ("det.normalization", "Log normalization and enrichment", "Normalize events, validate time/source identity and assess enrichment.", (2,3), ["det.event-sources"], 240,
    [("Normalized fields are trustworthy", "Parser errors silently drop fields"), ("UTC conversion is automatic", "Source zones vary")], "normalization report"),
   ("det.sigma-rules", "Sigma schema and detection hypotheses", "Understand Sigma, draft a detection hypothesis and test positive/negative fixtures.", (2,4), ["det.normalization"], 180,
    [("A rule that fires on the sample is done", "Negative fixtures and FP estimates are required"), ("Sigma rules are vendor-neutral in practice", "Backend conversions change semantics")], "Sigma test package"),
   ("det.correlation-timeline", "Correlation and investigation timeline", "Correlate sources, reconstruct a timeline and separate fact from inference.", (3,4), ["det.sigma-rules"], 180,
    [("Timeline order equals causality", "Clock skew and batching reorder events"), ("Absence of logs means absence of activity", "Coverage gaps hide activity")], "investigation timeline"),
   ("det.criteria-fp", "Detection criteria and false-positive estimation", "Define detection criteria, estimate false positives and refine rules.", (3,4), ["det.sigma-rules"], 180,
    [("Low alert volume means low FP", "It may mean low coverage"), ("Tuning is whitelisting", "Whitelists create blind spots")], "detection validation report"),
   ("hunt.hypothesis", "Hunt hypothesis and approved telemetry", "Define a hunt hypothesis, examine approved telemetry and record findings/detections.", (3,5), ["det.correlation-timeline"], 180,
    [("Hunting is searching for IOCs", "Hunting tests behavioral hypotheses"), ("No findings means a failed hunt", "Negative results refine coverage")], "hunt report"),
   ("det.blind-telemetry", "Blind telemetry investigation", "Investigate blind telemetry, implement an evidence-based detection and report limitations.", (3,5), ["det.criteria-fp", "hunt.hypothesis"], 180,
    [("The detection must catch everything", "Scoped detections with known limits are valid"), ("Reporting limitations weakens the deliverable", "Limits are evidence of rigor")], "week-7 blind assessment"),
   ("det.sigma-backend-fidelity", "Backend conversion fidelity", "Verify Sigma-to-backend conversions preserve semantics with fixture tests.", (4,5), ["det.sigma-rules"], 240,
    [("Conversion is lossless", "Modifiers and field names differ per backend"), ("One fixture per rule suffices", "Edge cases per modifier are needed")], "conversion fidelity test package")]),
 ("dfir-forensics-malware", "DFIR, forensic evidence and malware triage", "M12", "Week 8", "VM",
  "Host artifacts (MFT, registry, prefetch, journals), memory and network evidence; static malware characteristics; chain-of-custody records.",
  ["Static analysis only unless an isolated detonation VM is explicitly approved", "Samples are hashed, classified SECURITY_SENSITIVE and never leave the lab store", "Preserve evidence and recovery before destructive diagnostics"],
  [("NIST SP 800-86 Guide to Integrating Forensic Techniques", "NIST", "STANDARD", "public domain (US Government work)", "ASSUMED"), ("CyberDefenders labs", "CyberDefenders", "LAB", "free tier; terms to verify", "ASSUMED")],
  [("ir.scoping-triage", "Incident scoping, severity and triage plan", "Scope an incident, assess severity, preserve evidence and draft a triage plan.", (1,3), [], 240,
    [("Contain first, always", "Containment can destroy evidence and tip off actors; decide per severity"), ("Severity equals number of hosts", "Impact and data sensitivity drive severity")], "IR triage report"),
   ("dfir.host-artifacts", "Host artifacts and event timeline", "Analyze host artifacts, construct a timeline and explain chain-of-custody assumptions.", (2,4), ["ir.scoping-triage"], 180,
    [("File timestamps are reliable", "Timestomping and copy semantics alter them"), ("Deleted files are gone", "Journals and slack space retain them")], "host forensic timeline"),
   ("dfir.network-evidence", "Network forensic evidence", "Analyze suspicious captures, correlate network and host evidence and identify gaps.", (2,4), ["net.pcap-analysis"], 180,
    [("Beaconing has a fixed interval", "Jitter is common"), ("TLS hides everything", "JA3/JA4, SNI and sizes remain")], "network forensic report"),
   ("malware.static-triage", "Static malware triage", "Inspect a safe static fixture, record characteristics and distinguish behavior from inference.", (2,4), ["dfir.host-artifacts"], 180,
    [("Strings reveal behavior", "They suggest capability, not execution"), ("Packed means malicious", "Packing is common in legitimate software")], "malware triage"),
   ("dfir.chain-of-custody", "Chain of custody and evidence integrity", "Hash, document and preserve evidence with custody records.", (2,3), ["ir.scoping-triage"], 240,
    [("Hashes prove authenticity", "They prove integrity since hashing, not origin"), ("Screenshots are evidence", "Only with provenance and hashes")], "custody record review"),
   ("hunt.approved-telemetry", "Hunt execution on approved telemetry", "Run a defined hunt on approved telemetry and record detections.", (3,4), ["hunt.hypothesis"], 180,
    [("Hunts need full data", "Scoped data with declared gaps is valid"), ("Automation replaces hunting", "Hypotheses need analysts")], "hunt report"),
   ("ir.recovery-recommendation", "Recovery recommendation", "Recommend recovery steps with evidence preservation and verification.", (3,5), ["dfir.host-artifacts", "dfir.network-evidence"], 180,
    [("Reimage solves it", "Root cause and persistence elsewhere remain"), ("Recovery ends the incident", "Lessons and detections close it")], "recovery plan"),
   ("dfir.blind-incident", "Blind incident dataset triage", "Triage a blind dataset, reconstruct the timeline and recommend recovery.", (3,5), ["malware.static-triage", "ir.recovery-recommendation"], 180,
    [("Speed beats accuracy", "Wrong containment costs more"), ("Timeline gaps are failures", "Declared gaps are findings")], "week-8 blind assessment")]),
 ("llm-rag-security", "LLM applications, RAG, prompt injection and model evaluation", "M10", "Week 9", "CONTAINER",
  "Prompt/response metadata logs, retrieval provenance, schema validation failures, tool-call audit; injection attempts become detection fixtures.",
  ["Only local or explicitly approved models; no production API keys in the lab", "Injection fixtures are data, never instructions for the mentor itself", "No personal data in evaluation sets"],
  [("OWASP Top 10 for LLM Applications 2026", "OWASP GenAI Security Project", "STANDARD", "CC BY-SA 4.0 (verify)", "ASSUMED"), ("NIST AI RMF and Generative AI Profile", "NIST", "STANDARD", "public domain", "ASSUMED"),
   ("MITRE ATLAS", "MITRE", "STANDARD", "free with attribution", "ASSUMED")],
  [("llm.app-mapping", "LLM application mapping and threat model", "Map an LLM application, identify data/tool boundaries and threat-model it.", (1,3), [], 180,
    [("The model is the attack surface", "The bridge between text and tools is"), ("System prompts are confidential", "Assume extraction")], "LLM threat model"),
   ("rag.ingestion-provenance", "RAG ingestion, retrieval and source trust", "Explain ingestion/retrieval, analyze source trust and test provenance controls.", (2,4), ["llm.app-mapping"], 180,
    [("Retrieved text is data the model will treat as data", "Models follow instructions in retrieved text"), ("Vector similarity implies relevance", "Adversarial chunks rank high")], "RAG review"),
   ("llm.prompt-injection", "Prompt-injection entry classification", "Classify injection entry points, run controlled fixtures and test authorization boundaries.", (2,4), ["rag.ingestion-provenance"], 180,
    [("Input filtering stops injection", "Encodings and indirect channels bypass"), ("A refusal is a security control", "Only code-enforced authorization is")], "injection evaluation"),
   ("llm.output-schema-tools", "Output schema failures and tool safety", "Analyze schema failures, examine unsafe tools and define typed enforcement.", (3,4), ["llm.prompt-injection"], 180,
    [("JSON mode guarantees valid output", "Validate against the schema every time"), ("Read-only tools are safe", "They exfiltrate")], "tool safety matrix"),
   ("llm.independent-eval", "Independent evaluation design", "Design independent eval cases, separate model from application security and report limits.", (3,5), ["llm.output-schema-tools"], 240,
    [("Benchmarks transfer to your app", "Context and tools change behavior"), ("Self-evaluation by the model is evidence", "Independent rubrics are required")], "LLM eval report"),
   ("llm.trust-failure-diagnosis", "Trust failure diagnosis", "Diagnose a controlled trust failure in an AI application and verify mitigation.", (3,5), ["llm.independent-eval"], 180,
    [("The fix is a better prompt", "Fix the authorization path"), ("One mitigation closes the class", "Defense in depth is needed")], "week-9 blind evaluation"),
   ("llm.data-leakage", "Data leakage and privacy exposure", "Assess privacy exposure through prompts, logs and retrieval.", (2,4), ["rag.ingestion-provenance"], 240,
    [("Logs are internal so they are safe", "Prompts contain secrets; logs leak"), ("Anonymization is irreversible", "Re-identification via context")], "privacy exposure review"),
   ("llm.eval-leakage", "Benchmark leakage detection", "Detect training/retrieval contamination of evaluation sets.", (3,5), ["llm.independent-eval"], 240,
    [("Held-out means unseen", "Public write-ups are in pretraining"), ("High scores prove capability", "Contamination inflates")], "evaluation protocol")]),
 ("agentic-mcp-supplychain", "Agentic AI, MCP authorization, tool misuse and AI supply chain", "M16", "Weeks 10-11", "CONTAINER",
  "Tool-call audit trails, approval decision logs, token audience checks, package/model provenance records; excessive agency shows as unapproved actions.",
  ["Agents run only in the isolated lab with mock tools", "No real tokens; audience/resource binding tested with fixtures", "Supply-chain experiments use quarantined packages, never installed on the host"],
  [("OWASP Top 10 for Agentic Applications 2026", "OWASP GenAI Security Project", "STANDARD", "CC BY-SA 4.0 (verify)", "ASSUMED"), ("Model Context Protocol authorization specification 2025-11-25", "MCP project", "STANDARD", "MIT (verify)", "ASSUMED"),
   ("Open Agent Skills specification", "agentskills.io", "STANDARD", "open specification", "VERIFIED")],
  [("agent.workflow-authority", "Agent workflow and authority crossings", "Map agent workflows, identify authority crossings and define bounded execution.", (2,3), ["llm.app-mapping"], 180,
    [("Agents are just chat with tools", "Each tool is an authority grant"), ("Sandboxing the model sandboxes the agent", "Tools run outside")], "agent boundary diagram"),
   ("agent.identity-scopes", "Identities, scopes and excessive agency", "Examine identities and scopes, identify excessive agency and design backend approvals.", (2,4), ["agent.workflow-authority"], 180,
    [("The agent acts as the user", "It needs its own narrower identity"), ("Approval prompts in the UI are a boundary", "Backend-enforced approvals are")], "authorization specification"),
   ("mcp.transport-auth", "MCP transport, actors and token audience", "Analyze MCP transport and actors, token audience/exfiltration risk, define regression cases.", (3,4), ["agent.identity-scopes"], 180,
    [("A valid token is valid for any server", "Resource indicators bind audience"), ("Local servers need no auth", "Local sockets are reachable by malware")], "MCP security report"),
   ("agent.memory-integrity", "Persistent memory trust and poisoning", "Explain memory trust, evaluate poisoning fixtures and design correction/provenance.", (3,4), ["mcp.transport-auth"], 180,
    [("Memory is the agent's own notes", "It is untrusted input on the next run"), ("Deleting poisoned entries fixes it", "Derived state must be invalidated")], "memory integrity plan"),
   ("agent.redblue-experiment", "Bounded red/blue experiment", "Define a bounded experiment, observe behavior and verify defensive controls.", (3,5), ["agent.memory-integrity"], 180,
    [("Red teaming the model suffices", "Test the app with tools"), ("One run proves safety", "Variance requires repeated runs")], "agent evaluation"),
   ("ai.threat-model-redteam", "AI asset threat modeling and adversarial fixtures", "Identify AI assets/threats, prioritize failure modes, build adversarial fixtures and report uncertainty.", (3,5), ["agent.redblue-experiment"], 180,
    [("Jailbreaks are the main threat", "Data and tool authority are"), ("A passing suite proves no attack exists", "It bounds known classes")], "AI threat model and red-team fixture set"),
   ("ai.supply-chain", "Model and dependency provenance", "Trace model/dependency provenance, assess licensing/integrity and define acquisition controls.", (3,5), ["agent.identity-scopes"], 240,
    [("Downloaded weights are what the card says", "Verify checksums and signatures"), ("Permissive licenses allow everything", "Derivative and redistribution terms vary")], "supply-chain review"),
   ("ai.data-governance-eval", "Data poisoning, privacy and evaluation protocol", "Evaluate poisoning and privacy exposure, define governance controls, metrics and regression criteria.", (3,5), ["ai.supply-chain", "llm.eval-leakage"], 240,
    [("More training data dilutes poison", "Targeted poison survives"), ("Metrics are objective", "Leakage and selection bias distort")], "data security report and evaluation protocol")]),
]

def comp_json(pack_id, c):
    cid, name, desc, band, prereqs, validity, miscs, focus = c
    return {
        "id": cid, "name": name, "description": desc,
        "dimensions": DIMS,
        "difficulty_band": {"min": band[0], "max": band[1]},
        "prerequisites": prereqs,
        "validity_window_days": validity,
        "nice_refs": [
            {"kind": "KNOWLEDGE", "id": "K-UNPINNED", "confidence": "ASSUMED"},
            {"kind": "SKILL", "id": "S-UNPINNED", "confidence": "ASSUMED"},
        ],
        "misconceptions": [
            {"id": f"{cid}.m{i+1}", "statement": m[0], "counterexample": m[1]} for i, m in enumerate(miscs)
        ],
        "lab_fixtures": [
            {"id": f"{cid}.lab1", "description": f"Isolated fixture for: {desc}", "isolation": PACK_ISOLATION[pack_id],
             "data_license_note": "synthetic or lab-generated data; external datasets listed in the pack sources with their own license notes"}
        ],
        "assessments": [
            {"id": f"{cid}.blind", "kind": "BLIND_PRACTICAL", "description": f"Blind practical: {focus}", "rubric_focus": RUBRIC, "difficulty": band[1]},
            {"id": f"{cid}.transfer", "kind": "TRANSFER_CHECK", "description": f"Unseen-context transfer check for {name.lower()} with changed surface features", "rubric_focus": ["independent_execution", "root_cause_reasoning", "remediation_verification"], "difficulty": max(band[0], min(band[1], band[0] + 1))},
            {"id": f"{cid}.teachback", "kind": "TEACHBACK", "description": f"Teach-back of the mechanism behind {name.lower()} with misconception probes", "rubric_focus": ["root_cause_reasoning", "evidence_documentation"], "difficulty": band[0]},
        ],
        "detection_defense_visibility": PACK_VISIBILITY[pack_id],
        "safety_boundaries": PACK_SAFETY[pack_id],
    }

PACK_ISOLATION, PACK_VISIBILITY, PACK_SAFETY = {}, {}, {}

def qualify_prerequisites() -> None:
    """Cross-pack prerequisites must be written as `pack_id:competency_id` (validator rule)."""
    owner = {c[0]: pack_id for pack_id, *_rest, comps in [(p[0], p[-1]) for p in PACKS] for c in comps}
    for p in PACKS:
        pack_id, comps = p[0], p[-1]
        for c in comps:
            c[4][:] = [pre if pre in {x[0] for x in comps} or ':' in pre else f"{owner[pre]}:{pre}" for pre in c[4]]

def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    qualify_prerequisites()
    for pack_id, title, owner, weeks, isolation, visibility, safety, sources, comps in PACKS:
        PACK_ISOLATION[pack_id], PACK_VISIBILITY[pack_id], PACK_SAFETY[pack_id] = isolation, visibility, safety
        doc = {
            "pack_id": pack_id, "title": title, "version": "0.1.0", "status": "PROPOSED",
            "nice_framework_version": "2.2.0",
            "competencies": [comp_json(pack_id, c) for c in comps],
            "sources": [{"title": t, "organisation": o, "kind": k, "license_note": l, "verification": v} for (t, o, k, l, v) in sources],
            "maintenance_policy": {"review_interval_days": 90, "owner_module": owner},
        }
        d = OUT / pack_id
        d.mkdir(parents=True, exist_ok=True)
        (d / "pack.json").write_text(json.dumps(doc, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        rows = "\n".join(f"| `{c[0]}` | {c[1]} | {c[3][0]}–{c[3][1]} | {', '.join(c[4]) or '—'} | {c[5]} |" for c in comps)
        srcs = "\n".join(f"| {t} | {o} | {k} | {l} | {v} |" for (t, o, k, l, v) in sources)
        (d / "README.md").write_text(f"""# Knowledge pack: {title}

**Status:** PROPOSED (v0.1.0) · **Owner module:** {owner} · **Seed curriculum weeks:** {weeks} · **Review interval:** 90 days

## Purpose

Competency map, misconceptions, lab fixture descriptors, assessment descriptors and source register for this domain. Generated from `scripts/gen_competency_packs.py`; edit the table there, not this file.

## Competency map

| Id | Name | Difficulty band | Prerequisites | Validity (days) |
|---|---|---|---|---|
{rows}

Prerequisites use bare ids inside this pack and `pack_id:competency_id` across packs. Every competency carries all four learning dimensions (RECALL, UNDERSTANDING, APPLICATION, TRANSFER), two misconceptions with counterexamples, one isolated lab fixture ({isolation}), a blind practical, a transfer check and a teach-back assessment.

## NICE Framework mapping

`nice_framework_version` is pinned to the NICE Framework Components v2.2.0 release (28 April 2026). TKS ids are **not** pinned yet: every `nice_refs` entry is an `ASSUMED` placeholder (`K-UNPINNED`, `S-UNPINNED`). Pin exact ids from the v2.2.0 release artifact before this pack leaves PROPOSED.

## Detection and defense visibility

{visibility}

## Safety boundaries

{chr(10).join('- ' + s for s in safety)}

## Source register

| Title | Organisation | Kind | License note | Verification |
|---|---|---|---|---|
{srcs}

Sources marked ASSUMED must be verified (current terms, redistribution/embedding rights) during curriculum reconciliation (`docs/curriculum/provenance-and-reconciliation.md`).
""", encoding="utf-8")
    print(f"generated {len(PACKS)} packs under {OUT}")
    return 0

if __name__ == "__main__":
    sys.exit(main())
