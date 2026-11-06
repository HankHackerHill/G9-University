# Dependency and supply-chain policy (A12, R-013)

- Versions are pinned in `Cargo.toml`/`Cargo.lock` and `package.json`/`package-lock.json`; upgrades are reviewed change sets with changelog links.
- `cargo-deny` (`deny.toml`): license allow-list, advisories, yanked crates, banned HTTP stacks in runtime crates, registry restricted to crates.io.
- `cargo-audit` and `npm audit --audit-level=high` in CI (PROPOSED for cargo-audit until the runner is configured).
- SBOM: `cargo tree -e normal` and `npm ls --all --json` artifacts attached to every release; CycloneDX export via `cargo-cyclonedx` is PROPOSED.
- Skill packages: SHA-256 pinned in `docs/skills/registry.md`; the registry refuses mismatches; no scripts in packages are ever executed.
- Model artifacts: SHA-256 and license recorded in `model_registry`; load is operator-only.
- Releases: tagged, built reproducibly from a clean checkout on the CI image, artifacts hashed and signed (signing key management is an operator decision; PROPOSED).
- Rollback: application binaries can be rolled back only to versions whose schema version is compatible; otherwise rollback is restore-from-backup (C12).

## Recorded decisions and accepted exceptions (2026-11-06, platform audit PLT-01…PLT-18)

| Item | Decision | Expiry / review |
|---|---|---|
| `cargo-deny` targets | `[graph] targets = ["x86_64-unknown-linux-gnu"]` — Linux is the only shipping platform; Tauri's Android/iOS-only `reqwest`/`hyper` no longer trip the HTTP-stack ban (PLT-02) | revisit if ARM64 Linux is added |
| `MIT-0` | allowed (OSI-approved, no attribution); arrives transitively via `jsonschema → referencing → fluent-uri → borrow-or-share` (PLT-01) | — |
| `rs-fsrs` licence | pinned with `[[licenses.clarify]]` expression `MIT` and LICENSE XXH32 hash `0x260b1e30`; a changed licence text fails the gate (PLT-17) | on every rs-fsrs upgrade |
| `RUSTSEC-2024-0370` | ignored with reason: unmaintained build-time proc-macro via gtk-rs 0.18 in Tauri's Linux stack; no safe upgrade (PLT-08) | re-review at every Tauri minor |
| Workspace crates | `publish = false` for every member so versionless path dependencies are allowed (`allow-wildcard-paths`) (PLT-02) | — |
| vitest | pinned to `4.1.12` (critical GHSA-5xrq-8626-4rwp affected `>=4.0.0 <4.1.0`); never run `vitest --ui` on the G9 host (PLT-03) | — |
| `minimatch` GHSA-g3hv-75wg-m249 (moderate) | accepted: dev-only, reached through `typescript-eslint`'s estree; not shipped in the bundle; `npm audit --audit-level=high` passes | **expires 2026-12-31**: upgrade typescript-eslint when a release depends on `minimatch >= 11.0.1` |
| `@tauri-apps/api` | moved to `dependencies` because Vite bundles it into the shipped app; production SBOMs now include it (PLT-09) | — |
| SBOM | CI job `sbom` produces CycloneDX for Rust (`cargo-cyclonedx`, linux target) and npm (`@cyclonedx/cyclonedx-npm --omit dev`) and uploads them as artifacts (PLT-09) | system libraries (WebKitGTK, GTK3, libsoup3, glib) and model artefacts need separate component lists (Stage 14/06) |
| CI actions | pinned by commit SHA to Node-24 releases: checkout v7.0.1, setup-node v7.1.0, upload-artifact v7.0.2, rust-cache v2.9.3, cargo-deny-action v2.1.1, rust-toolchain v1 (PLT-16, SEC-18) | refresh quarterly |
| Tauri major | stay on Tauri 2.x (2.12.5) until 3.0 is stable **and** a migration ADR is accepted; `tauri 3.0.0-alpha` exists (PLT-18) | at 3.0 stable |
| MSRV | workspace `rust-version` set to the CI toolchain (1.97); the declared MSRV is the version CI builds (PLT-18) | with toolchain bumps |

## Model artefact and runtime controls (PLT-05, PLT-06)

- Every `model_registry` row records a `license_class`: `OSI_PERMISSIVE` (Apache-2.0, MIT — default eligible), `COMMUNITY_AUP` (e.g. Llama community licences, Gemma terms — operator review; ineligible for offensive-lab skills where the use policy forbids malicious-code generation), `NON_PRODUCTION` (research/non-commercial — personal evaluation only), with NOTICE/attribution text stored.
- GGUF and runtime binaries are native-code attack surface: record the minimum runtime build (llama.cpp ≥ b7824 per the platform audit; Ollama with `OLLAMA_NO_CLOUD=1` and the local socket only), run the runtime as a separate unprivileged process without network access, and accept only quantized artefacts whose provenance chain leads to the official upstream weights (checksums at each step).

## External training resources (PLT-07)

The free tiers and publication terms differ per resource (table in `docs/audit/platform-licensing-audit.md`). Until verified per resource, curriculum days may *reference* these platforms for individual study but K03 knowledge exports and CT9 business use must not include their content, solutions or datasets (PortSwigger forbids commercial use of content without a licence; HTB and CyberDefenders allow write-ups only for retired content; Splunk BOTS v3 data is CC0 but needs separately licensed Splunk software).
