# BlenderHelper -> SmolPC Shared Engine Onboarding Plan

Start date target: Monday, March 2, 2026
Audience: BlenderHelper developer and AI-assisted implementation sessions

## Summary

BlenderHelper should migrate from direct Ollama usage to the SmolPC shared inference engine contract while preserving the educational RAG behavior and Blender scene-aware workflows.

Safe migration order:

1. Stabilize the current Blender frontend baseline (`src/lib` is currently missing in `main`).
2. Add an engine bridge layer in `rag_system` that talks to `/engine/*` and `/v1/*`.
3. Rewire frontend UI/state to operate on engine lifecycle + model load + generation + cancel + diagnostics.
4. Keep Blender addon endpoint contracts stable so existing user workflows are not broken.
5. Add packaging/runtime alignment and validate against the shared-engine onboarding checklist.

This document is intended to be decision-complete and directly executable.

## Ground Truth (Current State)

1. Blender repo (`smolpc-blenderhelper`) currently runs a local Flask server (`rag_system/server.py`) and calls Ollama directly (`127.0.0.1:11434`).
2. Tauri starts/stops the Python RAG server at app startup/shutdown.
3. Blender addon and frontend both depend on Flask endpoints (`/ask`, `/scene_analysis`, `/scene/update`, `/health`, tutorials endpoints).
4. Shared engine already exists in `smolpc-codehelper`:
   1. API contract: `docs/ENGINE_API.md`
   2. App onboarding checklist: `docs/APP_ONBOARDING_PLAYBOOK.md`
   3. Rust client: `crates/smolpc-engine-client`
5. Blender frontend file `src/App.svelte` imports many `$lib/*` modules that are currently absent in this checkout; compile baseline must be restored before deeper onboarding work.
6. Shared engine model registry currently includes `qwen2.5-coder-1.5b`; Blender currently expects an educational Ollama instruct model.

## Goals

1. Move primary generation path from Ollama API calls to shared engine API calls.
2. Preserve educational prompting + RAG retrieval + tutorial/scene flows.
3. Expose shared-engine diagnostics in UI:
   1. backend
   2. runtime engine
   3. selection reason/state
   4. load/generation/cancel state
4. Keep addon compatibility and endpoint stability.
5. Produce clean handoff and reusable AI task cards.

## Non-Goals

1. Full rewrite of backend from Python to Rust in this phase.
2. Reworking tutorial validation semantics.
3. Multi-model orchestration beyond one active model at a time.

## Architecture Decisions (Locked)

1. Primary integration path:
   1. Keep Flask backend as app-facing API.
   2. Replace `call_ollama(...)` internals with shared-engine client calls over localhost HTTP.
2. Lifecycle:
   1. Flask server is responsible for checking engine health/meta and loading model.
   2. Tauri still manages Python backend process.
3. Compatibility:
   1. Preserve existing endpoint shapes where possible.
   2. Extend responses to include engine diagnostics/metrics.
4. Rollout:
   1. Non-stream generation first.
   2. Streaming endpoint added after non-stream path is stable.
5. Default model:
   1. Use env-configurable `SHARED_ENGINE_MODEL_ID`.
   2. Default: `qwen2.5-coder-1.5b` until educational ONNX model variant is registered.
6. Temporary fallback:
   1. Optional Ollama fallback behind env flag only.
   2. Default fallback disabled.

## Public API and Interface Changes (Server)

File: `rag_system/server.py`

### 1. Expand `GET /health`

Current returns basic RAG status.
New response includes engine status summary:

```json
{
  "status": "ok",
  "rag": { "enabled": true, "docs": 150 },
  "engine": {
    "connected": true,
    "protocol_version": "1.0.0",
    "current_model": "qwen2.5-coder-1.5b",
    "generating": false,
    "backend_status": {
      "active_backend": "cpu",
      "runtime_engine": "ort_cpu",
      "selection_reason": "default_cpu"
    }
  }
}
```

### 2. Add `GET /engine/status`

Returns normalized engine diagnostics payload for UI and debugging:

1. `connected`
2. `meta`
3. `status`
4. `last_error`

### 3. Add `POST /engine/load`

Request:

```json
{ "model_id": "qwen2.5-coder-1.5b" }
```

Response:

```json
{ "ok": true, "model_id": "qwen2.5-coder-1.5b" }
```

### 4. Add `POST /engine/cancel`

Response:

```json
{ "ok": true }
```

### 5. Add `GET /models`

Returns engine models (`/v1/models`) in frontend-friendly shape.

### 6. Extend `POST /ask`

Accept optional model + generation config.
Return:

1. `answer`
2. `contexts_used`
3. `rag_enabled`
4. `model_id`
5. `smolpc_metrics`
6. `backend_status` snapshot

### 7. Add phase-2 streaming endpoint (`/ask/stream`)

Relays engine SSE chunks and a final metrics event.

## Error Normalization (Locked)

Map engine outcomes to stable app-level errors:

1. HTTP `429` -> `ENGINE_QUEUE_FULL`
2. HTTP `504` -> `ENGINE_QUEUE_TIMEOUT`
3. generation cancel -> `INFERENCE_GENERATION_CANCELLED`
4. protocol mismatch -> `ENGINE_PROTOCOL_MISMATCH`
5. auth/token failures -> `ENGINE_AUTH_FAILED`
6. generic runtime errors -> `ENGINE_RUNTIME_ERROR`

## Backend Implementation Plan

## Workstream A: Baseline Recovery (Required Gate)

Goal: restore compile-able frontend baseline.

Tasks:

1. Create missing `src/lib` tree referenced by `src/App.svelte`.
2. Add minimal typed stores and components to satisfy imports.
3. Ensure `npm run check` passes before further migration.

Exit criteria:

1. Frontend compiles without missing module errors.
2. Core chat screen renders.

## Workstream B: Shared Engine Bridge Module

Create `rag_system/engine_bridge.py` and `rag_system/config.py`.

Responsibilities:

1. Resolve token path:
   1. `%LOCALAPPDATA%/SmolPC/engine-runtime/engine-token.txt` on Windows
2. Build bearer auth header.
3. Provide typed helper functions:
   1. `health()`
   2. `meta()`
   3. `status()`
   4. `load_model(model_id)`
   5. `cancel()`
   6. `list_models()`
   7. `chat_completion(messages, stream=False, generation_config=None)`
4. Validate protocol major version from `/engine/meta`.
5. Normalize exceptions into app-level error codes/messages.

Env configuration:

1. `ENGINE_BASE_URL` default `http://127.0.0.1:19432`
2. `SHARED_ENGINE_MODEL_ID` default `qwen2.5-coder-1.5b`
3. `ALLOW_OLLAMA_FALLBACK` default `0`
4. `ENGINE_CONNECT_TIMEOUT_SECS` default `5`
5. `ENGINE_REQUEST_TIMEOUT_SECS` default `120`

Exit criteria:

1. Local script can call engine health/meta/status with auth token.

## Workstream C: `server.py` Inference Migration

Goal: switch generation path from Ollama to shared engine.

Tasks:

1. Replace `call_ollama(...)` usage in `/ask` and `/scene_analysis` with `engine_bridge.chat_completion(...)`.
2. Keep existing educational prompt format and RAG context composition.
3. Continue scene context injection behavior.
4. Add/extend endpoints:
   1. `/models`
   2. `/engine/load`
   3. `/engine/status`
   4. `/engine/cancel`
5. Expand `/health` with engine summary.
6. Add structured error mapping and HTTP status translation.

Exit criteria:

1. `/ask` returns valid answer via shared engine when Ollama is not running.
2. `/scene_analysis` also returns suggestions via shared engine.

## Workstream D: UI Rewiring

Goal: user-visible control over engine lifecycle and diagnostics.

Required frontend modules:

1. `src/lib/types/api.ts`
2. `src/lib/types/inference.ts`
3. `src/lib/utils/api.ts`
4. `src/lib/stores/settings.svelte.ts`
5. `src/lib/stores/chats.svelte.ts`
6. `src/lib/stores/rag.svelte.ts`
7. `src/lib/stores/blender.svelte.ts`
8. `src/lib/stores/inference.svelte.ts`
9. `src/lib/components/StatusIndicator.svelte`
10. `src/lib/components/ModelSelector.svelte`
11. `src/lib/components/ChatInput.svelte`
12. `src/lib/components/QueueStateBanner.svelte`
13. `src/lib/components/EngineDiagnosticsPanel.svelte`

UI behavior contract:

1. On startup, poll server health and engine status.
2. Auto-load default model once, with visible status.
3. Disable send while generation is in progress.
4. Provide cancel action while generating.
5. Always display:
   1. active model
   2. active backend
   3. runtime engine
   4. selection reason
6. Show explicit queue full/timeout banners.
7. App remains usable if Blender addon is disconnected.

Exit criteria:

1. End-user can model-select, chat, and inspect backend runtime details in UI.

## Workstream E: Blender Addon Compatibility

File: `blender_addon/blender_helper_http.py`

Tasks:

1. Keep `/ask`, `/scene_analysis`, `/scene/update` usage unchanged.
2. Update health parsing to support expanded `/health` payload.
3. Improve addon status messaging:
   1. server down
   2. engine disconnected
   3. model not loaded

Exit criteria:

1. Addon Ask/Suggestions/Test Connection still work without UI regression.

## Workstream F: Packaging and Runtime Alignment

Files:

1. `src-tauri/tauri.conf.json`
2. build scripts (`build_app.bat`, `build_app.sh`)

Tasks:

1. Ensure bundle includes resources required for shared engine integration.
2. Add a release step that stages host/runtime sidecar binaries (if project chooses sidecar packaging strategy).
3. Keep `rag_system` resources bundled.
4. Update installer/startup docs to shared-engine-first path.

Exit criteria:

1. Fresh install can start Blender app and reach shared engine contract with no manual Ollama setup.

## Workstream G: Validation and QA

Use shared-engine onboarding checklist from `smolpc-codehelper/docs/APP_ONBOARDING_PLAYBOOK.md`.

Mandatory validation matrix:

1. `GET /engine/health` success
2. `GET /engine/meta` protocol major `1`
3. `POST /engine/load` success
4. `GET /engine/status` current model set
5. non-stream `/v1/chat/completions` includes `smolpc_metrics`
6. stream path emits chunks, metrics, `[DONE]` (when stream endpoint is implemented)
7. `POST /engine/cancel` works
8. queue full (`429`) handled in UI
9. queue timeout (`504`) handled in UI
10. diagnostics fields surfaced:
    1. `active_backend`
    2. `runtime_engine`
    3. `selection_reason`
    4. `dml_gate_state` when available

Evidence artifact:

1. Add `docs/ONBOARDING_VALIDATION.md` containing request/response snapshots and pass/fail outcomes.

## AI Session Task Cards

1. AG-01: Restore missing frontend baseline (`src/lib` + compile check)
2. AG-02: Implement `engine_bridge.py` + config + token handling
3. AG-03: Migrate `/ask` to shared-engine non-stream path
4. AG-04: Add server endpoints `/models` and `/engine/*`
5. AG-05: Add UI model selection + engine diagnostics
6. AG-06: Add cancel + queue state UX
7. AG-07: Addon compatibility pass and health UX updates
8. AG-08: Packaging alignment and docs update
9. AG-09: QA matrix + validation evidence doc

## Suggested Execution Order (Strict)

1. AG-01
2. AG-02
3. AG-03 + AG-04
4. AG-05 + AG-06
5. AG-07
6. AG-08
7. AG-09

Do not skip AG-01. Frontend baseline issues will block useful integration validation.

## Test Scenarios

## API-level tests

1. `/health` with engine reachable
2. `/health` with engine unreachable
3. `/engine/load` valid model
4. `/engine/load` invalid model
5. `/ask` valid question + scene context
6. `/ask` oversize input rejection
7. `/engine/cancel` during active generation
8. `429` queue full mapping
9. `504` queue timeout mapping
10. protocol mismatch mapping

## UI tests

1. startup status transitions render clearly
2. default model auto-load behavior
3. manual model switch behavior
4. cancel button while generating
5. queue full/timeout banner visibility
6. diagnostics panel values update after generation/load

## Addon tests

1. ask question from Blender panel
2. get suggestions from Blender panel
3. scene sync timer updates server
4. status check differentiates server and engine failures

## Risks and Mitigations

1. Model behavior mismatch (coder model vs educational instruct model).
   1. Mitigation: keep strict educational system prompt and short answers policy.
2. Missing frontend baseline files.
   1. Mitigation: AG-01 hard gate before integration work.
3. Token/auth failures.
   1. Mitigation: central token loader and explicit auth error messages.
4. Protocol drift.
   1. Mitigation: protocol-major guard in engine bridge.
5. Packaging misses sidecar dependencies.
   1. Mitigation: explicit artifact verification in release checklist.

## Definition of Done

1. Blender app primary generation no longer depends on Ollama.
2. Shared engine contract flow implemented:
   1. health/meta
   2. load
   3. generate
   4. cancel
   5. status
3. Backend diagnostics visible in UI.
4. Blender addon remains compatible.
5. Validation checklist and evidence doc committed.
6. Docs are sufficient for a new developer or AI agent to continue without tribal knowledge.

## Defaults and Assumptions

1. Windows-first development and validation.
2. Flask backend remains app-facing in this phase.
3. Shared engine base URL default: `http://127.0.0.1:19432`.
4. Default model id: `qwen2.5-coder-1.5b`.
5. Streaming is phase 2 after non-stream stabilization.
6. Ollama fallback disabled by default.