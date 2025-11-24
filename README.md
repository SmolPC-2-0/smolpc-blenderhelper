# Blender Helper

Local Blender modeling copilot powered by a Tauri desktop app, a Blender add‑on, and a local Ollama LLM.

> Status: early experiment – expect bugs and rough edges.

## Overview

- Type a high‑level goal (for example, “create a low‑poly duck on a plane”).
- Click **Next Step** to get a numbered plan you can follow in Blender.
- Click **Do It** to have code generated that builds the object/scene in Blender.
- A Blender add‑on can call the same local HTTP API and execute the generated `bpy` code for you.
- Recent actions are remembered in a short in‑memory log so the assistant can stay consistent within one session.

## Components

- **Tauri desktop app** (`src-tauri`, UI in `src/index.html` & `src/main.js`)  
  Opens a small window with a Goal input and the **Next Step** / **Do It** buttons.
- **Local HTTP server** (`src-tauri/src/main.rs`, `src-tauri/src/blender_bridge.rs`)  
  Runs on `http://127.0.0.1:17890` and exposes:
  - `POST /blender/next_step` → `{ step: string }`
  - `POST /blender/run_macro` → `{ code: string }`
  - `POST /blender/remember` → `{ ok: true }`
- **Blender add‑on** (`blender_addon/blender_helper.py`)  
  Adds a “Blender Helper” panel in the 3D Viewport sidebar with the same Goal / Next Step / Do It flow.
- **Local LLM via Ollama** (`src-tauri/src/ollama.rs`)
  Calls `http://127.0.0.1:11434/api/chat` and defaults to the `qwen2.5-coder:32b` model unless `OLLAMA_MODEL` is set.

All traffic is local: Blender and the Tauri UI only talk to the HTTP server on `127.0.0.1`, and the server only talks to the local Ollama instance.

## Requirements

- Blender 3.x or later (the prompts target Blender 4.x terminology).
- Rust and Cargo installed (for building the Tauri backend).
- Node.js and `npm` (to run the `@tauri-apps/cli` from `package.json`).
- A running **Ollama** instance with `qwen2.5-coder:32b` or another capable code model.
- In Blender's Python environment, the `requests` package must be available (the add‑on imports it).

## Setup & Running

### 1. Install JavaScript and Rust dependencies

From the project root:

```bash
npm install
```

This installs the Tauri CLI used by the project. Rust dependencies are handled automatically by Cargo the first time the app is built or run.

### 2. Start Ollama with Qwen2.5-Coder

Make sure Ollama is installed and running on the same machine, listening on the default `127.0.0.1:11434`.

1. Download and install Ollama from its official site.
2. Pull the recommended model:

```bash
ollama pull qwen2.5-coder:32b
```

This model provides excellent code generation quality for complex 3D modeling tasks.

**Alternative models** (if you have less RAM):
- `qwen2.5-coder:14b` - Good balance of quality and resource usage
- `deepseek-coder-v2:16b` - Another strong code-focused model
- `llama3.1:8b` - Lighter weight option (may require more sanitization fixes)

Optional environment variable:

- `OLLAMA_MODEL` – name of the model to use (defaults to `qwen2.5-coder:32b` if not set).

### 3. Run the Tauri app (UI + HTTP server)

From the project root:

```bash
npm run tauri dev
```

This will:

- Build and run the Rust crate in `src-tauri`.
- Start the local HTTP API on `http://127.0.0.1:17890`.
- Open the “Blender Helper” desktop window (using `src/index.html` & `src/main.js` as the frontend).

Leave this process running while you use the Blender add‑on.

To build a packaged application instead of running in dev mode:

```bash
npm run tauri build
```

### 4. Install the Blender add‑on

1. Open Blender.
2. Go to `Edit` → `Preferences…` → `Add-ons`.
3. Click **Install…**.
4. Select `blender_addon/blender_helper.py` from this repository.
5. Enable the add‑on named **“Blender Helper AI Link”**.

If Blender reports `ModuleNotFoundError: requests`, install `requests` into Blender’s Python:

```bash
# from a shell, using Blender's bundled python
path\to\blender\python.exe -m pip install requests
```

Restart Blender after installing if needed.

## Usage

### From Blender

1. Ensure the Tauri app (step 3 above) is running so the HTTP server is available on `127.0.0.1:17890`.
2. In Blender, open a 3D Viewport and press **N** to show the right‑hand sidebar.
3. Go to the **Blender Helper** tab / panel.
4. Enter a high‑level goal in the **Goal** field, for example:
   - `Create a low-poly duck on a plane`
   - `Make a simple sci-fi corridor with emissive panels`
5. Click:
   - **Next Step** – sends your goal to `/blender/next_step` and shows a numbered list of steps as an info message.
   - **Do It** – first tries a built‑in “QuickBuilder” for common primitives (cube, plane, cylinder, low‑poly duck, etc.).  
     If that can’t handle the request, it calls `/blender/run_macro`, sanitizes the returned `bpy` script, and executes it in the current scene.

The add‑on also sends short “memory” events to `/blender/remember` so the backend can keep some lightweight session context.

### From the desktop window

You can use the Tauri window on its own without Blender:

1. Run `npm run tauri dev`.
2. Type a goal into the **Goal** field.
3. Click **Next Step** or **Do It**.
4. The result (steps or Python code) appears in the output box in the window.

For **Do It**, the code is not executed automatically anywhere; you can copy/paste it into Blender’s Text Editor and run it there if you want to test it manually.

## Configuration & Internals (optional)

- The HTTP API is implemented in `src-tauri/src/main.rs` and `src-tauri/src/blender_bridge.rs` using Axum.
- LLM calls are defined in `src-tauri/src/ollama.rs`:
  - Uses a single non‑streaming `/api/chat` call against Ollama.
  - Honors `OLLAMA_MODEL`; otherwise falls back to `qwen2.5-coder:32b`.
  - Uses temperature 0.7 to balance creativity and consistency.
- The Blender add‑on:
  - Lives in `blender_addon/blender_helper.py`.
  - Provides operators `ai.next_step` and `ai.do_it` and a `BLENDERHELPER_PT_panel` UI panel.
  - Includes a `QuickBuilder` path for robust, non‑LLM creation of common primitives and a low‑poly duck.

## Caveats

- This project is still under active development and may contain bugs.
- Generated scripts should generally be safe, but always save your Blender file and inspect the code if you are unsure.
- The in‑memory “conversation” context is per‑process only; it resets each time you restart the Tauri app.
