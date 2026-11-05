# Security

## Model

The primary attack surface is the bridge between untrusted text and privileged host operations, not the language model alone. Untrusted surfaces: Markdown/HTML documents, PDFs, logs, screenshots, URLs, repository files, downloaded skill packages, model outputs, local notes, chat and knowledge snippets, exam submissions. Protected resources: host filesystem, executable tools, credentials, business/personal history, assessment database, curriculum versions, audit metadata, scheduling commitments, G9 OS controls.

Full threat model, trust boundaries, data flows and adversarial test catalogue: `docs/threat-model/`.

## Mandatory defenses (implemented or gated; see the stage report for status)

| Control | Where |
|---|---|
| Deny-by-default authorization, independent of SKILL.md | `crates/g9-policy` |
| Approval binding to exact operation hash, revision and expiry; single use; constant-time MAC check | `g9-policy::approvals`, `binding` |
| No shell, network or arbitrary filesystem capability in the mentor runtime; `cargo-deny` bans HTTP stacks | `deny.toml`, policy rules |
| Tauri capability minimization (`core:default` only), strict CSP, no remote origins | `apps/desktop/src-tauri/capabilities`, `tauri.conf.json` |
| Untrusted content is data: context assembler refuses untrusted segments in instruction roles and all `Secret` content | `g9-ai-adapter::context` |
| Typed output validation (JSON Schema) before any proposal is surfaced | `g9-ai-adapter::validate`, `g9-skills` |
| Exam isolation: sealed answer keys never enter model context in EXAM mode; exposure logged | `g9-assessment::integrity` |
| Finalized grades, consumed approvals and audit rows immutable (state machines + SQL triggers) | `g9-domain::state`, `g9-storage` |
| Provenance and trust class on every record; model-sourced observations rejected | `g9-domain::trust`, `g9-competency` |
| Skill packages pinned by SHA-256; mismatches refused at load | `g9-skills::registry` |
| Model artifacts checksum-verified; load is operator-only | `g9-ai-adapter::registry` |
| Hash-chained, metadata-only audit records; raw prompts never persisted | `g9-storage::audit`, `g9-ai-adapter::record` |
| Consistent backups (Online Backup API / VACUUM INTO), integrity-checked and restore-tested | `g9-storage::backup` |
| Secret-free IPC errors (closed category set, redacted internals) | `g9-domain::envelope` |
| Sanitized Markdown rendering (allow-list) | `apps/desktop/src/lib/markdown.ts` |

## Residual risks (cannot be eliminated by prompts or policy)

System-level compromise, malicious native dependencies, device theft, compromised GPU/runtime libraries, model hallucination, undetected data poisoning, prompt injection inside approved-source content, statistical overfitting of personal recommendations. Mitigations: supply-chain scanning (`cargo-deny`, `cargo-audit`, `npm audit`), full-disk encryption on the host, encrypted backups, operator review of all proposals, pre-registered evaluation protocols.

## Process

The application runs as an ordinary unprivileged Linux user. Lab orchestration, if ever added, is a separately approved adapter targeting isolated VMs/containers, never the host, and never arbitrary targets. Report suspected vulnerabilities to the G9 operator through the repository issue tracker with the `security` label; do not include exploit payloads against third-party systems.
