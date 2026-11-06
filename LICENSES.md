# Licenses

## Project license (OPERATOR DECISION PENDING)

The workspace manifests currently declare `Apache-2.0` as a **PROPOSED** placeholder so that tooling (cargo-deny, SBOM) has a value. The operator must confirm or replace it before any release; until then the repository is private to G9 / Cyber Team 9.

## Third-party components (pinned 2026-11-05; VERIFIED from registry metadata)

| Component | Version | License | Use |
|---|---|---|---|
| Rust toolchain | 1.97.0 | MIT OR Apache-2.0 | build |
| tauri / tauri-build | 2.12.5 / 2.7.4 | Apache-2.0 OR MIT | desktop shell (not compiled in the sandbox) |
| rusqlite (+ bundled SQLite) | 0.41.1 | MIT (SQLite: public domain) | storage, Online Backup API |
| serde / serde_json | 1.x | MIT OR Apache-2.0 | serialization |
| thiserror | 2.x | MIT OR Apache-2.0 | errors |
| uuid | 1.29 | MIT OR Apache-2.0 | UUIDv7 ids |
| sha2 / hmac / hex / subtle | 0.10 / 0.12 / 0.4 / 2.6 | MIT OR Apache-2.0 (subtle: BSD-3-Clause) | hashing, approval binding |
| chrono / chrono-tz | 0.4.45 / 0.10.4 | MIT OR Apache-2.0 | civil time, DST |
| jsonschema | 0.60.2 (no default features) | MIT | structured-output validation |
| rs-fsrs | 1.2.1 (optional feature) | MIT | FSRS scheduling for declarative items |
| tracing | 0.1 | MIT | local diagnostics |
| proptest / tempfile / pretty_assertions | dev | MIT OR Apache-2.0 | tests |
| React / react-dom | 19.x | MIT | UI |
| react-router-dom | 7.x | MIT | routing |
| Vite / @vitejs/plugin-react | 7.x | MIT | build |
| TypeScript | 5.9 | Apache-2.0 | types |
| vitest / testing-library / jsdom | vitest 4.1.12; others pinned in package.json | MIT | tests (dev only) |
| eslint, typescript-eslint, eslint-plugin-jsx-a11y, eslint-plugin-react-hooks | 9.x | MIT | lint |
| marked / DOMPurify | pinned in package.json | MIT (plus the original Markdown BSD-style notice) / Apache-2.0 OR MPL-2.0 | sanitized Markdown |
| @tauri-apps/api | 2.12.4 (runtime dependency; bundled) | Apache-2.0 OR MIT | IPC transport (dynamic import) |

Not used by decision: **FullCalendar** (Standard is MIT, but v7 Premium is AGPLv3/commercial; an in-house accessible grid avoids the licensing boundary — see `docs/adr/`). **fsrs-rs** (BSD-3-Clause) is not used because it pulls a full ML training stack; `rs-fsrs` (scheduler only) is sufficient.

External training resources (HTB, PortSwigger Academy, CyberDefenders, OverTheWire, Splunk BOTS, LetsDefend) are **ASSUMED** free-tier/licensed for individual use until verified per resource during curriculum reconciliation (`docs/curriculum/`). The full SBOM is produced by `cargo-deny`/`cargo tree` and `npm ls` in CI.

## Transitive licences outside the direct list (cargo-deny, linux target, 2026-11-06)

- `MIT-0`: `borrow-or-share` (via `jsonschema → referencing → fluent-uri`). Allowed in `deny.toml`.
- `rs-fsrs` ships a LICENSE file without an SPDX field; `deny.toml` clarifies it as MIT and pins the text hash.
- The Tauri Linux shell links the distribution's WebKitGTK, GTK3, glib and libsoup3 (LGPL family) dynamically; no obligation arises while they come from the OS packages. Bundling them (AppImage) would create LGPL obligations, which is why the `.deb` against Ubuntu 26.04.1 is the packaging default (PLT-12).
- The shipped frontend bundle strips legal comments; a `THIRD_PARTY_NOTICES` file generated from the SBOM must accompany releases (PLT-10, Stage 14).

## Corrections to v5.1 licensing statements

- "Open weights (GPL/MIT)": wrong. Mainstream open-weight models are Apache-2.0, MIT or community licences with acceptable-use policies; none is GPL (PLT-06). See the model licence classes in `docs/release/dependency-policy.md`.
- "Impacket (LGPL2/MS-PL)": wrong. Impacket uses a modified Apache Software License 1.1 (PLT-15).
- "Wasmtime or llama.cpp for model": Wasmtime is a WebAssembly runtime, not an LLM inference runtime.
