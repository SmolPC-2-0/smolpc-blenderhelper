# Codebase Audit - Problems Found

**Date:** 2026-02-10
**Files Reviewed:** 58 across Rust backend, Svelte frontend, Python, and config
**Total Issues:** 54 (0 critical, 5 high, 18 medium, 18 low, 13 info)

---

## HIGH Priority (5)

### H1. `HealthResponse` TypeScript type missing fields from Rust
- **Files:** `src/lib/types/rag.ts`, `src-tauri/src/commands/assistant.rs`
- **Problem:** Rust `AssistantStatusResponse` includes `generating: bool` and `rag_error: Option<String>` fields that the frontend `HealthResponse` type does not capture. The frontend silently drops these fields.
- **Impact:** Frontend cannot display whether the backend is currently generating or show RAG error diagnostics.

### H2. `AskResponse` TypeScript type has stale fields
- **Files:** `src/lib/types/rag.ts`, `src-tauri/src/commands/assistant.rs`
- **Problem:** Frontend `AskResponse` declares `sources?: string[]` and `confidence?: number` which the Rust backend never sends. Rust sends `contexts_used: usize` and `rag_enabled: bool` which the frontend type does not include. Leftover from when the backend was Python/Flask.
- **Impact:** Code consuming `AskResponse` on the frontend can never access `contexts_used` or `rag_enabled`, and `sources`/`confidence` are always `undefined`.

### H3. `{@html}` renders markdown directly into DOM
- **Files:** `src/lib/components/ChatMessage.svelte`
- **Problem:** `{@html formattedContent}` bypasses Svelte's built-in XSS protection. Security depends entirely on the correctness of `parseMarkdown()` in `markdown.ts`. While the current `escapeHtml()` implementation is applied first and appears sound, any future regex edge case could become an XSS vector.
- **Impact:** Defense-in-depth concern. If `parseMarkdown` is ever modified incorrectly, XSS becomes possible.

### H4. ~421 lines of dead tutorial code still in codebase
- **Files:** `src/lib/stores/tutorials.svelte.ts` (137 lines), `src/lib/types/tutorial.ts` (33 lines), `src/lib/components/TutorialCard.svelte` (62 lines), `src/lib/components/TutorialViewer.svelte` (148 lines), tutorial types in `src/lib/types/rag.ts` (~26 lines), tutorial stubs in `src/lib/utils/api.ts` (~15 lines)
- **Problem:** Tutorial feature was removed from the UI (CLAUDE.md issue 6) but all supporting code remains. These files are bundled but never imported by any active code path.
- **Impact:** Unnecessary bundle size, maintenance confusion, dead code weight.

### H5. ONNX backend reports "connected" but is a stub
- **Files:** `src-tauri/src/inference/onnx.rs`, `src-tauri/src/models/loader.rs`
- **Problem:** `is_ready()` returns `true` when only a tokenizer is loaded. `inference_generate` returns a canned template string via `build_blender_response()`, not actual model inference. Status UI shows the backend as functional. `load_model` in `loader.rs` only loads the tokenizer, not an ONNX session.
- **Impact:** Users switching to the ONNX backend get canned responses while status says "connected."

---

## MEDIUM Priority (18)

### M1. 5 unused Cargo dependencies
- **File:** `src-tauri/Cargo.toml`
- **Problem:** `ort`, `ndarray`, `rand`, `rand_distr`, and `open` are listed as dependencies but never imported in any active Rust source file. They add significant compile time and binary size.
- **Impact:** Slower builds, larger binary, unnecessary complexity.

### M2. Expensive `Tokenizer` clone on every generation call
- **File:** `src-tauri/src/inference/onnx.rs` (line 172)
- **Problem:** `LoadedOnnxModel` (containing the full `Tokenizer` with vocabulary maps, merges, pre/post-processors) is cloned out of the mutex on every `generate_text` call. The clone is used to release the lock quickly, but `Tokenizer` is non-trivial to clone.
- **Impact:** Unnecessary memory allocation on every generation. Should use `Arc<LoadedOnnxModel>` for cheap reference-count cloning.

### M3. `panic!` in `get_rag_directory` instead of graceful error
- **File:** `src-tauri/src/main.rs` (line 184)
- **Problem:** If none of the 6 path resolution strategies find the RAG directory, the application panics with `panic!("Could not find rag_system directory")`. Compare with `get_models_directory` which gracefully returns a default path.
- **Impact:** Missing RAG directory shows an ugly crash message instead of a user-friendly error.

### M4. `stream_generate` and `cancel_generation` registered but unused
- **Files:** `src-tauri/src/commands/generation.rs`, `src-tauri/src/main.rs`
- **Problem:** These Tauri commands are registered in the handler but never called from the frontend. The frontend only uses `assistant_stream_ask` (which internally handles generation) and `inference_cancel`.
- **Impact:** Dead command surface area, potential confusion about which commands are active.

### M5. Settings store uses direct mutation
- **File:** `src/lib/stores/settings.svelte.ts` (lines 29-43)
- **Problem:** `settings.theme = theme` and `settings.showScenePanel = !settings.showScenePanel` directly mutate `$state` properties. This is inconsistent with the chats store which was specifically fixed to use immutable updates (`chats = chats.map(...)`) per CLAUDE.md issue 7.6.
- **Impact:** While Svelte 5 deep reactivity does track these mutations, the pattern is inconsistent with the established convention.

### M6. No ordered list support in markdown parser
- **File:** `src/lib/utils/markdown.ts` (lines 121-123)
- **Problem:** The parser only handles unordered lists (`*` and `-`). Numbered lists (`1.`, `2.`, etc.) render as plain text. The LLM frequently generates numbered step-by-step instructions.
- **Impact:** Numbered lists display poorly without proper `<ol>` formatting.

### M7. `generateStream()` in inference store is never called
- **File:** `src/lib/stores/inference.svelte.ts` (lines 14-52)
- **Problem:** The low-level `generateStream()` method (which calls `inference_generate`) is never invoked. Only `askQuestionStream()` (which calls `assistant_stream_ask`) is used by `App.svelte`.
- **Impact:** Dead code within the inference store, untested in the application flow.

### M8. TOCTOU race in inference store `isGenerating` guard
- **File:** `src/lib/stores/inference.svelte.ts` (lines 19-21, 59-61)
- **Problem:** There is a time-of-check-time-of-use gap between checking `isGenerating` and setting it to `true`. Two async calls triggered simultaneously (e.g., double-click) could both pass the guard.
- **Impact:** Mitigated by Rust's `AtomicBool` compare-and-exchange guard and UI disabling the send button. Defense-in-depth concern.

### M9. `server.py` only loads `metadata.pkl`, ignores `metadata.json`
- **File:** `rag_system/server.py` (lines 76-90)
- **Problem:** The Rust RAG loader prefers `metadata.json` with fallback to `metadata.pkl`. The Python server does the opposite - only uses `metadata.pkl` with no `metadata.json` path. If someone runs the Python server standalone with only `metadata.json`, it would fail.
- **Impact:** Inconsistency between Rust and Python loaders.

### M10. `pickle.load()` security risk
- **File:** `rag_system/server.py` (lines 85-90)
- **Problem:** `pickle.load()` is inherently unsafe - if an attacker could replace `metadata.pkl` on disk, arbitrary code execution is possible. The `metadata.json` file is already generated, so JSON should be preferred.
- **Impact:** Potential code execution if metadata file is tampered with.

### M11. Blender timer blocks on synchronous health check
- **File:** `blender_addon/blender_helper_http.py` (line 453)
- **Problem:** `sync_scene_timer()` calls `check_server_health()` synchronously with a 5-second timeout. Blender timers run on the main thread. If the server is unresponsive, this blocks Blender for up to 5 seconds every 5 seconds.
- **Impact:** Potential Blender UI freezes when server is slow.

### M12. `_server_health_cache` loses `last_updated` key
- **File:** `blender_addon/blender_helper_http.py` (lines 439-454)
- **Problem:** Initial cache has `'last_updated': 0`, but `sync_scene_timer()` replaces the entire dict with the return from `check_server_health()`, which does not include a `last_updated` field. The key is lost after the first timer fire.
- **Impact:** Any code checking `last_updated` gets a `KeyError` after first health check.

### M13. `tauri.conf.json` version mismatch
- **File:** `src-tauri/tauri.conf.json` (line 4)
- **Problem:** Declares `"version": "4.0.0"` but CLAUDE.md states the application is at version `6.0.0`.
- **Impact:** Confusion in release tracking, could affect auto-update mechanisms.

### M14. Bundle config doesn't match documented scope
- **File:** `src-tauri/tauri.conf.json` (lines 13-17)
- **Problem:** CLAUDE.md states the bundle should include the "Entire `rag_system/` directory" including `server.py` and `requirements_server.txt`. The actual config only bundles `simple_db/**/*` and `tutorials.json`.
- **Impact:** If Python server is needed as fallback, it won't be in the bundle. Documentation should be updated if it's intentionally excluded.

### M15. Svelte runes mode not globally enabled
- **Files:** `svelte.config.js`, `vite.config.ts`
- **Problem:** `svelte.config.js` sets `runes: undefined` (let individual files control). Since the entire codebase uses Svelte 5 runes, this should be `true` globally to catch accidental use of legacy Svelte 4 reactivity.
- **Impact:** New components could accidentally use legacy patterns without error.

### M16. Unused `BlenderObject` import
- **File:** `src/lib/stores/blender.svelte.ts` (line 1)
- **Problem:** `BlenderObject` is imported but never referenced standalone in the file.
- **Impact:** Dead import, minor code quality issue.

### M17. `nResults` camelCase vs Rust's `n_results` snake_case
- **File:** `src/lib/utils/api.ts` (lines 56-61)
- **Problem:** Frontend sends `nResults` (camelCase), Rust expects `n_results` (snake_case). Works via Tauri's auto-conversion but is fragile implicit behavior.
- **Impact:** Could break if Tauri changes auto-conversion behavior.

### M18. Tutorials store mutates `$state` directly
- **File:** `src/lib/stores/tutorials.svelte.ts` (lines 58-66, 80-84)
- **Problem:** Direct nested mutation of `$state` objects (`progress[tutorial.id] = {...}`, `progress[id].currentStep = step`). Same inconsistency as settings store.
- **Impact:** Lower priority since tutorials are removed from UI, but the code remains in the codebase.

---

## LOW Priority (18)

### L1. Dead Rust files not compiled but still in repo
- **Files:** `src-tauri/src/python_checker.rs`, `src-tauri/src/server_manager.rs`
- **Problem:** Legacy from pre-Tier 2 migration. Not declared as `mod` in `main.rs` so never compiled, but still present in the directory.

### L2. `reqwest` "blocking" feature unused in active code
- **File:** `src-tauri/Cargo.toml`
- **Problem:** The `blocking` feature is only used by dead `server_manager.rs`.

### L3. Scene bridge always reports `generating: false`
- **File:** `src-tauri/src/scene_bridge.rs` (line 97)
- **Problem:** HTTP `/health` passes `false` for `generating` because the bridge has no access to `GenerationState`.

### L4. Log file path returned but never written to
- **Files:** `src-tauri/src/main.rs` (line 47), `src-tauri/src/logger.rs`
- **Problem:** `setup_log_file` creates the directory and returns a path, but nothing writes to this file in the Tier 2 architecture.

### L5. Vestigial `embeddings.npy` existence check
- **File:** `src-tauri/src/rag/index.rs` (lines 19-25)
- **Problem:** Embeddings path is checked for existence and a warning logged, but the file is never loaded. Retriever uses keyword overlap, not vector similarity.

### L6. ONNX stub prompt parsers coupled to format
- **File:** `src-tauri/src/inference/onnx.rs` (lines 205-229)
- **Problem:** `extract_question` and `extract_scene_hint` are tightly coupled to the exact formatting in `prompts.rs`. If prompt format changes, parsers silently fail.

### L7. Version mismatch across 4 files
- **Files:** `package.json` (`0.1.0`), `Cargo.toml` (`0.1.0`), `tauri.conf.json` (`4.0.0`), CLAUDE.md (`6.0.0`)
- **Problem:** Four different version numbers across four files.

### L8. `tokio` full features unnecessarily broad
- **File:** `src-tauri/Cargo.toml`
- **Problem:** `features = ["full"]` enables all Tokio sub-features. Only `macros`, `rt-multi-thread`, `sync`, and `time` are likely needed.

### L9. CSP set to `null` in Tauri config
- **File:** `src-tauri/tauri.conf.json` (line 25)
- **Problem:** Disables all Content Security Policy protections. A minimal CSP would add defense-in-depth.

### L10. `tsconfig.json` missing `forceConsistentCasingInImports`
- **File:** `tsconfig.json`
- **Problem:** On Windows (case-insensitive), import casing mismatches work locally but break on Linux CI.

### L11. Imports inside function bodies in `server.py`
- **File:** `rag_system/server.py` (lines 271, 314, 556, 589, 644)
- **Problem:** `json`, `time`, `re` imported inside functions instead of top-level. Violates PEP 8.

### L12. `model` parameter not validated in `server.py`
- **File:** `rag_system/server.py` (lines 453, 543)
- **Problem:** `data.get('model')` passed to `call_ollama()` without type or length validation.

### L13. Emoji in `build_database.py` will crash on Windows
- **File:** `rag_system/build_database.py`
- **Problem:** Uses emoji characters extensively. Windows non-UTF-8 console codepage will throw `UnicodeEncodeError`. `server.py` was fixed but this file was not.

### L14. Unicode emoji in Blender addon console output
- **File:** `blender_addon/blender_helper_http.py` (lines 237-238, 243, 280-281, etc.)
- **Problem:** Emoji in `self.report()` calls may fail on some Windows configurations.

### L15. Suggestions list stored in `StringProperty` (type mismatch)
- **File:** `blender_addon/blender_helper_http.py` (lines 127, 264-271, 497)
- **Problem:** `get_suggestions()` returns a list but `blenderhelper_suggestions` is declared as `StringProperty`. Storing a list in a string property produces `"['item1', 'item2']"`.

### L16. `QuickExamples.svelte` component never imported
- **File:** `src/lib/components/QuickExamples.svelte`
- **Problem:** This component exists but is never used. `App.svelte` has its own inline quick examples grid.

### L17. Verbose manual destructuring in `checkHealth()`
- **File:** `src/lib/utils/api.ts` (lines 30-50)
- **Problem:** Creates a separate inline type and manually copies fields instead of using `HealthResponse` directly.

### L18. Tutorial API stubs return hardcoded empty data
- **File:** `src/lib/utils/api.ts` (lines 67-81)
- **Problem:** `getTutorialList()` and `validateTutorialStep()` return hardcoded stub responses. Dead code since tutorials were removed.

---

## INFO (13)

### I1. Legacy tutorial endpoints still in `server.py`
- Tutorial `/tutorial/list` and `/tutorial/step` endpoints remain in the Python server.

### I2. `extract_code_block()` function never called in `server.py`
- Defined at line 198 but never used anywhere.

### I3. Hardcoded Blender version in `build_database.py`
- `BLENDER_VERSION = "4.2"` requires manual update for new Blender releases.

### I4. `tutorials.json` still bundled after feature removal
- `tauri.conf.json` still includes `../rag_system/tutorials.json` in resources.

### I5. Generic package name
- `package.json` has `"name": "tauri-app"` instead of project-specific name.

### I6. No CORS on scene bridge
- Axum router in `scene_bridge.rs` has no CORS middleware. Not needed for current use (Blender addon uses urllib).

### I7. No rate limiting on HTTP bridge
- Scene bridge has no rate limiting. Low risk since localhost-only.

### I8. Cargo.toml version mismatch
- `version = "0.1.0"` does not match documented version.

### I9. `SceneObject.modifiers` optional in TS but always present from Rust
- TypeScript has `modifiers?: Array<...>` but Rust always serializes it (defaults to empty vec via `#[serde(default)]`).

### I10. `response.clone()` unnecessary in ONNX tokenizer encode
- `src-tauri/src/inference/onnx.rs` (line 177) clones the response string unnecessarily for tokenizer encode.

### I11. Float comparison in retriever uses correct `unwrap_or` idiom
- `rag/retriever.rs` line 34 handles NaN correctly. No issue.

### I12. Dual state objects (`BackendState` + `GenerationState`)
- Commands needing both must accept two `State<>` parameters. Functional but fragmented.

### I13. No path sanitization on cache file path in `build_database.py`
- `url_path.replace("/", "_")` from hardcoded `API_PAGES` list. Not exploitable in practice.

---

## Recommended Fix Order

1. **Version sync** - Pick `6.0.0` and update `package.json`, `Cargo.toml`, `tauri.conf.json`
2. **Type alignment** - Fix `HealthResponse` and `AskResponse` to match Rust structs
3. **Dead code cleanup** - Remove tutorial files, `QuickExamples.svelte`, `python_checker.rs`, `server_manager.rs`
4. **Unused Cargo deps** - Remove `ort`, `ndarray`, `rand`, `rand_distr`, `open` until ONNX is real
5. **Settings store** - Switch to immutable update pattern
6. **Ordered list support** - Add `<ol>` parsing to markdown
7. **Blender addon** - Fix `last_updated` cache key + reduce health check timeout
8. **`server.py`** - Prefer `metadata.json` over `metadata.pkl`
9. **Runes mode** - Set `runes: true` globally in `svelte.config.js`
10. **Emoji cleanup** - Replace emoji in `build_database.py` with ASCII
