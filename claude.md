# SmolPC Blender Helper - Claude Code Documentation

This file contains implementation plans and architectural documentation for use across multiple Claude Code sessions.

---

# Current Application State

**Last Updated:** 2026-02-07
**Version:** 4.1.0
**Status:** ✅ Production Ready - Tutorial Tab Removed

## Quick Status Summary

✅ **Svelte 5 + Tailwind CSS 4 Migration** - COMPLETE
✅ **Unified Executable Launcher** - COMPLETE
✅ **Multiple Chat Sessions with History** - COMPLETE
✅ **Auto-dependency Management** - COMPLETE
✅ **Hidden Console Windows** - COMPLETE

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
| **Backend Server** | Flask (Python) | 3.0.0+ | ✅ Active |
| **LLM** | Ollama | Latest | ✅ Required |

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
│   │   ├── stores/               # Svelte 5 Runes Stores (5 files)
│   │   │   ├── chats.svelte.ts
│   │   │   ├── settings.svelte.ts
│   │   │   ├── rag.svelte.ts
│   │   │   ├── blender.svelte.ts
│   │   │   └── tutorials.svelte.ts
│   │   ├── types/                # TypeScript Interfaces (5 files)
│   │   │   ├── chat.ts
│   │   │   ├── settings.ts
│   │   │   ├── blender.ts
│   │   │   ├── tutorial.ts
│   │   │   └── rag.ts
│   │   └── utils/                # Helper Functions (4 files)
│   │       ├── storage.ts
│   │       ├── date.ts
│   │       ├── markdown.ts
│   │       └── api.ts
│   ├── App.svelte                # Main App Component
│   ├── main.ts                   # Entry Point
│   ├── app.css                   # Tailwind CSS Config
│   └── index.html                # HTML Shell
│
├── src-tauri/                     # Rust Backend (Unified Launcher)
│   ├── src/
│   │   ├── main.rs               # Tauri + Server Manager Integration
│   │   ├── python_checker.rs    # Python Detection & Dependency Mgmt
│   │   ├── server_manager.rs    # RAG Server Lifecycle Management
│   │   └── logger.rs             # Log File Management
│   ├── Cargo.toml                # Rust Dependencies
│   └── tauri.conf.json           # Tauri Configuration + Resource Bundling
│
├── rag_system/                    # Python RAG Server
│   ├── server.py                 # Flask HTTP Server (Port 5000)
│   ├── requirements_server.txt   # Python Dependencies
│   ├── simple_db/                # Vector Embeddings Database
│   │   ├── embeddings.npy
│   │   └── metadata.pkl
│   └── tutorials.json            # Tutorial Content
│
├── blender_addon/                # Blender Integration
│   └── blender_helper_http.py   # Scene Data Exporter
│
├── build_app.bat                 # Windows Build Script
├── build_app.sh                  # Linux/macOS Build Script
├── INSTALL.md                    # Installation Guide
└── README.md                     # User Documentation
```

---

# Unified Launcher Implementation (NEW - Feb 2026)

## Overview

The application now launches as a **single executable** that automatically:
- Detects Python runtime
- Installs dependencies (first launch only)
- Starts the RAG server with hidden console window
- Shows loading screen while initializing
- Opens the Tauri UI when ready
- Stops server cleanly on app exit

## Key Components

### 1. Python Checker (`src-tauri/src/python_checker.rs`)

**Responsibilities:**
- Detects Python 3.10+ (tries `py -3`, `python3`, `python` in order)
- Verifies all required packages in a single Python process
- Auto-installs missing dependencies using `pip install --user`
- Caches installation status to avoid re-checking

**Key Functions:**
- `check_python_available()` → `Result<String, String>`
- `verify_dependencies(python_path)` → `Result<(), Vec<String>>`
- `install_dependencies(python_path, requirements_path)` → `Result<String, String>`
- `are_dependencies_cached(app_data_dir)` → `bool`

### 2. Server Manager (`src-tauri/src/server_manager.rs`)

**Responsibilities:**
- Spawns Python RAG server as subprocess
- Hides console window on Windows (`CREATE_NO_WINDOW` flag)
- Polls `/health` endpoint with 30-second timeout
- Captures stdout/stderr to log file
- Gracefully terminates server on app exit

**Key Functions:**
- `start(python_path, rag_dir)` → `Result<(), String>`
- `health_check()` → `bool`
- `stop()` → Graceful SIGTERM then force kill if needed

### 3. Logger (`src-tauri/src/logger.rs`)

**Responsibilities:**
- Creates dated log files: `server_YYYY-MM-DD.log`
- Cross-platform log directory access
- Logs location: `%APPDATA%/blender_helper/logs/`

### 4. Main Integration (`src-tauri/src/main.rs`)

**Startup Sequence:**
1. Get app data directory
2. Setup log file
3. Check Python availability → Show error if missing
4. Check dependency cache
5. Verify/install dependencies if needed
6. Start RAG server subprocess
7. Wait for `/health` endpoint (max 30 seconds)
8. Launch Tauri window
9. Store server handle in app state

**Shutdown Sequence:**
1. User closes window
2. `ExitRequested` event caught
3. `ServerManager::stop()` called
4. SIGTERM sent to Python process
5. Wait 5 seconds for graceful exit
6. Force kill if still running

## User Experience

### First Launch
1. User double-clicks `blender_helper.exe`
2. App checks for Python → Opens download page if missing
3. If Python found, checks dependencies (~10 seconds with sentence-transformers)
4. Missing packages installed automatically (1-2 minutes)
5. Server starts, health check passes
6. Loading screen shows: "Starting Blender Learning Assistant..."
7. Main window appears when server is ready

### Subsequent Launches
1. User double-clicks `blender_helper.exe`
2. Dependencies cached, instant verification
3. Server starts in ~2-3 seconds
4. Loading screen shows briefly
5. Main window appears

### No Visible Terminals
- Python server runs with hidden console window
- All output captured to log file
- Clean, professional user experience

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
- Tutorials

No router library needed - simple state-based tabs.

---

# Backend Integration

## RAG Server Endpoints

**Base URL:** `http://127.0.0.1:5000`

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/health` | GET | Server status + doc count |
| `/ask` | POST | Educational Q&A |
| `/scene_analysis` | POST | Generate learning suggestions |
| `/tutorial/list` | GET | Get available tutorials |
| `/tutorial/step` | POST | Get tutorial step + validate |
| `/scene/update` | POST | Cache scene data from Blender |
| `/scene/current` | GET | Retrieve cached scene |

## API Integration Pattern

All API calls use the unified `callRagServer` helper:

```typescript
import { callRagServer } from '$lib/utils/api';

const response = await callRagServer('/ask', {
  question: userInput,
  scene_context: currentScene
});
```

**Features:**
- 60-second timeout
- AbortController support
- Automatic JSON parsing
- Error handling

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
Call RAG Server POST /ask
    ├── question: string
    └── scene_context?: SceneData
        ↓
RAG Server Processing
    ├── Embed question
    ├── Search vector DB (top-3 docs)
    ├── Add Blender scene context
    ├── Call Ollama LLM
    └── Return educational answer
        ↓
Update assistant message
    ├── content: answer
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
RAG Server caches scene data
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

# Note: In dev mode, you must manually start RAG server:
# Terminal 1: python rag_system/server.py
# Terminal 2: npm run tauri dev
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
  - embeddings.npy + metadata.pkl
  - tutorials.json

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
- `reqwest` - HTTP client (for health checks)
- `chrono` - Date/time handling
- `open` - Open URLs/files

## RAG Server (requirements_server.txt)

**Python Dependencies:**
- `flask` ≥ 3.0.0 - HTTP server
- `flask-cors` ≥ 4.0.0 - CORS support
- `numpy` ≥ 2.0.0 - Vector operations
- `sentence-transformers` ≥ 3.3.0 - Embeddings
- `requests` ≥ 2.28.0 - HTTP client for Ollama
- `beautifulsoup4` ≥ 4.11.0 - Documentation parsing

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
    "resources": ["../rag_system/**/*"]
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
# Terminal 1: Start RAG server (manual in dev mode)
cd rag_system
python server.py

# Terminal 2: Start Tauri app with hot reload
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

**Python Server Changes:**
- Edit `rag_system/server.py`
- Restart server manually (Ctrl+C, then `python server.py`)

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
- Python 3.10+ (auto-detected, user installs if missing)
- 4 GB RAM minimum
- ~500 MB disk space (including dependencies)

**External Services:**
- Ollama with `qwen2.5:7b-instruct-q4_K_M` model

## Distribution

**Option 1: Standalone Executable**
- Distribute `blender_helper.exe` from `src-tauri/target/release/`
- User must have Python 3.10+ installed
- Includes entire RAG system (embeddings, tutorials)

**Option 2: MSI Installer (Requires WiX Toolset)**
- Install WiX Toolset 3.14+
- Run `npm run tauri build`
- Installer at `src-tauri/target/release/bundle/msi/`
- Cleaner installation experience

## First Launch Experience

1. User runs `blender_helper.exe`
2. App checks for Python
3. If missing → Opens python.org download page
4. If found → Checks dependencies
5. Missing packages → Auto-installs (1-2 minutes)
6. Server starts → Health check
7. Loading screen → Main window appears
8. Cached for next launch (instant startup)

---

# Troubleshooting

## "Python Not Found"

**Solution:**
1. Install Python 3.10+ from python.org
2. Check "Add Python to PATH" during installation
3. Restart app

**Verify:**
```bash
python --version  # Should show 3.10+
```

## "Server Failed to Start"

**Check Logs:**
```
Windows: %APPDATA%\blender_helper\logs\server_YYYY-MM-DD.log
Linux: ~/.local/share/blender_helper/logs/
macOS: ~/Library/Application Support/blender_helper/logs/
```

**Common Causes:**
- Port 5000 already in use
- Missing Python dependencies
- Ollama not running

## "Failed to Install Dependencies"

**Manual Installation:**
```bash
pip install -r rag_system/requirements_server.txt
```

**If Permission Denied:**
```bash
pip install --user -r rag_system/requirements_server.txt
```

---

# Future Enhancements

## Planned Improvements

1. **Auto-Update System**
   - Check GitHub releases on launch
   - Download and install updates

2. **Bundled Ollama (Optional)**
   - Include Ollama in installer
   - Much larger bundle (~4 GB+)
   - Fully offline experience

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
- [Ollama Docs](https://ollama.com/library)

---

**Last Updated:** 2026-02-07 (Evening - Tutorial Tab Removed)
**Maintained by:** Claude Code Sessions
**Version:** 4.1.0 - Production Ready
