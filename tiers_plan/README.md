# BlenderHelper Tiered Migration Plans

This folder contains implementation plans for migrating BlenderHelper from the current Python Flask + Ollama architecture to a Rust-first architecture, and then optionally to full in-process ONNX inference.

Current baseline in this repository:

- Frontend calls Flask over HTTP (`src/lib/utils/api.ts` -> `http://127.0.0.1:5179`)
- Flask server orchestrates RAG + Ollama (`rag_system/server.py`)
- Tauri app boots Python, installs deps, and starts Flask (`src-tauri/src/main.rs`, `src-tauri/src/python_checker.rs`, `src-tauri/src/server_manager.rs`)
- Blender addon posts scene state to Flask (`blender_addon/blender_helper_http.py`)

## Plan Documents

1. `tiers_plan/TIER_1_STREAMING_AND_CANCELLATION.md`
   - Goal: token streaming and user cancellation for chat generation.
   - Status in repo: already drafted.
2. `tiers_plan/TIER_2_RUST_ORCHESTRATION_AND_RAG_PORT.md`
   - Goal: remove Python Flask orchestration and move chat/suggestions/RAG orchestration into Rust.
3. `tiers_plan/TIER_3_ONNX_RUNTIME_MIGRATION.md`
   - Goal: remove Ollama dependency and run model inference directly in-process via ONNX Runtime.

## Recommended Execution Order

1. Implement Tier 1 fully and validate UX wins (streaming + cancel).
2. Implement Tier 2 in two phases:
   - Phase A: Rust orchestration with temporary Flask RAG bridge.
   - Phase B: full Rust RAG retrieval and Flask removal.
3. Start Tier 3 only after Tier 2 is stable in production-like testing.

## Handoff Notes For Next Session

- Keep changes behind feature flags where possible (`settings` or env-based), so rollback is one toggle and not a revert.
- Preserve endpoint compatibility for Blender addon during Tier 2 (`/scene/update`, `/health`) or explicitly version addon and desktop app together.
- When removing Python/Flask, update both docs and installer assumptions:
  - `README.md`
  - `INSTALL.md`
  - `GETTING_STARTED.md`
  - `src-tauri/tauri.conf.json` resource bundling rules.

