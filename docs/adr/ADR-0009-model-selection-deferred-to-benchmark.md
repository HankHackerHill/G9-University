# ADR-0009: No model pre-selected; candidates discovered and benchmarked on the actual host

**Status:** Accepted (2026-11-06)

## Context

v5.1 named Llama 2 7B/13B and Vicuna as baselines and asserted 1000 tokens < 300 ms (C06, C07, TEC-28, CON-28).

## Decision

`model_registry` records architecture, source, license class, quantization, checksum, context length, tokenizer template and runtime; selection follows `docs/evaluations/model-benchmark-protocol.md` (TTFT, throughput, p50/p95, VRAM, cold start, cancellation, structured validity, correctness, security regressions). Loading requires `Capability::ModelLoad` (operator-only) and a checksum match. The 7B–14B quantized range is a benchmark range for a 16 GB VRAM budget, not a choice.

## Alternatives considered

Hardcode a current favourite (rejected: obsolescence and license risk).

## Consequences

Runtime remains swappable (R-013).

## Verification

`checksum_mismatch_rejected`; registry rows are empty until Stage 06.
