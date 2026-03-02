# BlenderHelper -> SmolPC Shared Engine Onboarding Plan

Start date target: Monday, March 2, 2026
Audience: BlenderHelper developer and AI-assisted implementation sessions

## Summary

BlenderHelper should migrate its generation backend to route through the SmolPC shared inference engine contract, replacing the current in-process ONNX runtime and direct Ollama fallback with calls to the shared engine's `/engine/*` and `/v1/*` API surface.

The Rust backend already owns all orchestration (RAG retrieval, prompt construction, streaming, cancellation). The migration adds a third `GenerationBackend` variant that delegates generation to the shared engine over localhost HTTP instead of running inference in-process.

Safe migration order:

1. Add a Rust engine client module that talks to `/engine/*` and `/v1/*`.
2. Wire the engine client into the existing `GenerationBackend` dispatch as a new `SharedEngine` variant.
3. Extend frontend UI/state to surface shared-engine diagnostics (runtime engine, selection reason, queue state).
4. Update the Axum scene bridge `/health` response to include engine status for addon compatibility.
5. Validate against the shared-engine onboarding checklist.

This document is intended to be decision-complete and directly executable.

## Ground Truth (Current State)

1. **Backend is Rust, not Python.** The Tauri app boots without Python/Flask. Rust handles all orchestration: RAG retrieval (`src-tauri/src/rag/`), prompt construction (`src-tauri/src/prompts.rs`), generation dispatch, streaming, and cancellation.
2. **No Python process management.** `main.rs` does not start or stop a Python server. Flask (`rag_system/server.py`) is legacy build-time tooling only, not used at runtime.
3. **Frontend uses Tauri IPC, not HTTP.** All frontend API calls go through `invoke()` to Rust Tauri commands (`src/lib/utils/api.ts`). The Blender addon is the only HTTP consumer.
4. **Axum scene bridge on `127.0.0.1:5179`.** A Rust HTTP server (`src-tauri/src/scene_bridge.rs`) provides addon-compatible endpoints: `/health`, `/scene/update`, `/scene/current`, `/rag/retrieve`, `/ask`, `/scene_analysis`.
5. **Frontend is complete and compiling.** `src/lib/` contains 30+ files (stores, components, types, utils). No baseline recovery needed.
6. **Generation backend is already abstracted.** `state.rs` defines `GenerationBackend` enum with `Onnx` and `Ollama` variants. `execute_stream_generation()` in `commands/generation.rs` dispatches based on the active variant.
7. **Streaming and cancellation are implemented.** Token-by-token streaming via IPC `Channel<String>`, mid-generation cancel via `AtomicBool` (Tier 1, Feb 2026).
8. **ONNX model lifecycle exists.** `commands/inference.rs` provides `list_models`, `load_model`, `unload_model`, `set_generation_backend`, `get_generation_backend`.
9. **Shared engine already exists in `smolpc-codehelper`:**
   1. API contract: `docs/ENGINE_API.md`
   2. App onboarding checklist: `docs/APP_ONBOARDING_PLAYBOOK.md`
   3. Rust client: `crates/smolpc-engine-client`
10. **Shared engine model registry currently includes `qwen2.5-coder-1.5b`; BlenderHelper uses educational instruct prompting.**

## Goals

1. Add `SharedEngine` as a third `GenerationBackend` variant that calls the shared engine API.
2. Preserve educational prompting + RAG retrieval + scene-aware workflows (all owned by Rust, unchanged).
3. Expose shared-engine diagnostics in UI:
   1. active backend (onnx / ollama / shared-engine)
   2. runtime engine (e.g. `ort_cpu`, `ort_dml`)
   3. selection reason
   4. load/generation/cancel state
4. Keep addon compatibility and Axum bridge endpoint stability.
5. Produce clean handoff and reusable AI task cards.

## Non-Goals

1. Removing the existing ONNX or Ollama backends. All three backends coexist.
2. Reworking educational prompt logic (already working in `prompts.rs`).
3. Multi-model orchestration beyond one active model at a time.
4. Reintroducing Python/Flask into the runtime path.

## Architecture Decisions (Locked)

1. **Primary integration path:**
   1. Add a Rust engine client module (`src-tauri/src/engine_client.rs` or similar) that calls the shared engine HTTP API.
   2. Optionally vendor or depend on `smolpc-engine-client` from the codehelper repo.
2. **Backend dispatch:**
   1. Extend `GenerationBackend` enum: `Onnx | Ollama | SharedEngine`.
   2. `execute_stream_generation()` gains a `SharedEngine` match arm that streams tokens from the engine's `/v1/chat/completions` SSE response.
3. **Lifecycle:**
   1. Rust startup discovers shared engine availability via `/engine/health` + `/engine/meta`.
   2. If engine is reachable and protocol-compatible, default backend can be set to `SharedEngine`.
   3. Falls back to ONNX or Ollama if engine is not available.
4. **Compatibility:**
   1. Axum scene bridge endpoints remain stable.
   2. `/health` response gains engine diagnostics fields.
5. **Streaming first:**
   1. The existing streaming architecture (IPC channels, `AtomicBool` cancel) maps directly to engine SSE. Implement streaming from the start.
6. **Default model:**
   1. Use env-configurable `SHARED_ENGINE_MODEL_ID`.
   2. Default: `qwen2.5-coder-1.5b` until an educational ONNX model variant is registered.
7. **Auth:**
   1. Read bearer token from `%LOCALAPPDATA%/SmolPC/engine-runtime/engine-token.txt` on Windows.

## Existing Command Surface (Reference)

These Tauri commands and types already exist. The migration extends them; it does not replace them.

### Tauri IPC Commands (`src-tauri/src/commands/`)

| Command | Module | Purpose |
|---------|--------|---------|
| `assistant_stream_ask` | `generation.rs` | Streaming chat with RAG + scene context |
| `assistant_ask` | `assistant.rs` | Non-streaming chat |
| `assistant_analyze_scene` | `assistant.rs` | Scene analysis suggestions |
| `assistant_status` | `assistant.rs` | Health + backend + model + RAG status |
| `retrieve_rag_context` | `assistant.rs` | RAG context retrieval |
| `scene_current` | `scene.rs` | Current scene snapshot |
| `scene_update` | `scene.rs` | Update scene cache |
| `list_models` | `inference.rs` | List discovered ONNX models |
| `load_model` | `inference.rs` | Load ONNX model by ID |
| `unload_model` | `inference.rs` | Unload current ONNX model |
| `set_generation_backend` | `inference.rs` | Switch backend (currently: `onnx` / `ollama`) |
| `get_generation_backend` | `inference.rs` | Get current backend |
| `inference_generate` | `inference.rs` | Raw streaming generate (no RAG/prompt) |
| `inference_cancel` | `inference.rs` | Cancel active generation |
| `is_generating` | `generation.rs` | Check generation state |

### Axum Scene Bridge Endpoints (`src-tauri/src/scene_bridge.rs`)

| Endpoint | Method | Consumer |
|----------|--------|----------|
| `/health` | GET | Blender addon |
| `/scene/update` | POST | Blender addon |
| `/scene/current` | GET | Blender addon |
| `/rag/retrieve` | POST | Blender addon |
| `/ask` | POST | Blender addon |
| `/scene_analysis` | POST | Blender addon |
| `/test` | GET | Debug |

### Frontend API Layer (`src/lib/utils/api.ts`)

All calls use `invoke()` to Tauri IPC. Wrappers exist for: `checkHealth`, `askQuestion`, `retrieveRagContext`, `analyzeScene`, `getCurrentScene`, `listModels`, `loadModel`, `unloadModel`, `setGenerationBackend`, `getGenerationBackend`.

### Frontend Stores

| Store | File | Purpose |
|-------|------|---------|
| `inferenceStore` | `inference.svelte.ts` | Streaming generation + cancel + metrics |
| `chatsStore` | `chats.svelte.ts` | Chat history + messages |
| `ragStore` | `rag.svelte.ts` | RAG connection status polling |
| `blenderStore` | `blender.svelte.ts` | Scene state polling |
| `settingsStore` | `settings.svelte.ts` | User preferences |

## Public API and Interface Changes

### 1. Extend `GenerationBackend` enum

File: `src-tauri/src/state.rs`

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum GenerationBackend {
    Onnx,
    Ollama,
    SharedEngine,
}
```

Add `from_str` mapping for `"shared_engine"` or `"engine"`.

### 2. Add Engine Client Module

New file: `src-tauri/src/engine_client.rs` (or integrate `smolpc-engine-client` crate)

Responsibilities:

1. Resolve token path (`%LOCALAPPDATA%/SmolPC/engine-runtime/engine-token.txt`).
2. Build bearer auth header.
3. Provide typed functions:
   1. `health()` -> engine health check
   2. `meta()` -> protocol version + capabilities
   3. `status()` -> current model, backend, diagnostics
   4. `load_model(model_id)` -> load model on engine
   5. `cancel()` -> cancel active generation
   6. `list_models()` -> list engine-registered models
   7. `chat_completion_stream(messages, cancel_token, on_token)` -> SSE streaming
   8. `chat_completion(messages)` -> non-streaming
4. Validate protocol major version from `/engine/meta`.
5. Normalize errors into app-level error codes.

Configuration (env vars or compile-time defaults):

1. `ENGINE_BASE_URL` default `http://127.0.0.1:19432`
2. `SHARED_ENGINE_MODEL_ID` default `qwen2.5-coder-1.5b`
3. `ENGINE_CONNECT_TIMEOUT_SECS` default `5`
4. `ENGINE_REQUEST_TIMEOUT_SECS` default `120`

### 3. Extend `execute_stream_generation()`

File: `src-tauri/src/commands/generation.rs`

Add `GenerationBackend::SharedEngine` match arm:

```rust
GenerationBackend::SharedEngine => {
    engine_client::chat_completion_stream(
        &system_prompt,
        &user_prompt,
        Arc::clone(&cancelled),
        move |token| {
            if let Err(e) = token_channel.send(token) {
                log::warn!("Failed to send token via channel: {}", e);
            }
        },
    )
    .await
}
```

### 4. Extend `ask_internal()` and `analyze_scene_internal()`

File: `src-tauri/src/commands/assistant.rs`

Add `GenerationBackend::SharedEngine` arms to the existing match blocks:

```rust
GenerationBackend::SharedEngine => {
    engine_client::chat_completion(&system_prompt, &user_prompt).await?
}
```

### 5. Extend `assistant_status_internal()`

File: `src-tauri/src/commands/assistant.rs`

When backend is `SharedEngine`, query the engine for status and include diagnostics:

```rust
GenerationBackend::SharedEngine => {
    let engine_status = engine_client::status().await;
    // Map to AssistantStatusResponse fields
}
```

Add new fields to `AssistantStatusResponse`:

```rust
pub runtime_engine: Option<String>,      // e.g. "ort_cpu", "ort_dml"
pub selection_reason: Option<String>,     // e.g. "default_cpu"
pub engine_protocol: Option<String>,      // e.g. "1.0.0"
```

### 6. Extend `set_generation_backend` command

File: `src-tauri/src/commands/inference.rs`

Accept `"shared_engine"` / `"engine"` as valid backend values. Validate engine reachability before committing the switch.

### 7. Add engine-specific IPC commands

File: `src-tauri/src/commands/inference.rs` (or new `commands/engine.rs`)

New commands:

1. `engine_status()` -> engine diagnostics payload
2. `engine_load_model(model_id)` -> load model on shared engine
3. `engine_list_models()` -> list engine-registered models

### 8. Expand scene bridge `/health`

File: `src-tauri/src/scene_bridge.rs`

When backend is `SharedEngine`, include engine diagnostics in the health response:

```json
{
  "status": "ok",
  "connected": true,
  "backend": "shared_engine",
  "model": "qwen2.5-coder-1.5b",
  "generating": false,
  "rag_enabled": true,
  "rag_docs": 958,
  "engine": {
    "connected": true,
    "protocol_version": "1.0.0",
    "runtime_engine": "ort_cpu",
    "selection_reason": "default_cpu"
  }
}
```

### 9. Extend frontend API + types

File: `src/lib/utils/api.ts`

Add wrappers for new engine IPC commands:

```typescript
export async function engineStatus(): Promise<EngineStatusResponse> { ... }
export async function engineLoadModel(modelId: string): Promise<...> { ... }
export async function engineListModels(): Promise<...> { ... }
```

File: `src/lib/types/inference.ts`

Add types for engine diagnostics, extend `BackendSelectionResponse` to include `"shared_engine"`.

## Error Normalization (Locked)

Map engine outcomes to stable app-level errors:

1. HTTP `429` -> `ENGINE_QUEUE_FULL`
2. HTTP `504` -> `ENGINE_QUEUE_TIMEOUT`
3. generation cancel -> `GENERATION_CANCELLED` (already implemented as `GENERATION_CANCELLED` string)
4. protocol mismatch -> `ENGINE_PROTOCOL_MISMATCH`
5. auth/token failures -> `ENGINE_AUTH_FAILED`
6. engine unreachable -> `ENGINE_UNREACHABLE`
7. generic runtime errors -> `ENGINE_RUNTIME_ERROR`

## Implementation Plan

### Workstream A: Rust Engine Client

Goal: create a Rust module that can talk to the shared engine HTTP API.

New files:

1. `src-tauri/src/engine_client.rs` (or `src-tauri/src/engine/` module tree)

Tasks:

1. Implement token resolution from `%LOCALAPPDATA%/SmolPC/engine-runtime/engine-token.txt`.
2. Implement `health()`, `meta()`, `status()`, `list_models()`, `load_model()`, `cancel()`.
3. Implement `chat_completion(messages)` for non-streaming calls.
4. Implement `chat_completion_stream(messages, cancel_token, on_token)` that parses SSE lines and relays tokens through a callback, checking `AtomicBool` between chunks.
5. Validate protocol major version from `/engine/meta`.
6. Normalize HTTP errors into app-level error codes.
7. Add `reqwest` client with configurable timeouts.

Exit criteria:

1. Standalone test can call engine health/meta/status with auth token.
2. Streaming generation works with cancel support.

### Workstream B: Backend Dispatch Integration

Goal: wire the engine client into the existing generation dispatch.

Files to modify:

1. `src-tauri/src/state.rs` - add `SharedEngine` variant to `GenerationBackend`
2. `src-tauri/src/commands/generation.rs` - add `SharedEngine` arm to `execute_stream_generation()`
3. `src-tauri/src/commands/assistant.rs` - add `SharedEngine` arms to `ask_internal()`, `analyze_scene_internal()`, `assistant_status_internal()`
4. `src-tauri/src/commands/inference.rs` - accept `"shared_engine"` in `set_generation_backend()`, add engine-specific commands

Tasks:

1. Extend `GenerationBackend` enum with `SharedEngine`.
2. Add streaming dispatch arm that calls `engine_client::chat_completion_stream()`.
3. Add non-streaming dispatch arms for ask and scene analysis.
4. Add engine diagnostics to `AssistantStatusResponse`.
5. Add validation in `set_generation_backend()` to check engine reachability before committing.
6. Add `engine_status`, `engine_load_model`, `engine_list_models` IPC commands.

Exit criteria:

1. `assistant_stream_ask` produces streaming tokens via shared engine when backend is set to `SharedEngine`.
2. `assistant_status` returns engine diagnostics when backend is `SharedEngine`.
3. Backend toggle between `onnx`, `ollama`, and `shared_engine` works.

### Workstream C: Startup Auto-Detection

Goal: auto-detect shared engine availability at startup and select optimal backend.

File to modify:

1. `src-tauri/src/main.rs`

Tasks:

1. On startup, after ONNX model discovery, probe `engine_client::health()`.
2. If engine is reachable and protocol-compatible, set default backend to `SharedEngine`.
3. If engine is not available, fall through to existing ONNX -> Ollama fallback chain.
4. Log detected backend and reason.

Exit criteria:

1. App starts with `SharedEngine` backend when engine is running.
2. App falls back to ONNX/Ollama when engine is not available.

### Workstream D: UI Diagnostics

Goal: surface shared-engine diagnostics in the frontend.

Files to modify:

1. `src/lib/types/inference.ts` - extend types
2. `src/lib/utils/api.ts` - add engine command wrappers
3. `src/lib/stores/rag.svelte.ts` - surface engine fields from status polling
4. `src/lib/components/StatusIndicator.svelte` - show engine diagnostics

New files (optional):

1. `src/lib/components/EngineDiagnosticsPanel.svelte` - detailed engine status panel
2. `src/lib/components/QueueStateBanner.svelte` - queue full/timeout banner

Tasks:

1. Extend `HealthResponse` type with optional engine diagnostics fields.
2. Add IPC wrappers for `engine_status`, `engine_load_model`, `engine_list_models`.
3. Update `StatusIndicator` to show backend type and engine info when relevant.
4. Add queue-full / queue-timeout banner component.
5. Update `set_generation_backend` call to accept `"shared_engine"`.

UI behavior contract:

1. On startup, poll `assistant_status` which now includes engine diagnostics.
2. Auto-detect and display active backend.
3. Disable send while generation is in progress (already implemented).
4. Provide cancel action while generating (already implemented).
5. Always display: active model, active backend.
6. When backend is `SharedEngine`, also display: runtime engine, selection reason.
7. Show explicit queue full/timeout banners when engine returns 429/504.
8. App remains usable if Blender addon is disconnected (already implemented).

Exit criteria:

1. End-user can see backend type, model, and engine diagnostics in UI.
2. Queue errors produce visible banners.

### Workstream E: Blender Addon Compatibility

File: `blender_addon/blender_helper_http.py`

Tasks:

1. Keep `/ask`, `/scene_analysis`, `/scene/update` usage unchanged (these still work through the Axum bridge on `:5179`).
2. Update health parsing to handle expanded `/health` payload with optional `engine` field.
3. Improve addon status messaging:
   1. bridge down -> "Blender Helper not running"
   2. engine disconnected -> "Shared engine not available"
   3. model not loaded -> "No model loaded"
4. Addon should not break if `engine` field is absent (backward compatibility).

Exit criteria:

1. Addon Ask/Suggestions/Test Connection work when backend is `SharedEngine`.
2. Addon still works when backend is `Onnx` or `Ollama` (no regression).

### Workstream F: Packaging and Runtime Alignment

Files:

1. `src-tauri/tauri.conf.json`
2. `src-tauri/Cargo.toml`
3. Build scripts (`build_app.bat`, `build_app.sh`)

Tasks:

1. Add `smolpc-engine-client` dependency to `Cargo.toml` (if vendoring crate) or confirm engine client module compiles standalone.
2. Ensure bundle still includes `rag_system/**/*` and `models/**/*` resources.
3. If shared engine sidecar is bundled, add to Tauri resource configuration.
4. Update startup docs to reflect shared-engine-first path.
5. Verify fresh install starts and reaches shared engine contract.

Exit criteria:

1. `npm run tauri build` succeeds with engine client code included.
2. Fresh install auto-detects engine when available.

### Workstream G: Validation and QA

Use shared-engine onboarding checklist from `smolpc-codehelper/docs/APP_ONBOARDING_PLAYBOOK.md`.

Mandatory validation matrix:

1. `engine_client::health()` returns success
2. `engine_client::meta()` confirms protocol major `1`
3. `engine_load_model` IPC command succeeds
4. `engine_status` IPC command shows current model
5. `assistant_stream_ask` streams tokens via shared engine backend
6. `inference_cancel` cancels shared engine generation mid-stream
7. Engine `429` maps to `ENGINE_QUEUE_FULL` and UI banner
8. Engine `504` maps to `ENGINE_QUEUE_TIMEOUT` and UI banner
9. Diagnostics fields visible in UI:
   1. `active_backend`
   2. `runtime_engine`
   3. `selection_reason`
   4. `dml_gate_state` (when available)
10. Backend switch from `shared_engine` to `onnx` or `ollama` works cleanly
11. Addon health check succeeds when backend is `SharedEngine`

Evidence artifact:

1. Add `Docs/ONBOARDING_VALIDATION.md` containing request/response snapshots and pass/fail outcomes.

## AI Session Task Cards

1. **AG-01:** Implement Rust engine client module (token resolution, HTTP calls, SSE streaming, error mapping)
2. **AG-02:** Extend `GenerationBackend` enum and wire `SharedEngine` into generation dispatch
3. **AG-03:** Add engine-specific IPC commands (`engine_status`, `engine_load_model`, `engine_list_models`)
4. **AG-04:** Add startup auto-detection of shared engine availability
5. **AG-05:** Extend frontend types + API wrappers for engine diagnostics
6. **AG-06:** Add UI engine diagnostics display + queue state banners
7. **AG-07:** Addon compatibility pass (health payload, status messaging)
8. **AG-08:** Packaging alignment + Cargo.toml deps + build verification
9. **AG-09:** QA matrix + validation evidence doc

## Suggested Execution Order (Strict)

1. AG-01 (engine client is the foundation)
2. AG-02 + AG-03 (backend dispatch + IPC surface)
3. AG-04 (startup auto-detection)
4. AG-05 + AG-06 (UI layer)
5. AG-07 (addon compat)
6. AG-08 (packaging)
7. AG-09 (validation)

## Test Scenarios

### IPC-level tests

1. `assistant_status` with engine reachable (backend: `shared_engine`)
2. `assistant_status` with engine unreachable (fallback active)
3. `engine_load_model` valid model
4. `engine_load_model` invalid model
5. `assistant_stream_ask` valid question + scene context via shared engine
6. `assistant_stream_ask` oversize input rejection (already implemented, verify still works)
7. `inference_cancel` during active shared engine generation
8. Engine `429` queue full -> error code mapping
9. Engine `504` queue timeout -> error code mapping
10. Protocol mismatch -> `ENGINE_PROTOCOL_MISMATCH`
11. Backend switch: `shared_engine` -> `onnx` -> `ollama` -> `shared_engine`

### Axum bridge tests

1. `/health` returns engine diagnostics when backend is `shared_engine`
2. `/health` omits engine field when backend is `onnx` or `ollama`
3. `/ask` works via bridge with shared engine backend

### UI tests

1. Startup auto-detection shows correct backend
2. Backend toggle works for all three options
3. Cancel button while generating via shared engine
4. Queue full/timeout banner visibility
5. Engine diagnostics display (runtime engine, selection reason)
6. Graceful degradation when engine goes down mid-session

### Addon tests

1. Ask question from Blender panel (routed through Axum bridge to shared engine)
2. Get suggestions from Blender panel
3. Scene sync timer updates scene cache
4. Health check differentiates bridge and engine failures
5. Addon works without regression when backend is `onnx` or `ollama`

## Risks and Mitigations

1. **Model behavior mismatch** (coder model vs educational instruct model).
   1. Mitigation: educational system prompt in `prompts.rs` is applied regardless of backend. Prompt logic is not changing.
2. **Token/auth failures.**
   1. Mitigation: central token loader in engine client module with explicit `ENGINE_AUTH_FAILED` error code.
3. **Protocol drift.**
   1. Mitigation: protocol-major guard in engine client. Reject connections with incompatible major version.
4. **Streaming format mismatch.**
   1. Mitigation: engine SSE format is documented in `ENGINE_API.md`. Parse `data:` lines, handle `[DONE]` sentinel. Test against real engine.
5. **Packaging misses engine client deps.**
   1. Mitigation: explicit `Cargo.toml` verification in build scripts. CI check for clean build.
6. **Engine unavailability at startup.**
   1. Mitigation: graceful fallback chain (`SharedEngine` -> `Onnx` -> `Ollama`). User can manually switch backend later.

## Definition of Done

1. `SharedEngine` backend variant is functional alongside `Onnx` and `Ollama`.
2. Shared engine contract flow implemented:
   1. health/meta check
   2. model load
   3. streaming generation
   4. cancellation
   5. status/diagnostics
3. Backend auto-detection selects `SharedEngine` when engine is available.
4. Engine diagnostics visible in UI.
5. Blender addon remains compatible across all three backends.
6. Validation checklist and evidence doc committed.
7. Docs are sufficient for a new developer or AI agent to continue without tribal knowledge.

## Defaults and Assumptions

1. Windows-first development and validation.
2. **Rust backend is the app-facing layer.** No Python at runtime.
3. All three backends coexist: `onnx` (in-process), `ollama` (fallback), `shared_engine` (new).
4. Shared engine base URL default: `http://127.0.0.1:19432`.
5. Default model id: `qwen2.5-coder-1.5b`.
6. Streaming is supported from the start (existing IPC channel architecture maps directly).
7. Scene bridge port: `127.0.0.1:5179` (unchanged).
