# Skill registry (37 logical skills)

Generated from the packages under `.agents/skills/` (source of truth: `scripts/gen_skill_packages.py`; validator: `scripts/check_skill_packages.py`). Package hash = SHA-256 over sorted relative paths and file bytes (first 16 hex); `g9_skills::Registry::verify_hashes` refuses mismatches at runtime. All packages are PROPOSED: runtime wiring and model evaluation are Stage 06/07.

| Code | Name | Family | Owner | Modes | Domains | Requested capabilities | Budget (tokens/seconds/tool calls) | Depends on | Status | Package hash |
|---|---|---|---|---|---|---|---|---|---|---|
| E01 | g9-spec-assurance | engineering | M01 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/120s/0 | — | PROPOSED | e6e02d0138932a3b |
| E02 | g9-skill-engineer | engineering | M11 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/120s/0 | g9-spec-assurance | PROPOSED | 2926e1f44df74ac1 |
| E03 | g9-curriculum-compiler | engineering | M02 | MAINTENANCE,REVIEW | SYSTEM,LEARNING | FILESYSTEM_READ | 8000/180s/0 | — | PROPOSED | 994d5be0ac24c469 |
| E04 | g9-domain-architect | engineering | M01 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/120s/0 | g9-spec-assurance | PROPOSED | 237ff3aabbd3844f |
| E05 | g9-ui-accessibility | engineering | M01 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/120s/0 | — | PROPOSED | 89a263070fb11178 |
| E06 | g9-security-boundaries | engineering | M16 | MAINTENANCE,REVIEW | SYSTEM,SECURITY_SENSITIVE | FILESYSTEM_READ | 8000/180s/0 | g9-domain-architect | PROPOSED | 14d0f5d5b045b8da |
| E07 | g9-data-integrity | engineering | M17 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/180s/0 | — | PROPOSED | abe1e3dd777fe90c |
| E08 | g9-scheduling-engine | engineering | M03 | MAINTENANCE,REVIEW,EXECUTIVE | SYSTEM,LEARNING | FILESYSTEM_READ | 8000/120s/0 | — | PROPOSED | fb0111575b5e07bc |
| E09 | g9-learning-data-model | engineering | M06 | MAINTENANCE,REVIEW | SYSTEM,LEARNING | FILESYSTEM_READ | 8000/120s/0 | — | PROPOSED | c5bf0a198daac024 |
| E10 | g9-model-runtime-engineer | engineering | M10 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/180s/0 | — | PROPOSED | 6c26b7e6657e21eb |
| E11 | g9-knowledge-engineer | engineering | M15 | MAINTENANCE,REVIEW,RESEARCH | SYSTEM,LEARNING | FILESYSTEM_READ | 8000/180s/0 | — | PROPOSED | 2d64fcda3ab6f00e |
| E12 | g9-mentor-evaluation | engineering | M13 | MAINTENANCE,REVIEW | SYSTEM,LEARNING | FILESYSTEM_READ | 8000/180s/0 | — | PROPOSED | 214af5b6860ae8bb |
| E13 | g9-adversarial-assurance | engineering | M16 | MAINTENANCE,REVIEW | SYSTEM,SECURITY_SENSITIVE | FILESYSTEM_READ | 8000/180s/0 | g9-security-boundaries | PROPOSED | 198b560901bd78b2 |
| E14 | g9-release-recovery | engineering | M17 | MAINTENANCE,REVIEW | SYSTEM | FILESYSTEM_READ | 8000/180s/0 | g9-data-integrity | PROPOSED | 622f57270f80309c |
| K01 | g9-source-curation | knowledge | M15 | RESEARCH,REVIEW | LEARNING,SYSTEM | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | a726295c8941102d |
| K02 | g9-memory-reconciliation | knowledge | M09 | REVIEW,STUDY | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | g9-competency-evidence | PROPOSED | 7120f68e5de2ca16 |
| K03 | g9-ct9-knowledge-export | knowledge | M14 | REVIEW,EXECUTIVE | LEARNING,BUSINESS | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | g9-source-curation | PROPOSED | d4040d54426049d7 |
| L01 | g9-misconception-analysis | learning | M09 | STUDY,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | g9-competency-evidence | PROPOSED | ec19760269fc98f3 |
| L02 | g9-competency-evidence | learning | M06 | STUDY,LAB,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | fc4e481a84ce5dac |
| L03 | g9-retrieval-and-transfer | learning | M08 | STUDY,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 4000/60s/0 | — | PROPOSED | 03907becda19167d |
| L04 | g9-intervention-designer | learning | M12 | STUDY,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | g9-prerequisite-diagnosis | PROPOSED | c7c6052ec32e6b18 |
| L05 | g9-learning-experiment | learning | M13 | REVIEW,EXECUTIVE | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 8000/120s/0 | g9-competency-evidence | PROPOSED | 67d41f136fe3ba66 |
| L06 | g9-confidence-calibration | learning | M13 | REVIEW,STUDY | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 4000/60s/0 | — | PROPOSED | 7c39fdfb9c7e9bc6 |
| P01 | g9-adaptive-agenda | performance | M03 | STUDY,EXECUTIVE,REVIEW | LEARNING,BUSINESS | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | bfe99943b26e623c |
| P02 | g9-performance-review | performance | M13 | REVIEW,EXECUTIVE | LEARNING,BUSINESS | SKILL_INVOKE,MODEL_INFERENCE | 8000/120s/0 | g9-competency-evidence | PROPOSED | 9af5e86c83a6074e |
| P03 | g9-goal-decomposition | performance | M14 | EXECUTIVE | BUSINESS | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | c4633856316c717a |
| P04 | g9-execution-diagnosis | performance | M14 | EXECUTIVE,REVIEW | BUSINESS | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | g9-goal-decomposition | PROPOSED | e2168c6e4b0dddf5 |
| P05 | g9-decision-review | performance | M14 | EXECUTIVE,REVIEW | BUSINESS | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | cf4719d7d0556270 |
| P06 | g9-reflection-coaching | performance | M14 | REVIEW | PERSONAL | SKILL_INVOKE,MODEL_INFERENCE | 3000/45s/0 | — | PROPOSED | 803f4cde95f95202 |
| T01 | g9-session-director | teaching | M12 | STUDY,LAB | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 4000/60s/0 | — | PROPOSED | 49409c465d6143a3 |
| T02 | g9-prerequisite-diagnosis | teaching | M12 | STUDY,LAB,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | g9-competency-evidence | PROPOSED | af0100356c1204ab |
| T03 | g9-adaptive-instruction | teaching | M12 | STUDY,LAB | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 8000/120s/0 | g9-session-director | PROPOSED | 8dc57092fdff253e |
| T04 | g9-socratic-coaching | teaching | M12 | STUDY,LAB | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 4000/60s/0 | — | PROPOSED | f080f6646f64395d |
| T05 | g9-laboratory-coach | teaching | M12 | LAB | LEARNING,SECURITY_SENSITIVE | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | 8e86d7efd3e89781 |
| T06 | g9-progressive-hints | teaching | M12 | STUDY,LAB | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 3000/45s/0 | g9-socratic-coaching | PROPOSED | 65ff60ba1e7e6250 |
| T07 | g9-teachback-review | teaching | M12 | STUDY,LAB,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 6000/90s/0 | — | PROPOSED | b3c711b64d4a7107 |
| T08 | g9-independent-examiner | teaching | M07 | EXAM,REVIEW | LEARNING | SKILL_INVOKE,MODEL_INFERENCE | 8000/180s/0 | — | PROPOSED | 86532f1b1015b379 |

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
