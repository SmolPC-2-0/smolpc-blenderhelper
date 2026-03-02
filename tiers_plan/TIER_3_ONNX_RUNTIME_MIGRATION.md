# Tier 3: Replace Ollama With In-Process ONNX Runtime

**Priority:** MEDIUM-HIGH (after Tier 2 stabilizes)  
**Effort:** High (5-8 implementation sessions)  
**Impact:** Removes external Ollama dependency; single self-contained app runtime for inference.

---

## Goal

Replace this Tier 2 generation path:

```text
Frontend -> Tauri IPC -> Rust -> HTTP (11434) -> Ollama
```

with:

```text
Frontend -> Tauri IPC -> Rust ONNX Runtime (in-process)
```

while keeping Tier 2 RAG retrieval and scene bridge behavior intact.

---

## Why Tier 3 Is Hard

- You are replacing the inference engine, not just transport.
- You must manage model loading, tokenization, sampling, KV cache, and cancellation in Rust.
- Packaging model artifacts and ONNX Runtime binaries is non-trivial.
- Quality/performance tradeoff vs Ollama 7B model must be explicitly accepted.

---

## Source Reference To Reuse

Primary port source is `smolpc-codehelper`:

- `src-tauri/src/inference/*`
  - `generator.rs`
  - `kv_cache.rs`
  - `session.rs`
  - `tokenizer.rs`
  - `input_builder.rs`
  - `types.rs`
- `src-tauri/src/models/*`
  - `registry.rs`
  - `runtime_spec.rs`
  - `loader.rs`
- `src-tauri/src/commands/inference.rs`
  - `InferenceState`
  - `GenerationPermit`
  - `inference_generate`
  - `inference_cancel`

The core adaptation work is wiring this into BlenderHelper's prompt/RAG/scene workflow.

---

## Tier 3 Prerequisites

Must be true before starting:

1. Tier 2 commands are stable (`assistant_ask`, `assistant_analyze_scene`, cancel, scene cache, RAG retrieval).
2. Frontend already uses Tauri IPC, not Flask HTTP.
3. Flask and Python are no longer in the critical path.
4. You have decided acceptable model-quality tradeoff for educational Blender responses.

---

## Model Strategy Decision (Do This First)

Decide one of:

1. **Single model**: `Qwen2.5-Coder-1.5B` ONNX (best simplicity, lower quality risk).
2. **Dual model**: small default + optional larger downloadable ONNX model.

Recommended initial Tier 3 path:

- Start with the known working CodeHelper model stack (`Qwen2.5-Coder-1.5B` ONNX).
- Add model abstraction from day one so larger models are possible later.

---

## Step-by-Step Plan

## Step 1: Add ONNX Runtime + Tokenizer Dependencies

Update `src-tauri/Cargo.toml` with dependencies aligned to CodeHelper:

- `ort` with `load-dynamic`, `ndarray`
- `tokenizers`
- `ndarray`
- sampling deps:
  - `rand`
  - `rand_distr`
- keep Tier 1/2 async deps:
  - `tokio`
  - `futures-util`

Use the same versions as CodeHelper first, then tune only if needed.

---

## Step 2: Port Inference and Model Modules

Copy/adapt into BlenderHelper:

- `src-tauri/src/inference/`
- `src-tauri/src/models/`

Then integrate with BlenderHelper module graph in `src-tauri/src/main.rs`.

Adaptation items:

- path resolution for model files in BlenderHelper resources/app data
- logger and error type integration
- command registration style (BlenderHelper uses `main.rs` builder path)

---

## Step 3: Introduce ONNX Inference Commands

Create `src-tauri/src/commands/inference.rs` (or equivalent) with:

- `load_model(model_id)`
- `unload_model()`
- `list_models()`
- `inference_generate(system_prompt, user_prompt, channel)`
- `inference_cancel()`
- `is_generating()`

Keep GenerationPermit + AtomicBool patterns from CodeHelper for robust cancellation and cleanup.

---

## Step 4: Integrate Tier 2 Assistant Flow With ONNX

In Tier 2, `assistant_ask` and `assistant_analyze_scene` likely call Ollama.

Refactor to backend interface:

- `GenerationBackend::Ollama`
- `GenerationBackend::Onnx`

Then switch `assistant_*` to use the backend abstraction.

Benefits:

- safe staged rollout (feature flag flip instead of invasive rewrites).
- easier benchmark comparison between Ollama and ONNX in same code path.

---

## Step 5: Model Asset and Runtime Packaging

Define model asset layout (example):

- `resources/models/qwen2.5-coder-1.5b/`
  - `model.onnx`
  - `tokenizer.json`
  - `config.json`
  - `special_tokens_map.json`
- `resources/ort/` (if shipping runtime binaries explicitly)

Update `src-tauri/tauri.conf.json` resources accordingly.

Decide delivery:

1. Bundle model in installer (larger installer, easiest offline UX).
2. First-run download with checksum (smaller installer, needs one-time network).

Given offline-first requirement, bundling is preferred if artifact size is acceptable.

---

## Step 6: Frontend Integration and UX Signals

If Tier 1 store exists (`inferenceStore`), keep using it.

Add visible backend/model status:

- model loaded / loading / failed
- active backend: `onnx` or `ollama`
- token/s metric display (optional)

Update:

- `src/App.svelte`
- `src/lib/components/StatusIndicator.svelte`
- `src/lib/stores/settings.svelte.ts` (optional backend toggle for rollout)

---

## Step 7: Performance and Memory Tuning

Tune these ONNX generation parameters:

- max context window
- sink size (Attention Sinks)
- temperature/top-p/top-k defaults for educational output
- repetition penalty settings

Track metrics per request:

- first token latency
- total tokens
- tokens/sec
- cancellation responsiveness
- memory footprint

Use same benchmark prompts for Ollama and ONNX comparisons.

---

## Step 8: Remove Ollama Path (Final Cutover)

After quality and performance sign-off:

- remove Ollama command module usage for main chat flow
- remove Ollama startup assumptions from docs
- make ONNX backend default and only backend

Optional:

- keep Ollama fallback behind compile-time feature for one release cycle.

---

## Quality and Regression Validation

## Functional Tests

- Q&A answers remain UI-action oriented (no Python code leakage).
- Scene analysis returns actionable 3-5 suggestions.
- Cancellation still interrupts generation quickly.
- Multi-chat behavior unchanged.

## Inference Engine Tests

- model load/unload idempotence
- tokenizer edge cases (unicode, long prompts, empty prompt)
- KV cache growth + sink shifting correctness
- deterministic output checks at fixed seed/temperature

## Performance Gates (Example)

- warm start first token latency acceptable on target student hardware
- sustained token throughput acceptable for educational use
- memory stable during long sessions and repeated generations

---

## Risks and Mitigations

1. **Quality drop vs Ollama 7B**
   - Mitigation: tighten prompts, improve RAG context quality, optionally add larger model profile.
2. **Installer/bundle size growth**
   - Mitigation: optional model packs or first-run download with offline cache.
3. **Cross-platform runtime issues (ORT DLL/so loading)**
   - Mitigation: match CodeHelper packaging approach and test release artifacts early.
4. **Complex migration blast radius**
   - Mitigation: backend abstraction and feature flags to switch between Ollama and ONNX.

---

## Definition of Done

- Core chat/suggestions run entirely without Ollama installed.
- No HTTP call to `127.0.0.1:11434` in normal app operation.
- ONNX backend supports streaming and cancellation.
- Tier 2 scene bridge + RAG retrieval still works unchanged.
- Docs and install guides describe ONNX-first setup accurately.

---

## File-Level Change Checklist

Expected additions/modifications for Tier 3:

- Add:
  - `src-tauri/src/inference/*`
  - `src-tauri/src/models/*`
  - `src-tauri/src/commands/inference.rs`
- Modify:
  - `src-tauri/src/main.rs` (state + command registration)
  - `src-tauri/Cargo.toml` (ORT/tokenizer deps)
  - `src-tauri/tauri.conf.json` (model/runtime resources)
  - Tier 2 assistant command layer to call ONNX backend
  - frontend status/settings UI for backend/model state
- Remove or deprecate:
  - Tier 2 Ollama orchestration path after cutover
  - user-facing Ollama installation requirement in docs

