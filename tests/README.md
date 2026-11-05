# Cross-crate tests

| Directory | Purpose | Runner |
|---|---|---|
| `integration/` | gate tests spanning crates through `g9-app` (A01, A04, A06, A08, A10 end-to-end) | `cargo test -p g9-evaluation` and `tests/integration/*.rs` registered in `g9-evaluation` |
| `recovery/` | populated-data backup → verify → restore drills, crash injection | same |
| `adversarial/` | fixtures ST01–ST12 (malicious notes, forged approvals, tampered skill packages, XSS payloads) | `g9-evaluation` harness + UI vitest |
| `e2e/` | Playwright flows against the built UI with the mock transport (and Tauri on a prepared host) | `apps/desktop` (PROPOSED) |

Fixtures contain synthetic data only — no secrets, no personal data.
