# Skill registry (37 logical skills)

Packages live under `.agents/skills/` (validator: `scripts/check_skill_packages.py`). Package hash = `g9_skills::package::package_sha256`: for every file in the package, sorted by relative path, SHA-256 absorbs the path, a `0x00` byte, the file length as 8 big-endian bytes and the file bytes; this table shows the first 16 hex. The same hashes (full 64 hex) are the compiled-in pin manifest `apps/desktop/src-tauri/skill-pins.json` that the desktop shell enforces through `Registry::load_pinned`. The hash column is generated, and CI-checked against both the packages and the pins, by `python3 -I scripts/skill_hashes.py --check`; after a reviewed package change run `--write` (a changed pin manifest is a change to the runtime trust anchor and needs the same review as the packages). All packages are PROPOSED: runtime wiring and model evaluation are Stage 06/07.

| Code | Name | Family | Owner | Modes | Domains | Requested capabilities | Budget (tokens/seconds/tool calls) | Depends on | Status | Package hash |
|---|---|---|---|---|---|---|---|---|---|---|
| E01 | g9-spec-assurance | engineering | M01 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/120s/0 | — | PROPOSED | af3df756c016be28 |
| E02 | g9-skill-engineer | engineering | M11 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/120s/0 | g9-spec-assurance | PROPOSED | 2d476c147b74a322 |
| E03 | g9-curriculum-compiler | engineering | M02 | MAINTENANCE,REVIEW | SYSTEM,LEARNING | FILESYSTEM_READ | 8000/180s/0 | — | PROPOSED | 5a378f259b113e52 |
| E04 | g9-domain-architect | engineering | M01 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/120s/0 | g9-spec-assurance | PROPOSED | d1a579f363126bbf |
| E05 | g9-ui-accessibility | engineering | M01 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/120s/0 | — | PROPOSED | f5eb9582db8f761c |
| E06 | g9-security-boundaries | engineering | M16 | MAINTENANCE,REVIEW | SYSTEM,SECURITY_SENSITIVE | FILESYSTEM_READ | 8000/180s/0 | g9-domain-architect | PROPOSED | 2617ba22378f4d54 |
| E07 | g9-data-integrity | engineering | M17 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/180s/0 | — | PROPOSED | 30a4d31f38102466 |
| E08 | g9-scheduling-engine | engineering | M03 | MAINTENANCE,REVIEW,EXECUTIVE | SYSTEM,LEARNING | FILESYSTEM_READ | 8000/120s/0 | — | PROPOSED | 31d445019f78f583 |
| E09 | g9-learning-data-model | engineering | M06 | MAINTENANCE,REVIEW | SYSTEM,LEARNING | FILESYSTEM_READ | 8000/120s/0 | — | PROPOSED | 48c9532cae8fd6c9 |
| E10 | g9-model-runtime-engineer | engineering | M10 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/180s/0 | — | PROPOSED | 99e505a14cac090f |
| E11 | g9-knowledge-engineer | engineering | M15 | MAINTENANCE,REVIEW,RESEARCH | SYSTEM,LEARNING | FILESYSTEM_READ | 8000/180s/0 | — | PROPOSED | 0abb186bc268d671 |
| E12 | g9-mentor-evaluation | engineering | M13 | MAINTENANCE,REVIEW | SYSTEM,LEARNING | FILESYSTEM_READ | 8000/180s/0 | — | PROPOSED | 3df8f7db59547e74 |
| E13 | g9-adversarial-assurance | engineering | M16 | MAINTENANCE,REVIEW | SYSTEM,SECURITY_SENSITIVE | FILESYSTEM_READ | 8000/180s/0 | g9-security-boundaries | PROPOSED | e95372dc4be91d7f |
| E14 | g9-release-recovery | engineering | M17 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/180s/0 | g9-data-integrity | PROPOSED | 39420a9f1491320c |
| K01 | g9-source-curation | knowledge | M15 | RESEARCH,REVIEW | LEARNING,SYSTEM | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | b6291008c260cda4 |
| K02 | g9-memory-reconciliation | knowledge | M09 | REVIEW,STUDY | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | g9-competency-evidence | PROPOSED | b68e37d75cd0a224 |
| K03 | g9-ct9-knowledge-export | knowledge | M14 | REVIEW,EXECUTIVE | LEARNING,BUSINESS | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | g9-source-curation | PROPOSED | 2133e9dd7b79f0c1 |
| L01 | g9-misconception-analysis | learning | M09 | STUDY,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | g9-competency-evidence | PROPOSED | 3e4f9011fca798a3 |
| L02 | g9-competency-evidence | learning | M06 | STUDY,LAB,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | aff1c7872a45a91f |
| L03 | g9-retrieval-and-transfer | learning | M08 | STUDY,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 4000/60s/0 | — | PROPOSED | c87d6270aebae31b |
| L04 | g9-intervention-designer | learning | M12 | STUDY,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | g9-prerequisite-diagnosis | PROPOSED | 7b631ca342dd0610 |
| L05 | g9-learning-experiment | learning | M13 | REVIEW,EXECUTIVE | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 8000/120s/0 | g9-competency-evidence | PROPOSED | d9426e632241f833 |
| L06 | g9-confidence-calibration | learning | M13 | REVIEW,STUDY | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 4000/60s/0 | — | PROPOSED | 0fe4399e910c1b4f |
| P01 | g9-adaptive-agenda | performance | M03 | STUDY,EXECUTIVE,REVIEW | LEARNING,BUSINESS | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | 95011d095c27ff02 |
| P02 | g9-performance-review | performance | M13 | REVIEW,EXECUTIVE | LEARNING,BUSINESS | SKILL_INVOKE,MODEL_INFERENCE | 8000/120s/0 | g9-competency-evidence | PROPOSED | cfc9c1bade2963da |
| P03 | g9-goal-decomposition | performance | M14 | EXECUTIVE | BUSINESS | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | 6f85c80b7832af9c |
| P04 | g9-execution-diagnosis | performance | M14 | EXECUTIVE,REVIEW | BUSINESS | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | g9-goal-decomposition | PROPOSED | a6340fe0df9a69c5 |
| P05 | g9-decision-review | performance | M14 | EXECUTIVE,REVIEW | BUSINESS | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | 9b1f03881bfc5bf2 |
| P06 | g9-reflection-coaching | performance | M14 | REVIEW | PERSONAL | SKILL_INVOKE,MODEL_INFERENCE | 3000/45s/0 | — | PROPOSED | 1713df56212d6da9 |
| T01 | g9-session-director | teaching | M12 | STUDY,LAB | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 4000/60s/0 | — | PROPOSED | 5f67b719b7466f49 |
| T02 | g9-prerequisite-diagnosis | teaching | M12 | STUDY,LAB,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | g9-competency-evidence | PROPOSED | b4474fab7544368c |
| T03 | g9-adaptive-instruction | teaching | M12 | STUDY,LAB | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 8000/120s/0 | g9-session-director | PROPOSED | 9c9e0f62b77a7c7f |
| T04 | g9-socratic-coaching | teaching | M12 | STUDY,LAB | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 4000/60s/0 | — | PROPOSED | e100098fe900df47 |
| T05 | g9-laboratory-coach | teaching | M12 | LAB | LEARNING,SECURITY_SENSITIVE | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | dd8cd001be4950da |
| T06 | g9-progressive-hints | teaching | M12 | STUDY,LAB | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 3000/45s/0 | g9-socratic-coaching | PROPOSED | 42e6e876e5d876cd |
| T07 | g9-teachback-review | teaching | M12 | STUDY,LAB,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | 81d5859e050f4265 |
| T08 | g9-independent-examiner | teaching | M07 | EXAM,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 8000/180s/0 | — | PROPOSED | 01a1836b404870fd |

## Dependency waves (implementation order)

| Wave | Skills | Rationale |
|---|---|---|
| 1 | E01, E02, E03, E04, E05, E06, E07, E08, E09, E10, E11, E12, E13, E14 | engineering skills: build-time workflows the operator runs during Stages 00–02 |
| 2 | T01, T02, L02 | deterministic brief, diagnosis and evidence aggregation (Stage 03–05) |
| 3 | T03, T04, T05, T06, T07, L01, L03, L04 | classroom skills and remediation (Stage 07–09) |
| 4 | T08, L05, L06, P01, P02 | examiner, experiments, calibration, agenda, performance review (Stage 04/10) |
| 5 | P03, P04, P05, P06, K01, K02, K03 | executive and knowledge skills (Stage 11–12) |

## Knowledge packs

| Pack | Title | Competencies | Seed weeks |
|---|---|---|---|
| `agentic-mcp-supplychain` | Agentic AI, MCP authorization, tool misuse and AI supply chain | 8 | Weeks 10-11 |
| `detection-soc-hunting` | Detection engineering, SOC and threat hunting | 8 | Week 7 |
| `dfir-forensics-malware` | DFIR, forensic evidence and malware triage | 8 | Week 8 |
| `linux-python-systems` | Linux, Python and systems engineering | 8 | Week 1 |
| `llm-rag-security` | LLM applications, RAG, prompt injection and model evaluation | 8 | Week 9 |
| `networking-protocols-recon` | Networking, protocol analysis and scoped reconnaissance | 8 | Week 2 |
| `web-api-appsec` | Web, API and application security | 8 | Weeks 3-4 |
| `windows-identity-ad` | Windows, identity, Active Directory and access controls | 8 | Weeks 5-6 |
