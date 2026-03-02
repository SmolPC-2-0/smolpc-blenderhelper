# SmolPC Blender Helper - Claude Code Documentation

This file contains implementation plans and architectural documentation for use across multiple Claude Code sessions.

---

# Current Application State

**Last Updated:** 2026-02-10
**Version:** 6.0.0
**Status:** ✅ Production Ready - Tier 3 ONNX Backend Scaffolding + Tier 2 Scene Bridge

## Quick Status Summary

✅ **Svelte 5 + Tailwind CSS 4 Migration** - COMPLETE
✅ **Unified Executable Launcher** - COMPLETE
✅ **Multiple Chat Sessions with History** - COMPLETE
✅ **Auto-dependency Management** - COMPLETE
✅ **Hidden Console Windows** - COMPLETE
✅ **Security Audit & Hardening** - COMPLETE (Feb 2026)
✅ **UI Bug Fixes & Improvements** - COMPLETE (Feb 2026)
✅ **Tier 1 Streaming + Cancellation** - COMPLETE (Feb 2026)
✅ **Tier 2 Rust Orchestration + RAG Port** - COMPLETE (Feb 2026)
✅ **Tier 3 ONNX Runtime Backend Integration** - COMPLETE (Feb 2026, scaffolding stage)

---

# Tier 1 Streaming + Cancellation (NEW - Feb 2026)

## Overview

Chat responses now stream token-by-token through Tauri IPC channels instead of waiting for a full `/ask` response. Cancellation is supported mid-generation through a shared Rust `AtomicBool` state.

## Implemented Changes

### Rust Backend (`src-tauri`)

- Added streaming Ollama client in `src-tauri/src/ollama.rs`
  - Uses `reqwest` streaming (`bytes_stream()`) with line-buffered JSON parsing
  - Supports cancellation checks between chunks
  - Returns generation metrics (`total_tokens`, `total_time_ms`, `tokens_per_second`)
- Added generation command/state module:
  - `src-tauri/src/commands/mod.rs`
  - `src-tauri/src/commands/generation.rs`
  - Commands:
    - `stream_generate(system_prompt, user_prompt, on_token)`
    - `cancel_generation()`
    - `is_generating()`
- Registered `GenerationState` and commands in `src-tauri/src/main.rs`
- Added Rust deps in `src-tauri/Cargo.toml`:
  - `tokio`, `log`, `env_logger`, `futures-util`
  - enabled `reqwest` `stream` feature

### Python RAG Server (`rag_system`)

- Added `POST /rag/retrieve` in `rag_system/server.py`
  - Returns context chunks without calling Ollama
  - Includes input validation for `query` and `n_results`

### Frontend (`src`)

- Added inference store: `src/lib/stores/inference.svelte.ts`
  - Creates Tauri `Channel<string>` and handles stream lifecycle
  - Handles cancellation and metrics
- Added prompt builder: `src/lib/utils/prompt.ts`
  - Builds system/user prompts client-side to mirror server educational prompt behavior
- Added inference type: `src/lib/types/inference.ts`
- Extended RAG types/API:
  - `src/lib/types/rag.ts` (`RagContext`, `RagRetrieveResponse`)
  - `src/lib/utils/api.ts` (`retrieveRagContext(query)`)
- Updated chat flow in `src/App.svelte`
  - Fetches RAG contexts via `/rag/retrieve`
  - Builds prompts in TS
  - Streams tokens via `stream_generate` into live assistant message updates
- Updated chat UI:
  - `src/lib/components/ChatInput.svelte`: cancel button while generating
  - `src/lib/components/ChatMessage.svelte`: typing dots + blinking cursor during stream

---

# Tier 2 Rust Orchestration + RAG Port (NEW - Feb 2026)

## Overview

Core orchestration now runs in Rust/Tauri, removing Python/Flask from desktop startup. Frontend app flows now use Tauri IPC for health, scene state, RAG retrieval, chat streaming, and scene analysis.  
For Blender addon compatibility, Rust also runs an HTTP scene bridge on `127.0.0.1:5179`.

## Implemented Changes

### Rust Backend (`src-tauri`)

- Added shared backend state in `src-tauri/src/state.rs`
  - Scene cache with stale detection (`> 30s`)
  - In-memory RAG index state
- Added Rust RAG modules:
  - `src-tauri/src/rag/types.rs`
  - `src-tauri/src/rag/retriever.rs`
  - `src-tauri/src/rag/index.rs`
  - Loads metadata (prefers `metadata.json`, fallback `metadata.pkl`)
  - Uses keyword-overlap ranking for top-K retrieval in Rust
- Added prompt builders in `src-tauri/src/prompts.rs`
  - Educational UI-only response policy for Blender guidance
  - Scene analysis prompt templates
- Added command modules:
  - `src-tauri/src/commands/assistant.rs`
  - `src-tauri/src/commands/scene.rs`
  - Extended `src-tauri/src/commands/generation.rs` with `assistant_stream_ask`
- Added HTTP compatibility bridge:
  - `src-tauri/src/scene_bridge.rs`
  - Endpoints: `/health`, `/scene/update`, `/scene/current`, `/rag/retrieve`, `/ask`, `/scene_analysis`, `/test`
- Updated `src-tauri/src/main.rs`
  - Removed Python dependency checks and Flask process bootstrap
  - Initializes Rust RAG state and starts Axum bridge on startup
  - Graceful bridge shutdown on app exit
- Updated `src-tauri/Cargo.toml`
  - Added: `axum`, `serde-pickle`

### Frontend (`src`)

- Migrated app API wrappers to IPC in `src/lib/utils/api.ts`
  - `assistant_status`, `assistant_ask`, `assistant_analyze_scene`, `retrieve_rag_context`, `scene_current`
- Updated chat flow in `src/App.svelte`
  - Uses `assistant_stream_ask` (Rust handles retrieval + prompts)
- Updated inference store in `src/lib/stores/inference.svelte.ts`
  - Added `askQuestionStream(question, sceneContext, onToken)`
- Existing polling stores (`ragStore`, `blenderStore`) now resolve through IPC-backed API wrappers.

### RAG Data Pipeline (`rag_system`)

- Updated `rag_system/build_database.py` to also emit:
  - `rag_system/simple_db/metadata.json`
- `metadata.pkl` remains generated for transitional compatibility.

---

# Tier 3 ONNX Runtime Backend Integration (NEW - Feb 2026)

## Overview

Generation now supports a native in-process ONNX backend in Rust with model discovery/load/unload commands, while preserving an Ollama fallback path for staged rollout.

Current stage note: ONNX model lifecycle, backend routing, and streaming/cancel plumbing are implemented; full decoder graph execution/sampling is still a follow-up step.

## Implemented Changes

### Rust Backend (`src-tauri`)

- Added ONNX/model modules:
  - `src-tauri/src/inference/onnx.rs`
  - `src-tauri/src/models/runtime_spec.rs`
  - `src-tauri/src/models/registry.rs`
  - `src-tauri/src/models/loader.rs`
- Added backend selection to shared state:
  - `src-tauri/src/state.rs`
  - `GenerationBackend` enum (`onnx`/`ollama`)
  - ONNX runtime/model lifecycle state
- Added inference command module:
  - `src-tauri/src/commands/inference.rs`
  - Commands:
    - `list_models()`
    - `load_model(model_id?)`
    - `unload_model()`
    - `set_generation_backend(backend)`
    - `get_generation_backend()`
    - `inference_generate(system_prompt, user_prompt, on_token)`
    - `inference_cancel()`
- Refactored existing assistant + streaming commands to route through backend abstraction:
  - `src-tauri/src/commands/assistant.rs`
  - `src-tauri/src/commands/generation.rs`
- Updated startup wiring:
  - Discover ONNX models from `models/`
  - Attempt default ONNX model auto-load
  - Fallback to Ollama backend if default model load fails
- Updated health/bridge surface:
  - `assistant_status` and `/health` now expose backend + model fields.

### Frontend (`src`)

- Extended IPC API wrappers in `src/lib/utils/api.ts`:
  - model commands + backend toggle commands
- Extended status/types:
  - `src/lib/types/inference.ts`
  - `src/lib/types/rag.ts`
- Updated status polling/store behavior in `src/lib/stores/rag.svelte.ts`
  - Shows backend/model in status state
- Updated inference store command targets in `src/lib/stores/inference.svelte.ts`
  - Uses `inference_generate` and `inference_cancel`
- Added backend toggle control in `src/App.svelte`
- Updated status UI in `src/lib/components/StatusIndicator.svelte`

### Packaging + Assets

- Added model asset folder scaffold:
  - `models/README.md`
- Updated Tauri resources:
  - `src-tauri/tauri.conf.json` now bundles `../models/**/*`
- Added Tier 3 deps in `src-tauri/Cargo.toml`:
  - `ort` (`load-dynamic`, `ndarray`)
  - `tokenizers`
  - `ndarray`
  - `rand`, `rand_distr`

---

# Architecture Overview

## Technology Stack

| Layer | Technology | Version | Status |
|-------|-----------|---------|--------|
| **Frontend Framework** | Svelte | 5.46.1 | ✅ Active |
| **State Management** | Svelte 5 Runes | Built-in | ✅ Active |
| **Styling** | Tailwind CSS | 4.1.18 | ✅ Active |
| **Icons** | Lucide Svelte | 0.511.0 | ✅ Active |
| **Build Tool** | Vite | 6.4.1 | ✅ Active |
| **Language** | TypeScript | 5.8.3 | ✅ Active |
| **Desktop Framework** | Tauri | 2.9.2 | ✅ Active |
| **Backend Orchestration** | Rust (Tauri Commands) | 2021 Edition | ✅ Active |
| **LLM Backend** | ONNX Runtime (Rust, in-process) | `ort` 2.0.0-rc.9 | ✅ Primary |
| **LLM Fallback** | Ollama | Latest | ✅ Optional Fallback |

## Application Structure

```
smolpc-blenderhelper/
├── src/                           # Svelte 5 Frontend
│   ├── lib/
│   │   ├── components/           # UI Components (10 files)
│   │   │   ├── Sidebar.svelte
│   │   │   ├── ChatMessage.svelte
│   │   │   ├── ChatInput.svelte
│   │   │   ├── StatusIndicator.svelte
│   │   │   ├── BlenderIndicator.svelte
│   │   │   ├── ScenePanel.svelte
│   │   │   ├── QuickExamples.svelte
│   │   │   ├── SuggestionList.svelte
│   │   │   ├── TutorialCard.svelte
│   │   │   └── TutorialViewer.svelte
│   │   ├── stores/               # Svelte 5 Runes Stores (6 files)
│   │   │   ├── chats.svelte.ts
│   │   │   ├── settings.svelte.ts
│   │   │   ├── rag.svelte.ts
│   │   │   ├── blender.svelte.ts
│   │   │   ├── inference.svelte.ts
│   │   │   └── tutorials.svelte.ts
│   │   ├── types/                # TypeScript Interfaces (6 files)
│   │   │   ├── chat.ts
│   │   │   ├── settings.ts
│   │   │   ├── blender.ts
│   │   │   ├── inference.ts
│   │   │   ├── tutorial.ts
│   │   │   └── rag.ts
│   │   └── utils/                # Helper Functions (5 files)
│   │       ├── storage.ts
│   │       ├── date.ts
│   │       ├── markdown.ts
│   │       ├── prompt.ts
│   │       └── api.ts
│   ├── App.svelte                # Main App Component
│   ├── main.ts                   # Entry Point
│   ├── app.css                   # Tailwind CSS Config
│   └── index.html                # HTML Shell
│
├── src-tauri/                     # Rust Backend (Tier 3 Inference + Tier 2 Orchestration)
│   ├── src/
│   │   ├── main.rs               # Tauri startup + scene bridge lifecycle
│   │   ├── ollama.rs             # Ollama Streaming Client
│   │   ├── inference/            # ONNX runtime backend implementation
│   │   ├── models/               # ONNX model registry + loading
│   │   ├── scene_bridge.rs       # Addon-compatible HTTP bridge (:5179)
│   │   ├── state.rs              # Shared scene + RAG app state
│   │   ├── prompts.rs            # Prompt construction templates
│   │   ├── commands/             # Tauri commands (assistant/scene/generation/inference)
│   │   ├── rag/                  # Rust RAG loader + retriever
│   │   └── logger.rs             # Log File Management
│   ├── Cargo.toml                # Rust Dependencies
│   └── tauri.conf.json           # Tauri Configuration + Resource Bundling
│
├── rag_system/                    # RAG Data Assets + Build Tools
│   ├── server.py                 # Legacy Flask server (not used in Tier 2 runtime)
│   ├── build_database.py         # Builds embeddings + metadata.json
│   ├── requirements_server.txt   # Legacy Python requirements for tooling
│   ├── simple_db/                # Vector Embeddings Database
│   │   ├── embeddings.npy
│   │   ├── metadata.json
│   │   └── metadata.pkl
│   └── tutorials.json            # Tutorial Content
│
├── blender_addon/                # Blender Integration
│   └── blender_helper_http.py   # Scene Data Exporter
│
├── models/                       # ONNX model assets (tier 3)
│   └── README.md
│
├── build_app.bat                 # Windows Build Script
├── build_app.sh                  # Linux/macOS Build Script
├── INSTALL.md                    # Installation Guide
└── README.md                     # User Documentation
```

---

# Runtime Orchestration (Updated - Feb 2026)

## Overview

The desktop app boots without Python/Flask orchestration. Rust initializes RAG assets, discovers ONNX models, selects a generation backend (`onnx` primary, `ollama` fallback), starts the addon-compatible scene bridge (`127.0.0.1:5179`), and serves app functionality through Tauri IPC commands.

## Key Components

### 1. App State (`src-tauri/src/state.rs`)

**Responsibilities:**
- Owns scene cache shared by IPC commands and HTTP bridge
- Tracks stale scene state (`> 30s`)
- Owns in-memory RAG index handle
- Owns generation backend selection + ONNX runtime state

### 2. RAG Loader + Retriever (`src-tauri/src/rag/*`)

**Responsibilities:**
- Loads metadata from `metadata.json` (fallback: `metadata.pkl`)
- Performs lightweight keyword-overlap ranking for top-K context retrieval

### 3. Scene Bridge (`src-tauri/src/scene_bridge.rs`)

**Responsibilities:**
- Maintains addon compatibility over HTTP (`/scene/update`, `/health`, `/scene/current`)
- Proxies `/ask`, `/scene_analysis`, `/rag/retrieve` to Rust orchestration logic

### 4. Command Surface (`src-tauri/src/commands/*`)

**Responsibilities:**
- `assistant_stream_ask` for streaming chat + cancellation
- `assistant_ask` and `assistant_analyze_scene` for non-streaming calls
- `assistant_status`, `retrieve_rag_context`, `scene_current`, `scene_update`
- `list_models`, `load_model`, `unload_model`
- `set_generation_backend`, `get_generation_backend`
- `inference_generate`, `inference_cancel`

### 5. Main Integration (`src-tauri/src/main.rs`)

**Startup Sequence:**
1. Setup logs and resolve bundled `rag_system` directory
2. Load Rust RAG index
3. Discover ONNX models in `models/`
4. Attempt default ONNX model load (fallback to Ollama backend if unavailable)
5. Register shared backend state + generation state
6. Start Rust scene bridge on `127.0.0.1:5179`
7. Launch Tauri UI

**Shutdown Sequence:**
1. `ExitRequested` event received
2. Scene bridge receives shutdown signal
3. Async bridge task stops cleanly

---

# Frontend Architecture (Svelte 5)

## State Management with Runes

All stores use Svelte 5's new reactivity primitives:

### Example: Chat Store
```typescript
let chats = $state<Chat[]>(loadFromStorage(STORAGE_KEY, []));
let currentChatId = $state<string | null>(null);

const currentChat = $derived(
  chats.find(c => c.id === currentChatId) ?? null
);
```

### Available Stores

1. **`chatsStore`** - Chat history management
   - Multiple chat sessions
   - Message persistence
   - Auto-title generation
   - Time-grouped display

2. **`settingsStore`** - User preferences
   - Theme (light/dark)
   - Auto-scroll behavior
   - Scene panel visibility
   - Polling intervals

3. **`ragStore`** - RAG server connection
   - Health check polling
   - Connection status
   - Document count

4. **`blenderStore`** - Blender scene state
   - Scene data caching
   - Object list
   - Active object tracking

5. **`tutorialsStore`** - Tutorial system
   - Tutorial list
   - Current tutorial/step
   - Validation state

## Component Structure

### Core UI Components
- **Sidebar** - Time-grouped chat list (Today, Yesterday, Last Week, Older)
- **ChatMessage** - Markdown rendering, streaming indicator
- **ChatInput** - Auto-expanding textarea, Enter/Shift+Enter support
- **StatusIndicator** - RAG server health (color-coded dot + doc count)
- **BlenderIndicator** - Blender connection status

### Blender-Specific Components
- **ScenePanel** - Collapsible scene overview
- **QuickExamples** - Blender prompt suggestions
- **SuggestionList** - AI-generated learning suggestions
- **TutorialCard** - Tutorial preview with metadata
- **TutorialViewer** - Step-by-step tutorial with validation

## Routing & Navigation

**Tab-Based Navigation:**
- Chat (default)
- Suggestions

No router library needed - simple state-based tabs.

---

# Backend Integration

## Runtime Interfaces

**Base URL:** `http://127.0.0.1:5179`

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/health` | GET | Scene bridge health + RAG status (addon compatibility) |
| `/scene/update` | POST | Cache scene data from Blender addon |
| `/scene/current` | GET | Retrieve cached scene snapshot |
| `/rag/retrieve` | POST | Retrieve RAG contexts via Rust retriever |
| `/ask` | POST | Ask educational question via Rust orchestration |
| `/scene_analysis` | POST | Get scene suggestions via Rust orchestration |

Primary app path uses Tauri IPC, while the HTTP interface remains for Blender addon compatibility.

## API Integration Pattern

Frontend API wrappers call Tauri commands via `invoke`:

```typescript
import { invoke } from '@tauri-apps/api/core';

const status = await invoke('assistant_status');
const scene = await invoke('scene_current');
const analysis = await invoke('assistant_analyze_scene', {
  request: { scene_context: scene.scene_data }
});
```

Streaming chat uses the dedicated assistant command:

```typescript
const onTokenChannel = new Channel<string>();
onTokenChannel.onmessage = (token) => appendToAssistantMessage(token);

await invoke('assistant_stream_ask', {
  question,
  sceneContext,
  onToken: onTokenChannel
});
```

**Features:**
- Streaming token updates via IPC channel
- Mid-generation cancellation
- Rust-owned RAG retrieval and prompt construction

---

# Data Flow

## Question Answering Flow

```
User Input
    ↓
ChatInput Component
    ↓
App.svelte handleSendMessage()
    ↓
Add user message to chatsStore
    ↓
Add assistant placeholder (isStreaming: true)
    ↓
Invoke Tauri command assistant_stream_ask
    ├── question: string
    ├── scene_context: object|null
    └── on_token: Channel<string>
        ↓
Rust Orchestration Pipeline
    ├── Retrieve top-K RAG contexts
    ├── Build system + user prompts in Rust
    ├── Stream tokens from selected backend (ONNX in-process or Ollama fallback)
    └── Support cancel_generation via AtomicBool
        ↓
Update assistant message incrementally per token
        ↓
Finalize assistant message
    └── isStreaming: false
        ↓
ChatMessage renders with markdown
```

## Scene Sync Flow

```
Blender (blender_helper_http.py)
    ↓
POST /scene/update every 2 seconds
    ├── object_count
    ├── active_object
    ├── mode
    └── objects[]
        ↓
Rust scene bridge caches scene data
    ↓
Frontend polls /scene/current every 5 seconds
    ↓
blenderStore updates
    ↓
ScenePanel re-renders
```

---

# Build System

## Development

```bash
# Frontend only (Vite dev server)
npm run dev

# Full Tauri app (with hot reload)
npm run tauri dev
```

## Production Build

```bash
# Option 1: Use build script
build_app.bat          # Windows
build_app.sh           # Linux/macOS

# Option 2: Manual
npm install
npm run build
npm run tauri build
```

**Build Output:**
- Executable: `src-tauri/target/release/blender_helper.exe` (~11 MB)
- Installer: `src-tauri/target/release/bundle/msi/` (requires WiX Toolset)

**Bundle Contents:**
- Frontend assets (~5 MB)
- Tauri binary (~30 MB)
- Entire `rag_system/` directory (~15 MB)
  - server.py
  - requirements_server.txt
  - embeddings.npy + metadata.json + metadata.pkl
  - tutorials.json
- `models/` directory for ONNX artifacts (if present)

---

# Dependencies

## Frontend (package.json)

**Production:**
- `@tauri-apps/api` - Tauri JavaScript API
- `clsx` + `tailwind-merge` - Utility class merging
- `lucide-svelte` - Icon library

**Development:**
- `svelte` 5.46.1 - Reactive framework
- `vite` 6.4.1 - Build tool
- `typescript` 5.8.3 - Type checking
- `tailwindcss` 4.1.18 - CSS framework
- `@tailwindcss/vite` - Tailwind Vite plugin
- `prettier` + plugins - Code formatting

## Backend (Cargo.toml)

**Rust Dependencies:**
- `tauri` 2.9.2 - Desktop framework
- `serde` + `serde_json` - JSON serialization
- `ort` + `tokenizers` + `ndarray` - ONNX runtime/model/tokenization stack
- `reqwest` - HTTP client for Ollama fallback backend
- `axum` - HTTP scene bridge for Blender addon compatibility
- `serde-pickle` - Transitional `metadata.pkl` fallback support
- `chrono` - Date/time handling
- `open` - Open URLs/files

## RAG Tooling (Python, Optional)

**Python Dependencies (build-time tooling only):**
- `sentence-transformers`, `numpy`, `beautifulsoup4`, `requests`
- Used by `rag_system/build_database.py` when rebuilding embeddings

---

# Configuration Files

## Vite (`vite.config.ts`)

```typescript
export default defineConfig({
  plugins: [
    svelte({ compilerOptions: { runes: true } }),
    tailwindcss()
  ],
  resolve: {
    alias: { $lib: path.resolve('./src/lib') }
  },
  server: {
    port: 1420,
    strictPort: true
  }
});
```

## Tauri (`src-tauri/tauri.conf.json`)

```json
{
  "productName": "Blender Learning Assistant",
  "version": "4.0.0",
  "identifier": "app.blender.learning",
  "build": {
    "frontendDist": "../dist"
  },
  "bundle": {
    "resources": ["../rag_system/**/*", "../models/**/*"]
  },
  "app": {
    "windows": [{
      "title": "Blender Learning Assistant",
      "width": 1000,
      "height": 750
    }]
  }
}
```

---

# Known Issues & Fixes

## 1. Unicode Emoji in Python Console (FIXED)

**Problem:** Windows console can't display emoji characters (✅)
**Error:** `UnicodeEncodeError: 'charmap' codec can't encode character '\u2705'`
**Fix:** Replaced all emoji with ASCII text `[OK]` in `server.py`

```bash
# Already applied:
sed -i 's/✅/[OK]/g' rag_system/server.py
```

## 2. Slow Dependency Verification (FIXED)

**Problem:** Checking each package individually took 20+ seconds
**Fix:** Consolidated all imports into single Python process

**Before:**
```rust
for package in ["flask", "numpy", "sentence_transformers"] {
    Command::new("python").arg("-c").arg(f"import {package}")
}
```

**After:**
```rust
Command::new("python").arg("-c").arg(r#"
import flask, numpy, sentence_transformers, ...
print("OK")
"#)
```

## 3. Bundle Glob Pattern Error (FIXED)

**Problem:** Negation patterns (`!../rag_system/__pycache__/**`) failed build
**Fix:** Removed exclusion patterns, bundle everything

```json
// Before (failed):
"resources": ["../rag_system/**/*", "!../rag_system/__pycache__/**"]

// After (works):
"resources": ["../rag_system/**/*"]
```

## 4. Server Startup Timeout Too Short (FIXED - Feb 2026)

**Problem:** Sentence-transformers model loading takes 45-50 seconds, but health check timeout was only 30 seconds
**Symptom:** App would show blank window for a few seconds then close itself
**Error (in logs):** "Server failed to start within 30 seconds"

**Root Causes:**
1. TensorFlow backend in sentence-transformers is slow to initialize (~20 seconds)
2. RAG system loads 958 documents which takes additional time (~25 seconds)
3. Total startup time: 45-50 seconds on first run
4. Tauri resource bundling places files in `_up_/rag_system` directory which wasn't being checked

**Fixes Applied:**
1. Increased health check timeout from 30 to 60 seconds in [server_manager.rs](src-tauri/src/server_manager.rs#L40)
2. Fixed `get_rag_directory()` to check `_up_/rag_system` directory (Tauri bundling quirk)
3. Added debug output for path detection during startup
4. Changed health check status messages from every 5 seconds to every 10 seconds
5. Added user-friendly message: "(This may take up to 60 seconds on first run while loading AI models...)"

**Files Modified:**
- `src-tauri/src/server_manager.rs` - Timeout increased to 60s
- `src-tauri/src/main.rs` - Better resource path detection with `_up_/` fallback

## 5. Frontend Not Bundled - "localhost refused to connect" (FIXED - Feb 2026)

**Problem:** Running `cargo build --release` directly doesn't bundle the frontend files
**Symptom:** App window opens but shows "Hmmm... can't reach this page - localhost refused to connect"
**Error:** `ERR_CONNECTION_REFUSED` when trying to load `localhost:1420`

**Root Cause:**
- `cargo build` only compiles the Rust backend
- The frontend (`dist/` directory) is not copied into the Tauri bundle
- App tries to load from dev server (localhost:1420) which isn't running
- Tauri's `beforeBuildCommand` only runs with `tauri build`, not `cargo build`

**Fix:**
Always use `npm run tauri build` instead of `cargo build --release` directly

**Correct Build Process:**
```bash
npm install           # Install dependencies
npm run build        # Build frontend → dist/
npm run tauri build  # Build Tauri app with bundled frontend
```

**Build Scripts Updated:**
- `build_app.bat` and `build_app.sh` already use correct process
- Added warning in documentation about not using cargo directly

## 6. Tutorial Tab Stuck/Unresponsive (FIXED - Feb 2026)

**Problem:** Tutorial tab would get stuck and prevent switching back to other tabs
**Symptom:** Users couldn't navigate away from tutorial view once opened
**User Request:** Remove tutorial functionality entirely

**Solution:**
Completely removed the tutorial tab and all related functionality:
- Removed tutorial tab from navigation
- Removed `tutorialsStore` import and initialization
- Removed `TutorialCard`, `TutorialViewer`, and `Button` components
- Removed tutorial-related state management
- Removed tutorial content section (lines 395-437)

**Benefits:**
- Simpler, more focused UI (Chat + Suggestions only)
- Smaller bundle size: 115 KB → 96 KB (~16% reduction)
- No navigation issues
- Faster load times

**Files Modified:**
- `src/App.svelte` - Removed all tutorial functionality
- Tab type changed from `'chat' | 'suggestions' | 'tutorials'` to `'chat' | 'suggestions'`

## 7. Security & Code Quality Audit (FIXED - Feb 2026)

A comprehensive security and code quality audit was performed, identifying and fixing 23 issues across critical, high, and medium priorities.

### Critical Security Fixes

#### 7.1 XSS Vulnerability in Markdown Parser
**File:** `src/lib/utils/markdown.ts`

**Problem:** The regex-based markdown parser had potential XSS bypass vectors. Links didn't validate protocols, allowing `javascript:` URLs.

**Solution:** Added three-layer security:
- `sanitizeUrl()` - Blocks dangerous protocols (`javascript:`, `data:`, `vbscript:`, `file:`)
- `escapeHtml()` - Escapes all HTML special characters (`& < > " '`)
- `sanitizeAttribute()` - Prevents attribute injection in code blocks

#### 7.2 CORS Configuration Too Permissive
**File:** `rag_system/server.py`

**Problem:** `CORS(app)` enabled all origins without restriction.

**Solution:**
```python
CORS(app, origins=[
    'http://127.0.0.1:*',
    'http://localhost:*',
    'tauri://localhost'
])
```

#### 7.3 Input Validation Missing
**File:** `rag_system/server.py`

**Problem:** API endpoints accepted JSON without validation.

**Solution:** Added comprehensive validation to `/ask`, `/scene_analysis`, `/tutorial/step`, and `/scene/update` endpoints:
- JSON content-type checking
- Type validation (string, int, dict)
- Length limits (10,000 chars for questions, 500 for goals)
- Range validation for step numbers

### High Priority Fixes

#### 7.4 Unsafe `unwrap()` in Rust
**File:** `src-tauri/src/server_manager.rs`

**Problem:** `unwrap()` on `try_clone()` could panic and crash the app.

**Solution:** Proper error handling with `map_err()`:
```rust
let stdout_log = log.try_clone()
    .map_err(|e| format!("Failed to clone log file for stdout: {}", e))?;
```

#### 7.5 Hardcoded Port 5000 → 5179
**Files:** `server_manager.rs`, `api.ts`, `server.py`, `blender_helper_http.py`

**Problem:** Port 5000 conflicts with AirPlay on macOS.

**Solution:** Changed to port 5179 across all components.

#### 7.6 Race Condition in Chat Store
**File:** `src/lib/stores/chats.svelte.ts`

**Problem:** Mutating chat objects in place caused Svelte 5 reactivity issues.

**Solution:** Immutable updates with `chats = chats.map(...)` pattern.

#### 7.7 Blocking HTTP in Blender UI Thread
**File:** `blender_addon/blender_helper_http.py`

**Problem:** `check_server_health()` called in `draw()` blocked UI for up to 5 seconds.

**Solution:** Added health cache system:
- `_server_health_cache` - Global cache updated by timer
- `get_cached_server_health()` - Non-blocking cache reader
- `draw()` only reads from cache

#### 7.8 `any` Type Usage in TypeScript
**File:** `src/lib/utils/api.ts`, `src/lib/types/rag.ts`

**Problem:** Functions returned `Promise<any>`, losing type safety.

**Solution:** Added proper interfaces:
- `TutorialListResponse`
- `TutorialStepResponse`
- `SceneResponse`
- `HealthResponse` (corrected to match server)

#### 7.9 Missing Error Boundary
**File:** `src/App.svelte`

**Problem:** Unhandled errors crashed the entire app.

**Solution:**
- Added `appError` state
- Try-catch in all `$effect` blocks
- Global error banner UI with dismiss button

#### 7.10 Division by Zero in Cosine Similarity
**File:** `rag_system/server.py`

**Problem:** Zero-norm embeddings caused `nan`/`inf` values.

**Solution:**
```python
norms = np.where(norms == 0, 1, norms)
if query_norm == 0:
    query_norm = 1
```

### Medium Priority Fixes

#### 7.11 Console Logs in Production
**File:** `src/lib/utils/api.ts`

**Solution:** Conditional logging with `import.meta.env.DEV`.

#### 7.12 LocalStorage Quota Handling
**File:** `src/lib/utils/storage.ts`

**Solution:**
- `saveToStorage()` returns boolean
- Added `QuotaExceededError` handling
- Added `getStorageSize()` and `hasStorageSpace()` functions

#### 7.13 Exponential Backoff for Health Check
**File:** `src-tauri/src/server_manager.rs`

**Solution:** Changed from fixed 1s intervals to exponential backoff (1s → 2s → 4s → 5s cap).

#### 7.14 Type Guard in Blender Store
**File:** `src/lib/stores/blender.svelte.ts`

**Solution:** Added `isValidSceneData()` runtime type guard.

#### 7.15 Scene Data Validation
**File:** `rag_system/server.py`

**Solution:** Added validation for `/scene/update`:
- 1MB size limit
- Field type validation
- Nested object validation

#### 7.16 Standardized Error Messages
**Files:** `server.py`, `main.rs`, `server_manager.rs`, `python_checker.rs`

**Solution:** Consistent format: `[Component] Level: Message`
- `[RAG] OK: Successfully loaded 958 documents`
- `[Server] Error: Failed to spawn server`
- `[Python] Warning: Python 3.10+ not found`

#### 7.17 Unused Import Cleanup
**File:** `src-tauri/src/logger.rs`

**Solution:** Moved `use std::process::Command;` to top-level imports.

#### 7.18 Vite Environment Types
**File:** `src/vite-env.d.ts` (NEW)

**Solution:** Added TypeScript types for `import.meta.env.DEV`.

### UI Fixes

#### 7.19 Duplicate Close Buttons
**Files:** `src/App.svelte`, `src/lib/components/Sidebar.svelte`

**Problem:** Two X icons appeared on sidebar.

**Solution:** Removed redundant close button from sidebar header.

#### 7.20 Spacing Issues
**Files:** Multiple components

**Solution:** Comprehensive spacing fixes:
- Sidebar width: `w-100` → `w-72`
- Header/tabs padding adjustments
- Chat message spacing improvements
- Consistent 4px-based spacing scale

#### 7.21 Fullscreen Header Spacing
**File:** `src/App.svelte`

**Problem:** Header content appeared far from edges on large screens.

**Solution:** Removed `max-w-7xl mx-auto` from header and tabs containers.

---

# Testing Checklist

## ✅ Unified Launcher Tests

- [x] Python detection works (py -3, python3, python)
- [x] Dependency verification (all packages checked in single process)
- [x] Dependency caching (fast subsequent launches)
- [x] Server starts successfully
- [x] Health check passes within 30 seconds
- [x] Console window hidden on Windows
- [x] Logs captured to file
- [x] Graceful server shutdown on app exit
- [x] Loading screen displays during startup

## ✅ Frontend Tests

- [x] Multiple chat sessions
- [x] Time-grouped chat history
- [x] Message persistence (localStorage)
- [x] Chat deletion
- [x] Auto-scroll behavior
- [x] Dark/light theme toggle
- [x] Tab navigation (Chat, Suggestions, Tutorials)
- [x] Status indicators update correctly

## ✅ Integration Tests

- [x] Send message to RAG server
- [x] Receive and display response
- [x] Scene panel displays Blender data
- [x] Suggestions generate from scene
- [x] Tutorials load and display
- [x] Tutorial validation works

---

# Development Workflow

## Starting Development

```bash
# Start Tauri app with hot reload (Rust scene bridge starts automatically)
npm run tauri dev
```

## Making Changes

**Frontend Changes:**
- Edit files in `src/`
- Vite hot-reloads automatically
- Check browser console for errors

**Backend Changes:**
- Edit files in `src-tauri/src/`
- Save triggers Rust recompilation
- App restarts automatically

**RAG Data Changes:**
- Edit `rag_system/build_database.py` or source docs
- Rebuild embeddings/metadata when needed

## Type Checking

```bash
# Check TypeScript + Svelte types
npm run check

# Check Rust code
cd src-tauri && cargo check
```

## Building for Distribution

**IMPORTANT:** Always use `npm run tauri build` or the build scripts - **never** use `cargo build` directly!
Direct cargo builds will not bundle the frontend, resulting in "localhost refused to connect" errors.

```bash
# Option 1: Use build script (recommended)
build_app.bat          # Windows
./build_app.sh         # Linux/macOS

# Option 2: Manual build
npm install
npm run build          # Build frontend first
npm run tauri build    # Then build Tauri with bundling

# ❌ WRONG - Don't do this!
# cd src-tauri && cargo build --release  # This won't bundle the frontend!
```

---

# Deployment

## System Requirements

**User Machine:**
- Windows 10+, macOS, or Linux
- 4 GB RAM minimum
- ~500 MB disk space (including model cache/assets)

**External Services:**
- None required when ONNX model assets are bundled/installed
- Optional fallback: Ollama with `qwen2.5:7b-instruct-q4_K_M`

## Distribution

**Option 1: Standalone Executable**
- Distribute `blender_helper.exe` from `src-tauri/target/release/`
- Includes Rust orchestration + bundled RAG assets

**Option 2: MSI Installer (Requires WiX Toolset)**
- Install WiX Toolset 3.14+
- Run `npm run tauri build`
- Installer at `src-tauri/target/release/bundle/msi/`
- Cleaner installation experience

## First Launch Experience

1. User runs `blender_helper.exe`
2. Rust runtime initializes RAG index
3. Scene bridge starts on `127.0.0.1:5179`
4. Health/status checks complete
5. Main window appears

---

# Troubleshooting

## "Server Failed to Start"

**Check Logs:**
```
Windows: %APPDATA%\blender_helper\logs\server_YYYY-MM-DD.log
Linux: ~/.local/share/blender_helper/logs/
macOS: ~/Library/Application Support/blender_helper/logs/
```

**Common Causes:**
- Port 5179 already in use
- ONNX model files missing or not loaded
- Ollama not running (only if fallback backend is selected)

---

# Future Enhancements

## Planned Improvements

1. **Auto-Update System**
   - Check GitHub releases on launch
   - Download and install updates

2. **Tier 3 Quality/Model Profiles**
   - Add downloadable larger ONNX model profiles
   - Keep small model as default for low-resource machines
   - Benchmark first-token latency and tokens/sec per profile

3. **Advanced Markdown**
   - Code syntax highlighting
   - Math equation rendering

4. **Keyboard Shortcuts**
   - Ctrl+N: New chat
   - Ctrl+K: Search chats
   - Ctrl+/: Toggle sidebar

5. **Export Features**
   - Export chat as markdown
   - Export chat as JSON
   - Share chat via URL

6. **Real-Time Scene Sync**
   - WebSocket connection to Blender
   - Live object updates
   - Viewport screenshot capture

---

# Migration History

## Svelte 5 Migration (Jan 2026)

**From:** Vanilla JavaScript + Custom CSS
**To:** Svelte 5 + Tailwind CSS 4

**Duration:** ~6 hours
**Files Created:** 33
**Files Modified:** 2
**Status:** ✅ Complete

**Key Changes:**
- Reactive state with Svelte runes
- Component-based architecture
- Type-safe TypeScript
- Utility-first CSS with Tailwind
- Multiple chat sessions with history
- Time-grouped chat organization

## Unified Launcher (Feb 2026)

**Goal:** Single-click executable that starts everything

**Duration:** ~3 hours
**Files Created:** 6 (3 Rust modules, 2 build scripts, 1 guide)
**Files Modified:** 5
**Status:** ✅ Complete

**Key Features:**
- Python detection + dependency management
- Hidden console windows (Windows)
- Health check system
- Loading screen
- Graceful shutdown
- Log file capture

## Security & Code Quality Audit (Feb 2026)

**Goal:** Comprehensive security hardening and code quality improvements

**Duration:** ~4 hours
**Issues Identified:** 23 (3 critical, 8 high, 12 medium)
**Issues Fixed:** 21 (3 critical, 8 high, 10 medium)
**Status:** ✅ Complete

**Key Changes:**
- XSS protection in markdown parser (URL sanitization, HTML escaping)
- CORS restricted to localhost origins only
- Input validation on all API endpoints
- Port changed from 5000 to 5179 (macOS AirPlay conflict)
- Immutable state updates in Svelte 5 stores
- Non-blocking health cache for Blender addon UI
- Proper TypeScript types (removed all `any` usage)
- Error boundary for graceful error handling
- Division by zero protection in cosine similarity
- Exponential backoff for health checks
- LocalStorage quota handling
- Runtime type guards for API responses
- Standardized error message format across all components
- UI fixes: duplicate buttons, spacing issues, fullscreen header

**Files Modified:** 14
**New Files Created:** 1 (`src/vite-env.d.ts`)

---

# Reference Documentation

## Internal Documentation
- [INSTALL.md](INSTALL.md) - Installation & troubleshooting guide
- [README.md](README.md) - User-facing documentation
- [GETTING_STARTED.md](GETTING_STARTED.md) - Quick start guide

## External Resources
- [Svelte 5 Docs](https://svelte.dev/docs/svelte/overview)
- [Tailwind CSS 4 Docs](https://tailwindcss.com/docs)
- [Tauri 2 Docs](https://v2.tauri.app/)
- [ONNX Runtime Docs](https://onnxruntime.ai/docs/)
- [Ollama Docs](https://ollama.com/library) (fallback backend)

---

**Last Updated:** 2026-02-10 (Tier 3 ONNX Backend Scaffolding Complete)
**Maintained by:** Claude Code Sessions
**Version:** 6.0.0 - ONNX Backend Scaffolding + Rust Orchestration
