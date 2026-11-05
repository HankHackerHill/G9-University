# Dependency and supply-chain policy (A12, R-013)

- Versions are pinned in `Cargo.toml`/`Cargo.lock` and `package.json`/`package-lock.json`; upgrades are reviewed change sets with changelog links.
- `cargo-deny` (`deny.toml`): license allow-list, advisories, yanked crates, banned HTTP stacks in runtime crates, registry restricted to crates.io.
- `cargo-audit` and `npm audit --audit-level=high` in CI (PROPOSED for cargo-audit until the runner is configured).
- SBOM: `cargo tree -e normal` and `npm ls --all --json` artifacts attached to every release; CycloneDX export via `cargo-cyclonedx` is PROPOSED.
- Skill packages: SHA-256 pinned in `docs/skills/registry.md`; the registry refuses mismatches; no scripts in packages are ever executed.
- Model artifacts: SHA-256 and license recorded in `model_registry`; load is operator-only.
- Releases: tagged, built reproducibly from a clean checkout on the CI image, artifacts hashed and signed (signing key management is an operator decision; PROPOSED).
- Rollback: application binaries can be rolled back only to versions whose schema version is compatible; otherwise rollback is restore-from-backup (C12).
