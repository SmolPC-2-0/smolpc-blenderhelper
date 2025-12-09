# Blender Helper AI - Project Documentation

## Project Overview

**Blender Helper AI** is an intelligent assistant that generates Python code to create 3D models in Blender using natural language prompts. Users describe what they want to create (e.g., "a cube with rounded corners"), and the system generates executable Blender Python code using a local LLM (Ollama).

### Key Features
- Natural language to Blender Python code generation
- Automatic error detection and self-healing
- Deliberate reasoning for better code quality
- In-memory context retention across sessions
- Support for complex multi-object scenes

---

## Architecture

### System Components

```
┌─────────────────────────────────────────────────────────────────┐
│                         User Interface                          │
│                    (Blender 3D Viewport)                        │
└───────────────────────────────┬─────────────────────────────────┘
                                │
                                ↓
┌─────────────────────────────────────────────────────────────────┐
│                    Blender Python Addon                         │
│                  (blender_helper.py)                            │
│  - Captures user goals                                          │
│  - Executes generated code                                      │
│  - Handles errors with auto-retry                               │
└───────────────────────────────┬─────────────────────────────────┘
                                │ HTTP REST API
                                │ (127.0.0.1:17890)
                                ↓
┌─────────────────────────────────────────────────────────────────┐
│                   Tauri Backend (Rust)                          │
│                  (src-tauri/src/main.rs)                        │
│  - Axum REST server                                             │
│  - Routes: /next_step, /run_macro, /fix_error, /remember       │
└───────────────────────────────┬─────────────────────────────────┘
                                │
                                ↓
┌─────────────────────────────────────────────────────────────────┐
│                  Blender Bridge Module                          │
│              (src-tauri/src/blender_bridge.rs)                  │
│  - Business logic for code generation                           │
│  - In-memory conversation context (20 item buffer)              │
│  - Error recovery orchestration                                 │
└───────────────────────────────┬─────────────────────────────────┘
                                │
                                ↓
┌─────────────────────────────────────────────────────────────────┐
│                    Ollama Integration                           │
│                (src-tauri/src/ollama.rs)                        │
│  - LLM communication layer                                      │
│  - Two-phase deliberate reasoning                               │
│  - Timeout management (10min default)                           │
└───────────────────────────────┬─────────────────────────────────┘
                                │ HTTP API
                                │ (127.0.0.1:11434)
                                ↓
┌─────────────────────────────────────────────────────────────────┐
│                    Ollama LLM Server                            │
│              (qwen2.5-coder:7b-instruct-q3_K_M)                 │
│  - Local language model inference                               │
│  - Code generation and debugging                                │
└─────────────────────────────────────────────────────────────────┘
```

### Data Flow

#### 1. Code Generation Flow
```
User Input → Blender Addon → REST API → Blender Bridge → Ollama Module
                                                              ↓
                                                      Planning Phase (2s)
                                                              ↓
                                                      Code Generation Phase
                                                              ↓
Generated Code ← Blender Addon ← REST API ← Blender Bridge ← Ollama Module
      ↓
   Execute in Blender
      ↓
   Success ✓ or Error ✗
```

#### 2. Error Recovery Flow
```
Execution Error Detected
      ↓
Extract Error Message
      ↓
POST to /fix_error endpoint
      ↓
LLM analyzes: Original Goal + Broken Code + Error
      ↓
Generates Fixed Code
      ↓
Auto-retry Execution
      ↓
Success ✓ or Fail ✗
```

---

## Technical Stack

### Backend (Rust)
- **Framework**: Tauri v2
- **HTTP Server**: Axum 0.7
- **HTTP Client**: Reqwest 0.11
- **Async Runtime**: Tokio 1.x
- **Serialization**: Serde + Serde JSON
- **CORS**: Tower-HTTP

### Frontend (Python)
- **Host Application**: Blender 3.0+
- **API Client**: Requests library
- **Code Execution**: Python `exec()` with context override

### AI/LLM
- **Server**: Ollama
- **Model**: qwen2.5-coder:7b-instruct-q3_K_M
- **Temperature**: 0.7 (balanced creativity/consistency)
- **Timeout**: 600 seconds (configurable via `OLLAMA_TIMEOUT_SECS`)

---

## API Endpoints

### 1. `/blender/next_step` (POST)
**Purpose**: Generate step-by-step instructions for manual execution

**Request**:
```json
{
  "goal": "Create a Victorian house"
}
```

**Response**:
```json
{
  "step": "1. Add cube for base structure...\n2. Add pyramid for roof...\n..."
}
```

**Use Case**: When users want guidance rather than automated execution

---

### 2. `/blender/run_macro` (POST)
**Purpose**: Generate executable Blender Python code

**Request**:
```json
{
  "goal": "Create a cube with rounded corners"
}
```

**Response**:
```json
{
  "code": "import bpy\n\n# Create cube\nbpy.ops.mesh.primitive_cube_add()..."
}
```

**Features**:
- Uses deliberate reasoning (2-second planning phase)
- Accesses conversation memory for context
- Returns sanitized, executable Python code

---

### 3. `/blender/fix_error` (POST)
**Purpose**: Automatically fix code that failed to execute

**Request**:
```json
{
  "goal": "Create a cube with rounded corners",
  "code": "bpy.ops.mesh.primitive_cube_add()\nbevel.harden_edges = True",
  "error": "'BevelModifier' object has no attribute 'harden_edges'"
}
```

**Response**:
```json
{
  "code": "# Fixed: Removed non-existent harden_edges property\nbpy.ops.mesh.primitive_cube_add()..."
}
```

**Features**:
- Single-phase reasoning for speed
- Preserves working code, fixes only broken parts
- Adds explanatory comments

---

### 4. `/blender/remember` (POST)
**Purpose**: Store context for future requests

**Request**:
```json
{
  "event": "Executed macro for goal: 'Create a cube'"
}
```

**Response**:
```json
{
  "ok": true
}
```

**Features**:
- Maintains 20-item circular buffer
- Helps LLM avoid duplicates
- Provides session continuity

---

## LLM Prompting Strategy

### System Prompt Architecture

The system prompt includes:

1. **Role Definition**: "Expert Blender Python programmer"
2. **Critical Requirements**: Analyze ALL properties before coding
3. **API Documentation**: Exact properties for common modifiers
4. **Common Mistakes**: Explicit list of non-existent properties
5. **Safety Principles**: Null checking, mode awareness, error handling
6. **Response Format**: Analysis first, then code

### Deliberate Reasoning (Two-Phase)

**Phase 1 - Planning (2 seconds)**:
- Model writes internal notes about approach
- Identifies primitives, modifiers, and operations needed
- Not shown to user

**Phase 2 - Generation**:
- Receives planning notes as additional context
- Generates final code based on deliberation
- Results in higher quality output

### Why This Works
- Prevents "rushing" to simple solutions
- Forces requirement analysis before coding
- Reduces hallucination of non-existent API properties
- Improves consistency across similar prompts

---

## Memory System

### In-Memory Context Buffer

**Location**: `src-tauri/src/blender_bridge.rs:25`

**Implementation**:
```rust
static MEMORY: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::new()));
```

**Behavior**:
- Stores last 20 events as strings
- Circular buffer (oldest dropped when full)
- Thread-safe with Mutex
- Persists for application lifetime

**Usage**:
- "Plan requested for goal: 'X'"
- "Generated script for goal: 'Y'"
- "Fixed error: AttributeError..."

**Benefits**:
- Avoids creating duplicate objects
- Maintains consistency with previous actions
- Helps LLM understand session context

---

## Error Handling & Recovery

### Multi-Layer Error Strategy

#### Layer 1: Code Sanitization (Blender Addon)
- Extracts code from markdown blocks
- Fixes common syntax errors (bare try/except)
- Normalizes operator names
- Adds safety wrappers for unsafe operations

#### Layer 2: Safe Execution Context
- Ensures object mode before execution
- Provides fallback active object selection
- Uses `temp_override` for proper context
- Wraps in try/except for graceful failure

#### Layer 3: Automatic Error Recovery
- Catches execution exceptions
- Sends error details to `/fix_error` endpoint
- LLM analyzes error and generates fix
- Auto-retries with corrected code
- Reports success/failure to user

### Error Recovery Example

**Original Code**:
```python
bevel = obj.modifiers.new(name="Bevel", type='BEVEL')
bevel.harden_edges = True  # ❌ Property doesn't exist
```

**Error**: `'BevelModifier' object has no attribute 'harden_edges'`

**Fixed Code**:
```python
# Fixed: Removed non-existent harden_edges property
bevel = obj.modifiers.new(name="Bevel", type='BEVEL')
bevel.width = 0.1
bevel.segments = 4
```

**Outcome**: User sees "Auto-fix successful!" instead of error

---

## Configuration

### Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `OLLAMA_MODEL` | `qwen2.5-coder:7b-instruct-q3_K_M` | LLM model to use |
| `OLLAMA_TIMEOUT_SECS` | `600` (10 min) | Request timeout |
| `DELIBERATION_SECS` | `2` | Planning phase duration |

### Setting Environment Variables

**Windows (PowerShell)**:
```powershell
$env:OLLAMA_MODEL="qwen2.5-coder:14b"
$env:OLLAMA_TIMEOUT_SECS="900"
```

**Linux/Mac**:
```bash
export OLLAMA_MODEL="qwen2.5-coder:14b"
export OLLAMA_TIMEOUT_SECS="900"
```

---

## Development Setup

### Prerequisites
1. **Rust** (latest stable)
2. **Node.js** (for Tauri build)
3. **Blender** 3.0 or higher
4. **Ollama** with `qwen2.5-coder:7b-instruct-q3_K_M` model

### Installation Steps

1. **Install Ollama Model**:
   ```bash
   ollama pull qwen2.5-coder:7b-instruct-q3_K_M
   ```

2. **Build Tauri Backend**:
   ```bash
   cd src-tauri
   cargo build --release
   ```

3. **Install Blender Addon**:
   - Open Blender
   - Edit → Preferences → Add-ons → Install
   - Select `blender_addon/blender_helper.py`
   - Enable "Blender Helper AI Link"

4. **Start Application**:
   ```bash
   cargo tauri dev  # Development mode
   # or
   cargo tauri build  # Production build
   ```

### Development Workflow

1. Make changes to Rust code in `src-tauri/src/`
2. Rebuild: `cd src-tauri && cargo build`
3. Restart Tauri application
4. Reload Blender addon if Python code changed

---

## Key Design Decisions

### Why Tauri + Rust?
- **Performance**: Fast HTTP server with low overhead
- **Safety**: Rust's memory safety prevents crashes
- **Cross-platform**: Single codebase for Windows/Mac/Linux
- **Small binary**: Minimal runtime dependencies

### Why Local LLM (Ollama)?
- **Privacy**: No data sent to external services
- **Cost**: No API fees
- **Speed**: Local inference (when model is loaded)
- **Control**: Full control over model and parameters

### Why Two-Phase Deliberation?
- **Quality**: Reduces hallucination and errors
- **Consistency**: More reliable across similar prompts
- **Reasoning**: Forces analysis before generation
- **Trade-off**: Adds 2 seconds but significantly improves output

### Why Auto-Fix Instead of Perfect Generation?
- **Pragmatic**: Impossible to prevent all LLM hallucinations
- **Scalable**: Handles any error, not just documented ones
- **Self-improving**: Model learns from real errors
- **User Experience**: Seamless recovery vs. manual debugging

---

## Performance Considerations

### Response Times

| Operation | Typical Duration | Notes |
|-----------|-----------------|-------|
| Planning Phase | 15-30s | Model generates internal notes |
| Code Generation | 30-60s | Full code generation |
| Error Fix | 20-40s | Single-phase, faster than generation |
| Total (with deliberation) | 45-90s | Planning + Generation |

### Optimization Strategies

1. **Model Selection**: 7B model vs 14B/32B (3-5x faster)
2. **Quantization**: q3_K_M quantization reduces memory/improves speed
3. **Timeout Tuning**: Set based on hardware capabilities
4. **Prompt Length**: Shorter prompts = faster inference
5. **Temperature**: 0.7 balances quality and speed

### Resource Usage

| Resource | Typical | Peak |
|----------|---------|------|
| RAM (7B model) | 4-6 GB | 8 GB |
| CPU (inference) | 50-80% | 100% |
| GPU (if available) | 40-60% | 80% |
| Disk (model) | 2.5 GB | - |
| Network | Local only | - |

---

## Testing Strategy

### Manual Testing Checklist

**Basic Shapes**:
- [ ] Simple cube
- [ ] Cube with rounded corners
- [ ] Sphere (UV and Ico)
- [ ] Cylinder
- [ ] Cone

**Complex Shapes**:
- [ ] Victorian house (multiple objects)
- [ ] Christmas tree (cone + decorations)
- [ ] Simple character (multiple primitives)

**Error Recovery**:
- [ ] Trigger known error (e.g., invalid property)
- [ ] Verify auto-fix attempt
- [ ] Check success message

**Context Memory**:
- [ ] Create object A
- [ ] Create object B "next to A"
- [ ] Verify B is positioned relative to A

### Debug Mode

Enable detailed logging:
```python
# In Blender Python Console
import sys
sys.stdout = sys.__stdout__  # Enable console output
```

Check Rust logs:
```bash
# Terminal running Tauri app shows:
# "Ollama request failed: ..." for errors
# "TIMEOUT: ..." for timeout issues
```

---

## Common Issues & Solutions

### Issue: Timeouts
**Symptom**: Requests fail after 10 minutes
**Solution**:
```bash
export OLLAMA_TIMEOUT_SECS=900  # Increase to 15 minutes
```
Or use smaller model: `qwen2.5-coder:7b`

### Issue: Model Hallucinations
**Symptom**: Code uses non-existent properties
**Solution**: Auto-fix will handle it, but you can also:
- Add to "CRITICAL - COMMON API MISTAKES" section
- Use more explicit prompts

### Issue: Inconsistent Results
**Symptom**: Same prompt produces different shapes
**Solution**:
- Be more explicit in prompt (see detailed prompt examples)
- Lower temperature (edit `ollama.rs`, set to 0.3-0.5)

### Issue: Connection Refused
**Symptom**: "CONNECTION: Is Ollama running?"
**Solution**:
```bash
# Start Ollama
ollama serve

# Verify it's running
curl http://127.0.0.1:11434/api/tags
```

### Issue: Wrong Model Loaded
**Symptom**: Using wrong model or old cached model
**Solution**:
```bash
# Check currently loaded models
ollama ps

# Force unload all
pkill ollama

# Restart with specific model
ollama run qwen2.5-coder:7b-instruct-q3_K_M
```

---

## Future Enhancements

### Planned Features
1. **Streaming Responses**: Show progress as code generates
2. **Multi-attempt Error Recovery**: Try fix up to 3 times
3. **Code Library**: Save/reuse successful patterns
4. **Visual Feedback**: Preview in UI before execution
5. **History Panel**: View all generated code in session

### Potential Improvements
- **Fine-tuned Model**: Train on Blender-specific dataset
- **Retrieval Augmented Generation (RAG)**: Index Blender docs
- **Persistent Memory**: Save context to disk
- **Batch Operations**: Process multiple goals in sequence
- **Undo/Redo**: Rollback generated changes

---

## Contributing Guidelines

### Code Style
- **Rust**: Follow `rustfmt` defaults
- **Python**: PEP 8 compliance
- **Comments**: Explain "why", not "what"

### Pull Request Process
1. Fork repository
2. Create feature branch
3. Add tests if applicable
4. Update documentation
5. Submit PR with clear description

### Areas for Contribution
- Additional modifier documentation
- Error recovery heuristics
- Performance optimizations
- Test coverage
- Documentation improvements

---

## License & Credits

### Project License
[Specify your license here - e.g., MIT, GPL, etc.]

### Dependencies
- **Tauri**: MIT/Apache-2.0
- **Axum**: MIT
- **Ollama**: MIT
- **Qwen2.5-Coder**: Apache-2.0

### Acknowledgments
- Qwen team for the code generation model
- Ollama for local LLM infrastructure
- Blender Foundation for the Python API

---

## Appendix

### A. Blender API Quick Reference

**Common Primitives**:
```python
bpy.ops.mesh.primitive_cube_add(size=2)
bpy.ops.mesh.primitive_uv_sphere_add(radius=1)
bpy.ops.mesh.primitive_cylinder_add(radius=1, depth=2)
bpy.ops.mesh.primitive_cone_add(radius1=1, depth=2)
```

**Modifier Creation**:
```python
# Bevel (rounded edges)
bevel = obj.modifiers.new(name="Bevel", type='BEVEL')
bevel.width = 0.1
bevel.segments = 4

# Subdivision (smooth surface)
subsurf = obj.modifiers.new(name="Subsurf", type='SUBSURF')
subsurf.levels = 2
```

**Shading**:
```python
bpy.ops.object.shade_smooth()  # Smooth shading
bpy.ops.object.shade_flat()    # Flat shading
```

### B. File Structure
```
smolpc-blenderhelper/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs              # Axum server setup
│   │   ├── blender_bridge.rs    # Business logic & endpoints
│   │   └── ollama.rs            # LLM communication
│   ├── Cargo.toml               # Rust dependencies
│   └── target/                  # Build artifacts
├── blender_addon/
│   └── blender_helper.py        # Blender addon (UI + execution)
├── PROJECT_DOCUMENTATION.md     # This file
└── README.md                    # Quick start guide
```

### C. Glossary

- **LLM**: Large Language Model
- **Ollama**: Local LLM inference server
- **Tauri**: Rust-based desktop app framework
- **Axum**: Rust web framework
- **Bpy**: Blender Python API
- **Modifier**: Blender operation that changes geometry
- **Primitive**: Basic 3D shape (cube, sphere, etc.)
- **Deliberation**: Two-phase reasoning process
- **Auto-fix**: Automatic error recovery system

---

**Document Version**: 1.0
**Last Updated**: 2025-12-02
**Maintained By**: [Your Name/Team]
