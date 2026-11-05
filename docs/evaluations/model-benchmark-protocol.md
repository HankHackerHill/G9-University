# Local model and runtime benchmark protocol (Stage 06 gate)

Corrections preserved: no obsolete models are pre-selected (C07); no throughput targets are asserted before measurement (C06). This protocol produces the numbers; it contains none.

## Candidate registry (before any download)

For each candidate record in `model_registry`: base architecture, weights source (publisher, URL, commit/hash), license class and redistribution terms, quantization, SHA-256 of the artifact, context length, tokenizer/chat template id, runtime and version, evaluation conditions. Download requires an operator-approved `Connect` operation (network is off by default); load requires `Capability::ModelLoad` and a checksum match.

Size class: 7B–14B quantized candidates are the **benchmark range** for a 16 GB VRAM budget; smaller models and CPU offload are fallbacks. VRAM must accommodate weights + KV cache at the target context length; record the measured residency, not a formula.

## Runtime adapters to compare

| Adapter | Why | Check |
|---|---|---|
| llama.cpp-compatible | quantized local inference; grammar/JSON-schema constrained decoding | GPU build flags, structured output support, cancellation behaviour, offline operation |
| Ollama-compatible | model management convenience | external service lifecycle, disk usage, telemetry/network behaviour (must be fully offline), security of the local socket |
| vLLM-class | only if concurrency/throughput justify the footprint | out of scope unless a measured need appears |

## Mandatory measurements (per candidate × adapter × quantization)

| Metric | Method | Fixture |
|---|---|---|
| Time to first token (TTFT) | median and p95 over 30 runs, warm; plus cold start | 512-token prompt |
| Prompt processing throughput | tokens/s at 512, 2k, 8k context | synthetic + real lesson context |
| Output throughput | tokens/s at 128/512/1024 output | same |
| Latency p50/p95 by output size | 128/512/1024 | same |
| VRAM / RAM residency | runtime-reported + `nvidia-smi` sampling | at max context |
| Cold start | process start → first token | 5 runs |
| Crash / recovery | kill runtime mid-generation; app must return `RESOURCE_UNAVAILABLE` and recover without data change | adapter test |
| Cancellation | cancel at 50 % output; resources released within 2 s; late response discarded | adapter test |
| Structured-output validity | % of responses valid against the skill schema, with and without constrained decoding | 200 prompts across 10 skills |
| Context truncation behaviour | detection when context exceeds window; must be reported, never silent | assembler test |
| Knowledge / teaching correctness | known-answer technical fixtures (100 items across the 8 packs), expert-rubric scoring of 20 lesson generations | `data/fixtures/evals/` |
| Security regressions | adversarial suite ST01–ST12 with the model in the loop | `tests/adversarial/` |
| Energy / thermal | power draw sampling during a 10-minute mixed workload | host tools |

Quality and privacy outrank tokens/s. Every run records host, driver, runtime version, model hash, quantization, context, date.

## Decision rule

A candidate is eligible only if: structured validity ≥ 98 % with constrained decoding; zero security-regression failures; correctness within the pre-registered acceptance band for its role; VRAM residency leaves headroom for the UI and OS at the chosen context; license permits local use for this purpose. The operator selects among eligible candidates; the selection, evidence and rollback candidate are recorded in `model_evals` and an ADR.
