# Tier 2: Move Orchestration to Rust + Port RAG Retrieval

**Priority:** HIGH  
**Effort:** Medium (3-5 implementation sessions)  
**Impact:** Removes Python/Flask runtime dependency, simplifies startup, centralizes AI flow in Tauri Rust backend.

---

## Goal

Replace this current runtime path:

```text
Frontend -> HTTP (5179) -> Flask (rag_system/server.py) -> HTTP (11434) -> Ollama
```

with:

```text
Frontend -> Tauri IPC -> Rust commands -> HTTP (11434) -> Ollama
```

while keeping Blender addon scene sync functional.

---

## Current Baseline (In This Repo)

- Rust starts Python and Flask subprocess:
  - `src-tauri/src/main.rs`
  - `src-tauri/src/python_checker.rs`
  - `src-tauri/src/server_manager.rs`
- Frontend directly calls Flask endpoints:
  - `src/lib/utils/api.ts`
  - `src/lib/stores/rag.svelte.ts`
  - `src/lib/stores/blender.svelte.ts`
  - `src/App.svelte`
  - `src/lib/components/SuggestionList.svelte`
- Flask does both retrieval + prompting + Ollama requests:
  - `rag_system/server.py`
- Blender addon pushes scene state to `POST /scene/update`:
  - `blender_addon/blender_helper_http.py`

---

## Tier 2 Scope

In scope:

- Rust owns `ask_question`, `analyze_scene`, cancellation, and health.
- Rust owns RAG retrieval (load embeddings, compute similarity, return top-K context).
- Frontend uses Tauri `invoke` (not direct HTTP to localhost) for app features.
- Keep addon scene sync working, ideally without immediate addon breaking changes.

Out of scope:

- Replacing Ollama (that is Tier 3).
- Full ONNX generation model integration (Tier 3).

---

## Architecture Decision: Addon Compatibility

Blender addon currently calls HTTP endpoints on `127.0.0.1:5179`.  
If Flask is removed, scene sync breaks unless replaced.

Recommended approach:

1. Add a small Rust HTTP scene bridge on port `5179` with at least:
   - `POST /scene/update`
   - `GET /health`
   - optional `GET /scene/current` for debug/backward compatibility
2. Keep frontend off this HTTP path and on Tauri IPC.

This gives:

- no Python runtime required.
- addon can remain unchanged in first Tier 2 cut.

---

## Step-by-Step Plan

## Step 1: Create Rust Backend Module Layout

Add modules under `src-tauri/src/`:

- `commands/mod.rs`
- `commands/assistant.rs` (ask/analyze/cancel/status commands)
- `commands/scene.rs` (scene data getters)
- `scene_bridge.rs` (HTTP endpoints for addon)
- `rag/mod.rs`
- `rag/types.rs`
- `rag/index.rs` (loading embeddings + metadata)
- `rag/retriever.rs` (cosine search)
- `prompts.rs` (system/user prompt builders migrated from Python)
- `state.rs` (shared app state: scene cache, rag index, generation state, http client)

Notes:

- Keep code structure close to CodeHelper command/state patterns so Tier 3 reuse is easy.
- Tier 1 command names can remain if already implemented; Tier 2 should build on them.

---

## Step 2: Add Required Rust Dependencies

Update `src-tauri/Cargo.toml`:

- async/runtime:
  - `tokio` (full)
  - `futures-util`
- networking:
  - `reqwest` with `json`, `stream`
  - `axum` (or `warp`/`hyper`) for scene bridge
- RAG math/data:
  - `ndarray`
  - `ndarray-npy`
  - `serde`, `serde_json` (already present)
- embedding model in Rust:
  - `fastembed` (recommended) or equivalent sentence embedding crate
- utilities:
  - `log`, `env_logger` (if not already)
  - `thiserror`/`anyhow` for structured errors

Important:

- Keep versions consistent with Tier 1 changes to avoid duplicate runtime patterns.

---

## Step 3: Move Prompt Logic from Flask to Rust

Port the educational prompt construction from `rag_system/server.py` to `src-tauri/src/prompts.rs`:

- Q&A system prompt with strict "UI instructions only" policy.
- Scene analysis prompt with numbered action suggestions.
- Scene context formatting helpers.
- Context chunk formatting (`signature`, `text`, `url`).

This step is critical to preserve answer quality and behavior parity.

---

## Step 4: Implement Rust Ollama Orchestration Commands

In `commands/assistant.rs`, add:

- `assistant_ask(question, scene_context, model?) -> AskResponse`
- `assistant_analyze_scene(scene_context, goal?, model?) -> SceneAnalysisResponse`
- `assistant_cancel_generation()`
- `assistant_status() -> { connected, model, generating, rag_loaded }`

Implementation notes:

- Reuse Tier 1 streaming + cancellation path for generation.
- For non-streaming consumer paths, aggregate stream chunks server-side and return full response.
- Unify request contract names: use `scene_context` everywhere (frontend, addon adapter, Rust).

---

## Step 5: Port RAG Retrieval from Python to Rust

### 5A: Data Format Migration

Current metadata format is Python pickle (`rag_system/simple_db/metadata.pkl`), which is not safe/practical for Rust loading.

Update `rag_system/build_database.py` to also output:

- `rag_system/simple_db/metadata.json`

Keep `metadata.pkl` temporarily for backward compatibility while migrating.

### 5B: Rust Loader

In `rag/index.rs`:

- load `embeddings.npy` -> `Array2<f32>`
- load `metadata.json` -> `Vec<RagChunk>`
- validate row counts match metadata length at startup

### 5C: Query Embedding + Similarity

In `rag/retriever.rs`:

- generate query embedding using same model family as Python (`all-MiniLM-L6-v2`)
- compute cosine similarity with safe normalization (zero-norm protection)
- return top-K contexts with similarity scores

Parity target:

- Top-3 retrieval overlap close to Python implementation for fixed test queries.

---

## Step 6: Build Scene Cache + Scene Bridge Service

In `scene_bridge.rs`:

- run HTTP server on `127.0.0.1:5179`
- accept `POST /scene/update` payload from addon and write into shared `SceneCache`
- expose `GET /health` and optional `GET /scene/current`

In `state.rs`:

- `SceneCache` with:
  - latest scene payload
  - last update timestamp
  - stale threshold helper (`> 30s` is disconnected)

Lifecycle:

- start bridge in `main.rs` setup
- stop on app exit via cancellation token / join handle

---

## Step 7: Replace Frontend HTTP Calls with Tauri IPC

Update frontend API layer:

- `src/lib/utils/api.ts` should call `invoke(...)` for:
  - ask
  - analyze scene
  - health/status
  - current scene
  - tutorial queries (if migrated now)

Update dependent files:

- `src/App.svelte`
- `src/lib/components/SuggestionList.svelte`
- `src/lib/stores/rag.svelte.ts`
- `src/lib/stores/blender.svelte.ts`

Keep one compatibility helper for addon-only HTTP flow if needed, but do not use it in frontend chat flow.

---

## Step 8: Migrate Tutorial Endpoints (Optional but Recommended In Tier 2)

Move tutorial list/step logic from Flask to Rust:

- source currently in `rag_system/server.py` and `rag_system/tutorials.json`
- add Rust commands:
  - `tutorial_list`
  - `tutorial_step`
- keep validation behavior parity (`has_object_type`, `has_modifier`, `object_count`)

If not migrated in first pass, explicitly document that Flask remains required only for tutorials.

---

## Step 9: Remove Python Startup Path from Tauri

In `src-tauri/src/main.rs`:

- remove Python detection/install flow
- remove Flask process boot/wait logic
- initialize Rust-managed state and scene bridge instead

Then retire:

- `src-tauri/src/python_checker.rs`
- `src-tauri/src/server_manager.rs`

or keep temporarily behind a feature flag:

- `legacy_flask_backend`

---

## Step 10: Packaging and Resource Updates

Update:

- `src-tauri/tauri.conf.json`
- `README.md`
- `INSTALL.md`
- `GETTING_STARTED.md`

Decide resource layout:

- Keep `rag_system/simple_db/*` and `rag_system/tutorials.json` as data assets.
- Remove runtime requirement for:
  - Flask
  - requests
  - sentence-transformers
  - Python installation

---

## Contract Changes To Lock Early

Use one canonical scene key: `scene_context`.

Current mismatch:

- Frontend suggestions call sends `scene_context`
- Flask `scene_analysis` currently reads `scene_data`

Fix in Tier 2 Rust contracts:

- request structs read `scene_context` only
- addon adapter can map its internal scene payload to this schema

---

## Testing Plan

## Unit Tests (Rust)

- RAG loader validates embedding/metadata size mismatch.
- Cosine similarity handles zero vectors safely.
- Scene cache stale/fresh status transitions.
- Prompt builder snapshot tests for required safety instructions.
- Cancellation state tests (reuse Tier 1 patterns).

## Integration Tests

- `assistant_ask` with running Ollama returns non-empty answer.
- `assistant_analyze_scene` returns 3-5 suggestions parseable by UI.
- Scene bridge receives addon payload and `scene_current` command reflects it.

## Manual Regression Tests

1. App starts without Python installed.
2. Chat works (stream + cancel if Tier 1 completed).
3. Suggestion tab works with live Blender scene data.
4. Blender addon "Test Server" still succeeds.
5. Tutorial features unchanged (if migrated) or explicitly flagged as pending.

---

## Definition of Done

- No Python runtime check or pip install on app startup.
- Frontend no longer depends on localhost Flask endpoints for core features.
- RAG retrieval happens in Rust with local embeddings/metadata.
- Addon scene sync works through Rust scene bridge.
- Startup time is materially reduced vs Flask boot path.

---

## Rollback Strategy

Implement a backend selection switch for at least one release:

- `legacy_flask_backend = true` -> current path
- `legacy_flask_backend = false` -> Tier 2 Rust path

Rollback can be a configuration toggle, not a revert.

---

## File-Level Change Checklist

Expected changes during Tier 2 implementation:

- Add:
  - `src-tauri/src/state.rs`
  - `src-tauri/src/prompts.rs`
  - `src-tauri/src/scene_bridge.rs`
  - `src-tauri/src/rag/*`
  - `src-tauri/src/commands/*` (assistant/scene/tutorial)
- Modify:
  - `src-tauri/src/main.rs`
  - `src-tauri/Cargo.toml`
  - `src-tauri/tauri.conf.json`
  - `src/lib/utils/api.ts`
  - `src/App.svelte`
  - `src/lib/components/SuggestionList.svelte`
  - `src/lib/stores/rag.svelte.ts`
  - `src/lib/stores/blender.svelte.ts`
  - `rag_system/build_database.py` (add JSON metadata output)
- Remove or feature-flag:
  - `src-tauri/src/python_checker.rs`
  - `src-tauri/src/server_manager.rs`
  - Flask runtime assumptions in docs/install flow

