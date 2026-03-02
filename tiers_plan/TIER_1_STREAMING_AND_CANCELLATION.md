# Tier 1: Streaming Responses + Cancellation

**Priority:** HIGH
**Effort:** Medium (2-3 sessions)
**Impact:** Immediate UX improvement - tokens appear in real-time instead of waiting 2-5s for full response

---

## Overview

Port the CodeHelper's Tauri Channel streaming pattern and AtomicBool cancellation system to BlenderHelper. Currently, the BlenderHelper calls the Flask server which calls Ollama with `"stream": false`, waits for the entire response, then sends it all at once. This tier makes responses stream token-by-token and adds a cancel button.

**Current flow (BlenderHelper):**
```
Frontend → fetch POST /ask → Flask → Ollama (stream:false, 2-5s wait) → full response back
```

**Target flow (after Tier 1):**
```
Frontend → Tauri Channel → Rust command → HTTP stream from Flask → tokens piped to Channel → live UI updates
```

---

## Source Reference (CodeHelper patterns to port)

These are the exact files and patterns from `smolpc-codehelper` to adapt:

| CodeHelper File | Pattern to Port | Adaptation Needed |
|---|---|---|
| `src-tauri/src/commands/inference.rs:38-104` | `InferenceState`, `GenerationPermit`, `AtomicBool` cancellation | Rename to `OllamaState`, same pattern |
| `src-tauri/src/commands/inference.rs:282-333` | `inference_generate` with `Channel<String>` param | Adapt to stream from Ollama HTTP instead of ONNX |
| `src-tauri/src/commands/inference.rs:336-350` | `inference_cancel` command | Nearly identical |
| `src/lib/stores/inference.svelte.ts:147-209` | `generateStream()` with `Channel<string>` creation | Adapt for BlenderHelper's chat flow |
| `src/lib/stores/inference.svelte.ts:214-224` | `cancel()` method | Nearly identical |

---

## Step-by-Step Implementation Plan

### Step 1: Add Rust Dependencies

**File:** `src-tauri/Cargo.toml`

Add `tokio` for async runtime and update `reqwest` to support streaming:

```toml
[dependencies]
tauri = { version = "2", features = ["protocol-asset", "config-toml"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
reqwest = { version = "0.12", features = ["json", "stream"] }  # ADD "stream" feature
chrono = "0.4"
open = "5"
tokio = { version = "1", features = ["full"] }                  # ADD tokio
log = "0.4"                                                      # ADD logging
env_logger = "0.11"                                              # ADD logging
futures-util = "0.3"                                             # ADD for StreamExt on reqwest response
```

**Why:** `reqwest` with `"stream"` feature gives us `response.bytes_stream()` which lets us read Ollama's streaming JSON responses chunk by chunk. `tokio` is needed for async Tauri commands. `futures-util` provides `StreamExt` for `.next()` on the byte stream.

---

### Step 2: Create Ollama Streaming Module

**New file:** `src-tauri/src/ollama.rs`

This module handles the HTTP streaming connection to Ollama. It replaces the Python Flask middleman for the inference call.

```rust
// src-tauri/src/ollama.rs
//
// Handles streaming HTTP calls to the local Ollama API.
// Ollama's streaming format: one JSON object per line, each containing a partial response.
//
// Ollama streaming response format (one per line):
// {"model":"qwen2.5:7b-instruct-q4_K_M","message":{"role":"assistant","content":"To"},"done":false}
// {"model":"qwen2.5:7b-instruct-q4_K_M","message":{"role":"assistant","content":" add"},"done":false}
// ...
// {"model":"qwen2.5:7b-instruct-q4_K_M","message":{"role":"assistant","content":""},"done":true,"total_duration":...}

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const OLLAMA_URL: &str = "http://127.0.0.1:11434/api/chat";
const DEFAULT_MODEL: &str = "qwen2.5:7b-instruct-q4_K_M";

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    stream: bool,
    messages: Vec<OllamaMessage>,
    options: OllamaOptions,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
struct OllamaOptions {
    temperature: f64,
}

#[derive(Deserialize)]
struct OllamaStreamChunk {
    message: Option<OllamaChunkMessage>,
    done: bool,
    // These fields only appear on the final chunk (done=true):
    total_duration: Option<u64>,
    eval_count: Option<u64>,
    eval_duration: Option<u64>,
}

#[derive(Deserialize)]
struct OllamaChunkMessage {
    content: String,
}

/// Metrics returned after generation completes
#[derive(Serialize, Clone)]
pub struct OllamaMetrics {
    pub total_tokens: u64,
    pub total_time_ms: u64,
    pub tokens_per_second: f64,
}

/// Stream a chat completion from Ollama, calling `on_token` for each text chunk.
///
/// Returns metrics on success. Checks `cancelled` flag between chunks.
///
/// # Arguments
/// * `system_prompt` - System message content
/// * `user_prompt` - User message content
/// * `cancelled` - AtomicBool flag; set to true to abort streaming
/// * `on_token` - Callback invoked with each text fragment
pub async fn stream_chat<F>(
    system_prompt: &str,
    user_prompt: &str,
    cancelled: Arc<AtomicBool>,
    mut on_token: F,
) -> Result<OllamaMetrics, String>
where
    F: FnMut(String),
{
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(180)) // 3 min total timeout
        .build()
        .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

    let request_body = OllamaChatRequest {
        model: std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string()),
        stream: true,  // KEY CHANGE: stream=true
        messages: vec![
            OllamaMessage { role: "system".to_string(), content: system_prompt.to_string() },
            OllamaMessage { role: "user".to_string(), content: user_prompt.to_string() },
        ],
        options: OllamaOptions { temperature: 0.7 },
    };

    let response = client
        .post(OLLAMA_URL)
        .json(&request_body)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                "Ollama not running. Start it with: ollama serve".to_string()
            } else {
                format!("Ollama request failed: {}", e)
            }
        })?;

    if !response.status().is_success() {
        return Err(format!("Ollama returned HTTP {}", response.status()));
    }

    // Stream the response body line by line
    let mut stream = response.bytes_stream();
    let mut total_tokens: u64 = 0;
    let mut buffer = String::new();
    let start = std::time::Instant::now();

    while let Some(chunk_result) = stream.next().await {
        // Check cancellation between chunks
        if cancelled.load(Ordering::Relaxed) {
            return Err("GENERATION_CANCELLED: Generation cancelled by user".to_string());
        }

        let chunk_bytes = chunk_result
            .map_err(|e| format!("Stream read error: {}", e))?;

        // Ollama sends newline-delimited JSON. A single TCP chunk may contain
        // partial lines or multiple lines, so we buffer and split on newlines.
        buffer.push_str(&String::from_utf8_lossy(&chunk_bytes));

        // Process complete lines
        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim().to_string();
            buffer = buffer[newline_pos + 1..].to_string();

            if line.is_empty() {
                continue;
            }

            match serde_json::from_str::<OllamaStreamChunk>(&line) {
                Ok(chunk) => {
                    if let Some(msg) = &chunk.message {
                        if !msg.content.is_empty() {
                            total_tokens += 1; // Approximate: each chunk ~= 1 token
                            on_token(msg.content.clone());
                        }
                    }

                    if chunk.done {
                        // Use Ollama's own metrics if available
                        let elapsed = start.elapsed();
                        let total_time_ms = elapsed.as_millis() as u64;

                        // Ollama provides eval_count (actual tokens) on final chunk
                        let actual_tokens = chunk.eval_count.unwrap_or(total_tokens);
                        let eval_duration_ms = chunk.eval_duration
                            .map(|ns| ns / 1_000_000)
                            .unwrap_or(total_time_ms);

                        let tps = if eval_duration_ms > 0 {
                            actual_tokens as f64 / (eval_duration_ms as f64 / 1000.0)
                        } else {
                            0.0
                        };

                        return Ok(OllamaMetrics {
                            total_tokens: actual_tokens,
                            total_time_ms,
                            tokens_per_second: tps,
                        });
                    }
                }
                Err(e) => {
                    log::warn!("Failed to parse Ollama chunk: {} (line: {})", e, &line);
                    // Non-fatal: skip malformed lines
                }
            }
        }
    }

    // Stream ended without done=true
    let elapsed = start.elapsed();
    Ok(OllamaMetrics {
        total_tokens,
        total_time_ms: elapsed.as_millis() as u64,
        tokens_per_second: if elapsed.as_secs_f64() > 0.0 {
            total_tokens as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        },
    })
}
```

**Key design decisions:**
- Uses `reqwest` streaming (`bytes_stream()`) instead of buffering the full response
- Line-by-line JSON parsing (Ollama sends newline-delimited JSON)
- `cancelled` AtomicBool checked between each chunk (same pattern as CodeHelper's `generator.rs:329`)
- Returns `OllamaMetrics` with token count and speed from Ollama's own stats
- Buffer handles TCP chunks that may split across JSON line boundaries

---

### Step 3: Create Generation State & Tauri Commands

**New file:** `src-tauri/src/commands/generation.rs`

This is the direct adaptation of CodeHelper's `commands/inference.rs`. The pattern is nearly identical - only the backend call changes (Ollama HTTP instead of ONNX).

```rust
// src-tauri/src/commands/generation.rs
//
// Tauri commands for streaming Ollama generation with cancellation.
// Ported from smolpc-codehelper's commands/inference.rs pattern.

use crate::ollama::{self, OllamaMetrics};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::ipc::Channel;
use tauri::State;

const ERR_GENERATION_IN_PROGRESS: &str = "Generation already in progress";

/// Global generation state (managed by Tauri)
/// Ported from CodeHelper's InferenceState (inference.rs:38-61)
pub struct GenerationState {
    /// Cancellation token for active generation
    active_cancel: Arc<StdMutex<Option<Arc<AtomicBool>>>>,
    /// Whether generation is in progress
    generating: Arc<AtomicBool>,
}

impl Default for GenerationState {
    fn default() -> Self {
        Self {
            active_cancel: Arc::new(StdMutex::new(None)),
            generating: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// RAII guard - clears generation state when dropped.
/// Ported from CodeHelper's GenerationPermit (inference.rs:64-79)
struct GenerationPermit {
    generating: Arc<AtomicBool>,
    active_cancel: Arc<StdMutex<Option<Arc<AtomicBool>>>>,
}

impl Drop for GenerationPermit {
    fn drop(&mut self) {
        self.generating.store(false, Ordering::SeqCst);
        match self.active_cancel.lock() {
            Ok(mut guard) => *guard = None,
            Err(poisoned) => *poisoned.into_inner() = None,
        }
    }
}

impl GenerationState {
    fn try_begin(&self) -> Result<(GenerationPermit, Arc<AtomicBool>), String> {
        if self.generating
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(ERR_GENERATION_IN_PROGRESS.to_string());
        }

        let cancel_token = Arc::new(AtomicBool::new(false));
        match self.active_cancel.lock() {
            Ok(mut guard) => *guard = Some(Arc::clone(&cancel_token)),
            Err(poisoned) => *poisoned.into_inner() = Some(Arc::clone(&cancel_token)),
        }

        Ok((
            GenerationPermit {
                generating: Arc::clone(&self.generating),
                active_cancel: Arc::clone(&self.active_cancel),
            },
            cancel_token,
        ))
    }
}

/// Stream an answer from Ollama via Tauri Channel.
///
/// This is the equivalent of CodeHelper's `inference_generate` command (inference.rs:282-333).
/// Instead of running ONNX, it streams from Ollama's HTTP API.
///
/// # Arguments
/// * `system_prompt` - System message for the LLM
/// * `user_prompt` - User's question (already formatted with RAG context by frontend or this command)
/// * `on_token` - Tauri Channel for streaming tokens to frontend
#[tauri::command]
pub async fn stream_generate(
    system_prompt: String,
    user_prompt: String,
    on_token: Channel<String>,
    state: State<'_, GenerationState>,
) -> Result<OllamaMetrics, String> {
    let (_permit, cancelled) = state.try_begin()?;

    log::info!("Starting streaming generation (prompt: {} chars)", user_prompt.len());

    let token_channel = on_token.clone();

    let result = ollama::stream_chat(
        &system_prompt,
        &user_prompt,
        Arc::clone(&cancelled),
        move |token| {
            if let Err(e) = token_channel.send(token) {
                log::warn!("Failed to send token via channel: {}", e);
            }
        },
    )
    .await;

    match result {
        Ok(metrics) => {
            if cancelled.load(Ordering::SeqCst) {
                log::info!("Generation was cancelled");
                Err("GENERATION_CANCELLED: Generation cancelled".to_string())
            } else {
                log::info!(
                    "Generation complete: {} tokens, {:.1} tok/s",
                    metrics.total_tokens, metrics.tokens_per_second
                );
                Ok(metrics)
            }
        }
        Err(e) => {
            if e.contains("GENERATION_CANCELLED") {
                Err(e)
            } else {
                log::error!("Generation error: {}", e);
                Err(e)
            }
        }
    }
}

/// Cancel the current generation.
/// Ported from CodeHelper's `inference_cancel` (inference.rs:336-350).
#[tauri::command]
pub async fn cancel_generation(state: State<'_, GenerationState>) -> Result<(), String> {
    let active_cancel = match state.active_cancel.lock() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };

    if let Some(cancel_token) = active_cancel {
        cancel_token.store(true, Ordering::SeqCst);
        log::info!("Generation cancellation requested");
    }

    Ok(())
}

/// Check if generation is in progress.
#[tauri::command]
pub async fn is_generating(state: State<'_, GenerationState>) -> Result<bool, String> {
    Ok(state.generating.load(Ordering::SeqCst))
}
```

---

### Step 4: Update main.rs to Register New Commands

**File:** `src-tauri/src/main.rs`

Add the new modules and register the new commands alongside the existing `open_logs` command. The existing Flask server management stays for now (it still handles RAG, scene data, health checks). Only the Ollama inference call moves to Rust.

**Changes to make:**

1. Add module declarations at top:
```rust
mod ollama;
mod commands;  // or mod commands { pub mod generation; }
```

2. Register `GenerationState` and new commands in the builder:
```rust
// In main(), inside tauri::Builder::default()
.manage(commands::generation::GenerationState::default())
.invoke_handler(tauri::generate_handler![
    open_logs,
    commands::generation::stream_generate,
    commands::generation::cancel_generation,
    commands::generation::is_generating,
])
```

**Important:** Keep ALL existing Flask server code (`ServerManager`, `python_checker`, etc.) in place. The Flask server still handles `/health`, `/scene/update`, `/scene/current`, `/scene_analysis`, and RAG retrieval. Only the Ollama chat call is moved to Rust in this tier.

---

### Step 5: Create RAG Context Fetcher in Rust

**New file:** `src-tauri/src/rag_client.rs`

Since the Flask server still handles RAG retrieval, this module calls the Flask server to get RAG context, then passes it to the Rust-side Ollama streamer. This avoids duplicating the RAG logic.

```rust
// src-tauri/src/rag_client.rs
//
// Fetches RAG context from the Python Flask server.
// The Flask server still handles embedding + vector search.
// This module just retrieves the results for prompt construction.

use serde::{Deserialize, Serialize};

const RAG_SERVER: &str = "http://127.0.0.1:5179";

#[derive(Deserialize)]
pub struct RagContext {
    pub text: String,
    pub signature: String,
    pub url: String,
    pub similarity: f64,
}

#[derive(Serialize)]
struct RagQueryRequest {
    query: String,
    n_results: usize,
}

#[derive(Deserialize)]
struct RagQueryResponse {
    contexts: Vec<RagContext>,
}

/// Fetch RAG context from Flask server.
/// Falls back to empty context if server is unavailable.
pub async fn fetch_rag_context(query: &str) -> Vec<RagContext> {
    // TODO: Add a /rag/retrieve endpoint to the Flask server that returns
    // just the RAG contexts without calling Ollama. For now, we can construct
    // the prompt on the Rust side using the existing /ask endpoint's logic,
    // or add this new endpoint to server.py.
    //
    // INTERIM APPROACH: The frontend can continue fetching scene context
    // via the existing API, and pass the formatted prompt to the Rust command.
    // This avoids needing to modify server.py in Tier 1.
    vec![]
}
```

**Interim approach for Tier 1:** Rather than adding a new Flask endpoint immediately, have the frontend:
1. Call the existing Flask `/ask` endpoint but with a new query param `?stream=true` or
2. **Simpler:** Have the Rust `stream_generate` command accept the pre-built `system_prompt` and `user_prompt` from the frontend, where the frontend builds the prompt using RAG data it already fetches.

The simplest Tier 1 approach: The frontend fetches RAG context from Flask (new `/rag/retrieve` endpoint), builds the system prompt with RAG context included, then passes the complete `system_prompt` + `user_prompt` to the Rust `stream_generate` command.

**New Flask endpoint to add to `rag_system/server.py`:**

```python
@app.route('/rag/retrieve', methods=['POST'])
def retrieve_rag():
    """Retrieve RAG context without calling Ollama.
    Used by Rust backend to get context for prompt building."""
    try:
        data = request.json
        if data is None:
            return jsonify({'error': 'Invalid JSON'}), 400

        query = data.get('query', '')
        if not isinstance(query, str) or not query.strip():
            return jsonify({'error': 'Query must be a non-empty string'}), 400

        if len(query) > 10000:
            return jsonify({'error': 'Query too long (max 10,000 characters)'}), 400

        n_results = data.get('n_results', 3)

        contexts = rag.retrieve_context(query.strip(), n_results=n_results)

        return jsonify({
            'contexts': contexts,
            'rag_enabled': rag.initialized
        })
    except Exception as e:
        return jsonify({'error': str(e)}), 500
```

---

### Step 6: Create Frontend Inference Store

**New file:** `src/lib/stores/inference.svelte.ts`

Ported from CodeHelper's `inference.svelte.ts`. This store manages streaming generation via Tauri Channels.

```typescript
// src/lib/stores/inference.svelte.ts
//
// Manages streaming Ollama generation via Tauri IPC Channels.
// Ported from smolpc-codehelper/src/lib/stores/inference.svelte.ts

import { invoke, Channel } from '@tauri-apps/api/core';

// Types
interface OllamaMetrics {
  total_tokens: number;
  total_time_ms: number;
  tokens_per_second: number;
}

// State
let isGenerating = $state(false);
let error = $state<string | null>(null);
let lastMetrics = $state<OllamaMetrics | null>(null);

export const inferenceStore = {
  get isGenerating() { return isGenerating; },
  get error() { return error; },
  get lastMetrics() { return lastMetrics; },

  /**
   * Stream a response from Ollama via Tauri Channel.
   *
   * @param systemPrompt - Full system prompt (including RAG context + scene data)
   * @param userPrompt - User's question
   * @param onToken - Callback for each streamed token
   * @returns Metrics on success, null on cancel/error
   */
  async generateStream(
    systemPrompt: string,
    userPrompt: string,
    onToken: (token: string) => void,
  ): Promise<OllamaMetrics | null> {
    if (isGenerating) {
      error = 'Generation already in progress';
      return null;
    }

    isGenerating = true;
    error = null;
    lastMetrics = null;

    try {
      // Create Tauri Channel - same pattern as CodeHelper
      const onTokenChannel = new Channel<string>();
      onTokenChannel.onmessage = onToken;

      // Invoke Rust command with Channel
      const metrics = await invoke<OllamaMetrics>('stream_generate', {
        systemPrompt,
        userPrompt,
        onToken: onTokenChannel,
      });

      lastMetrics = metrics;
      return metrics;
    } catch (e) {
      const message = String(e);

      // Cancellation is not an error
      if (message.includes('GENERATION_CANCELLED')) {
        return null;
      }

      error = message;
      console.error('Streaming generation failed:', e);
      return null;
    } finally {
      isGenerating = false;
    }
  },

  /**
   * Cancel the current generation.
   */
  async cancel(): Promise<void> {
    if (!isGenerating) return;

    try {
      await invoke('cancel_generation');
    } catch (e) {
      console.error('Failed to cancel generation:', e);
    }
  },

  clearError(): void {
    error = null;
  },
};
```

---

### Step 7: Add TypeScript Types

**New file:** `src/lib/types/inference.ts`

```typescript
export interface OllamaMetrics {
  total_tokens: number;
  total_time_ms: number;
  tokens_per_second: number;
}
```

---

### Step 8: Update `src/lib/utils/api.ts`

Add a function to fetch RAG context separately (for the new `/rag/retrieve` endpoint):

```typescript
// Add to api.ts

export interface RagContext {
  text: string;
  signature: string;
  url: string;
  similarity: number;
}

export interface RagRetrieveResponse {
  contexts: RagContext[];
  rag_enabled: boolean;
}

export async function retrieveRagContext(query: string): Promise<RagRetrieveResponse> {
  return callRagServer<RagRetrieveResponse>('/rag/retrieve', { query, n_results: 3 }, 'POST');
}
```

---

### Step 9: Create Prompt Builder Utility

**New file:** `src/lib/utils/prompt.ts`

Extracts the system prompt construction logic that currently lives in `server.py` (lines 375-405) into a TypeScript utility, so the frontend can build the full prompt before sending it to the Rust streaming command.

```typescript
// src/lib/utils/prompt.ts
//
// Builds system and user prompts for the Ollama LLM.
// Extracted from rag_system/server.py lines 375-409.

import type { RagContext } from './api';

interface SceneContext {
  object_count?: number;
  active_object?: string | null;
  mode?: string;
  objects?: Array<{ name: string; type: string; modifiers?: Array<{ name: string; type: string }> }>;
  render_engine?: string;
}

/**
 * Build the system prompt with RAG context and scene data.
 * Mirrors the prompt from server.py lines 375-405.
 */
export function buildSystemPrompt(
  ragContexts: RagContext[],
  sceneContext?: SceneContext | null,
): string {
  // Scene summary
  let sceneSummary = '';
  if (sceneContext) {
    sceneSummary = `
Current Scene Information:
- Objects: ${sceneContext.object_count ?? 0} total
- Active: ${sceneContext.active_object ?? 'None'}
- Mode: ${sceneContext.mode ?? 'OBJECT'}
`;
  }

  // RAG context section
  let contextSection = '(No specific documentation found)';
  if (ragContexts.length > 0) {
    contextSection = ragContexts
      .map(ctx => `### ${ctx.signature}\n${ctx.text}`)
      .join('\n\n');
  }

  return `You are a patient Blender instructor helping students learn 3D modeling through the Blender interface.

CRITICAL INSTRUCTION: You MUST teach using UI-based instructions only. NEVER provide Python code or bpy commands.

Your teaching style:
- Provide step-by-step UI instructions (menu clicks, keyboard shortcuts, tool selections)
- Explain which menus to use (Add > Mesh > ..., Modifier Properties > Add Modifier > ...)
- Describe what buttons to click and what values to adjust in the properties panels
- Use clear descriptions like "In the 3D Viewport, press Shift+A, then select Mesh > UV Sphere"
- Explain concepts clearly and simply, using analogies when helpful
- Break down complex tasks into numbered steps
- Encourage experimentation with different settings
- Focus on understanding WHY each step matters, not just WHAT to do

${sceneSummary}

The documentation below contains Python code for reference ONLY - you must translate these concepts into UI actions:
${contextSection}

Answer the student's question in a friendly, educational manner with UI-based instructions. Keep answers concise (2-4 paragraphs).

EXAMPLES OF GOOD RESPONSES:
- "To add a sphere, press Shift+A in the 3D Viewport, then navigate to Mesh > UV Sphere"
- "In the Modifier Properties panel (wrench icon), click Add Modifier and select Bevel"
- "Select your object, press Tab to enter Edit Mode, then press Ctrl+R to add an edge loop"

NEVER write responses like this:
- "Use bpy.ops.mesh.primitive_uv_sphere_add(radius=1.0)"
- "Run this Python code: ..."
- Any Python code snippets or bpy commands`;
}

/**
 * Build the user prompt.
 * Mirrors server.py lines 407-409.
 */
export function buildUserPrompt(question: string): string {
  return `Question: ${question}

Provide a clear, educational answer that helps the student understand this Blender concept.`;
}
```

---

### Step 10: Update `App.svelte` - Wire Up Streaming

**File:** `src/App.svelte`

Replace the current `handleSendMessage` function (lines 79-136) with a streaming version:

**Current code to replace (lines 79-136):**
```typescript
async function handleSendMessage(content: string) {
  // ... currently calls askQuestion() and waits for full response
}
```

**New implementation:**
```typescript
import { inferenceStore } from '$lib/stores/inference.svelte';
import { retrieveRagContext } from '$lib/utils/api';
import { buildSystemPrompt, buildUserPrompt } from '$lib/utils/prompt';

async function handleSendMessage(content: string) {
  if (!currentChat || isWaitingForResponse) return;

  const chatId = currentChat.id;

  try {
    // Add user message
    const userMessage = {
      id: crypto.randomUUID(),
      role: 'user' as const,
      content,
      timestamp: Date.now()
    };
    chatsStore.addMessage(chatId, userMessage);

    // Add assistant placeholder (streaming)
    const assistantMessageId = crypto.randomUUID();
    chatsStore.addMessage(chatId, {
      id: assistantMessageId,
      role: 'assistant' as const,
      content: '',
      timestamp: Date.now(),
      isStreaming: true
    });

    isWaitingForResponse = true;

    try {
      // 1. Fetch RAG context from Flask server
      const sceneContext = blenderStore.getSceneContext();
      let ragContexts: RagContext[] = [];
      try {
        const ragResponse = await retrieveRagContext(content);
        ragContexts = ragResponse.contexts;
      } catch {
        // RAG unavailable - continue without context
        console.warn('RAG context unavailable, proceeding without');
      }

      // 2. Build prompts (logic moved from server.py to TypeScript)
      const systemPrompt = buildSystemPrompt(ragContexts, sceneContext);
      const userPrompt = buildUserPrompt(content);

      // 3. Stream response via Tauri Channel
      let accumulatedText = '';

      const metrics = await inferenceStore.generateStream(
        systemPrompt,
        userPrompt,
        (token: string) => {
          // Called for each streamed token
          accumulatedText += token;
          chatsStore.updateMessage(chatId, assistantMessageId, {
            content: accumulatedText,
            isStreaming: true
          });
        }
      );

      // 4. Finalize message
      chatsStore.updateMessage(chatId, assistantMessageId, {
        content: accumulatedText,
        isStreaming: false
      });

      // Optionally show metrics in UI (e.g., "42 tokens, 8.3 tok/s")
      if (metrics) {
        console.log(`Generated ${metrics.total_tokens} tokens at ${metrics.tokens_per_second.toFixed(1)} tok/s`);
      }

    } catch (error) {
      const errorMessage = error instanceof Error ? error.message : 'Failed to get response';
      chatsStore.updateMessage(chatId, assistantMessageId, {
        content: `Error: ${errorMessage}`,
        isStreaming: false
      });
    } finally {
      isWaitingForResponse = false;
    }
  } catch (error) {
    console.error('[App] Error in handleSendMessage:', error);
    appError = error instanceof Error ? error.message : 'Error sending message';
    isWaitingForResponse = false;
  }
}
```

---

### Step 11: Add Cancel Button to ChatInput

**File:** `src/lib/components/ChatInput.svelte`

Add a cancel button that appears while generation is in progress. When clicked, it calls `inferenceStore.cancel()`.

**Changes needed:**
1. Import `inferenceStore` from the store
2. Add a cancel button next to the send button that shows when `inferenceStore.isGenerating` is true
3. The cancel button calls `inferenceStore.cancel()`

```svelte
{#if inferenceStore.isGenerating}
  <button
    onclick={() => inferenceStore.cancel()}
    class="p-2 text-red-500 hover:bg-red-50 rounded-lg transition-colors"
    aria-label="Cancel generation"
  >
    <!-- Stop/square icon -->
    <svg class="w-5 h-5" fill="currentColor" viewBox="0 0 24 24">
      <rect x="6" y="6" width="12" height="12" rx="2" />
    </svg>
  </button>
{/if}
```

---

### Step 12: Update ChatMessage Component for Streaming Indicator

**File:** `src/lib/components/ChatMessage.svelte`

The component already handles `isStreaming` on messages. Ensure the streaming indicator (blinking cursor) shows while content is being streamed in:

```svelte
{#if message.isStreaming && !message.content}
  <!-- Show typing indicator when waiting for first token -->
  <div class="flex gap-1 py-2">
    <div class="w-2 h-2 bg-[var(--muted-foreground)] rounded-full animate-bounce"></div>
    <div class="w-2 h-2 bg-[var(--muted-foreground)] rounded-full animate-bounce" style="animation-delay: 0.1s"></div>
    <div class="w-2 h-2 bg-[var(--muted-foreground)] rounded-full animate-bounce" style="animation-delay: 0.2s"></div>
  </div>
{:else if message.isStreaming && message.content}
  <!-- Show content with blinking cursor while streaming -->
  {@html parseMarkdown(message.content)}<span class="inline-block w-2 h-5 bg-[var(--foreground)] animate-pulse ml-0.5"></span>
{:else}
  {@html parseMarkdown(message.content)}
{/if}
```

---

## File Summary

| Action | File | Description |
|--------|------|-------------|
| ADD | `src-tauri/src/ollama.rs` | Streaming HTTP client for Ollama |
| ADD | `src-tauri/src/commands/generation.rs` | Tauri commands with Channel + cancellation |
| MODIFY | `src-tauri/src/main.rs` | Register new modules and commands |
| MODIFY | `src-tauri/Cargo.toml` | Add tokio, futures-util, log deps |
| ADD | `src/lib/stores/inference.svelte.ts` | Frontend streaming store |
| ADD | `src/lib/types/inference.ts` | TypeScript types for metrics |
| ADD | `src/lib/utils/prompt.ts` | System/user prompt builder |
| MODIFY | `src/lib/utils/api.ts` | Add `retrieveRagContext()` function |
| MODIFY | `src/App.svelte` | Wire up streaming in `handleSendMessage` |
| MODIFY | `src/lib/components/ChatInput.svelte` | Add cancel button |
| MODIFY | `src/lib/components/ChatMessage.svelte` | Improve streaming indicator |
| MODIFY | `rag_system/server.py` | Add `/rag/retrieve` endpoint |

---

## Testing Plan

1. **Unit test:** `GenerationState::try_begin` rejects concurrent generations (same as CodeHelper's test in inference.rs:359-373)
2. **Unit test:** `GenerationPermit` drop clears state (same as inference.rs:375-396)
3. **Integration test:** Start Ollama, send a question through `stream_generate`, verify tokens arrive via Channel
4. **Manual test:** Open app, type a question, verify tokens appear one-by-one in the chat
5. **Manual test:** Click cancel mid-generation, verify response stops and UI is left in a clean state
6. **Manual test:** Verify existing features still work (scene panel, suggestions tab, chat history)

---

## Rollback Strategy

If Tier 1 has issues, the frontend can fall back to the original `askQuestion()` HTTP path. Keep the original `handleSendMessage` as a commented-out block or behind a feature flag in settings (e.g., `useStreaming: boolean`).
