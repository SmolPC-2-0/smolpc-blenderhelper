# Blender Learning Assistant - Getting Started

Welcome to the Blender Learning Assistant! This educational tool helps students learn Blender through AI-powered Q&A, guided tutorials, and scene-aware suggestions.

## 🎯 What This Does

The Blender Learning Assistant is **NOT** a code generator. Instead, it's an educational companion that:

- ✅ **Answers questions** about Blender concepts using RAG-enhanced AI
- ✅ **Provides suggestions** for what to try next based on your current scene
- ✅ **Guides you through tutorials** with step-by-step validation
- ✅ **Runs completely offline** on your local machine
- ✅ **Works on low-end hardware** using quantized 7B models

## 🏗️ Architecture

```
┌─────────────────────────┐
│  Tauri Frontend (UI)    │  ← Main learning interface
│  Port: Desktop App      │
└───────────┬─────────────┘
            │ HTTP
            ▼
┌─────────────────────────┐
│  RAG Server (Flask)     │  ← Educational AI backend
│  Port: 5000             │
└───────────┬─────────────┘
            │ HTTP
            ▼
┌─────────────────────────┐
│  Ollama (LLM)           │  ← qwen2.5:7b-instruct-q4_K_M
│  Port: 11434            │
└─────────────────────────┘
            ▲
            │
┌─────────────────────────┐
│  Blender Addon          │  ← Scene data provider
│  (Inside Blender)       │
└─────────────────────────┘
```

## 📋 Prerequisites

### 1. Install Ollama

**Download:** https://ollama.com/download

After installation, pull the educational model:
```bash
ollama pull qwen2.5:7b-instruct-q4_K_M
```

**Why this model?**
- **qwen2.5:7b-instruct-q4_K_M** is a 4-bit quantized model (~4.7GB)
- Excellent for educational explanations
- Runs on 6-8GB RAM
- Fast enough for responsive Q&A

**For even lower-end hardware:**
```bash
ollama pull qwen2.5:7b-instruct-q3_K_M  # 3-bit, ~3.5GB
```

Then set the environment variable:
```bash
# Windows (PowerShell)
$env:OLLAMA_MODEL="qwen2.5:7b-instruct-q3_K_M"

# Linux/Mac
export OLLAMA_MODEL="qwen2.5:7b-instruct-q3_K_M"
```

### 2. Install Python Dependencies

```bash
cd rag_system
pip install flask flask-cors sentence-transformers numpy requests
```

**Optional:** Build the RAG database (for better answers):
```bash
python build_database.py
```

This downloads Blender API documentation and creates a vector database for context-aware answers.

### 3. Install Node.js and Rust (for Tauri)

**Node.js:** https://nodejs.org/ (LTS version)

**Rust:** https://rustup.rs/

After Rust installation:
```bash
# Install Tauri CLI
npm install
```

## 🚀 Starting the System

You need to start **3 components** in order:

### Step 1: Start Ollama

```bash
# Usually starts automatically after installation
# If not, run:
ollama serve
```

**Verify it's running:**
```bash
curl http://127.0.0.1:11434/api/tags
```

### Step 2: Start the RAG Server

```bash
cd rag_system
python server.py
```

You should see:
```
============================================================
Blender Learning Assistant - RAG Server
============================================================
Running at: http://127.0.0.1:5000
Educational Mode:
  - Q&A endpoint: POST /ask
  - Scene analysis: POST /scene_analysis
  - Tutorials: GET /tutorial/list, POST /tutorial/step

Model: qwen2.5:7b-instruct-q4_K_M
============================================================

✅ RAG system ready (API documentation loaded)
```

### Step 3: Start the Tauri App

```bash
# From project root
npm run tauri dev
```

This will:
- Start the Tauri development server
- Open the Blender Learning Assistant window
- The backend REST API runs on http://127.0.0.1:17890

### Step 4: Install the Blender Addon

**Option A: Automatic Installation (Windows)**
```batch
install_to_blender.bat
```

**Option B: Manual Installation**
1. Open Blender
2. Go to Edit → Preferences → Add-ons
3. Click "Install" button
4. Navigate to `blender_addon/blender_helper_http.py`
5. Click "Install Add-on"
6. Enable the checkbox next to "3D View: Blender Learning Assistant"

**Find the addon:**
- Open 3D Viewport
- Press `N` to open the sidebar
- Click the "Learn" tab

## 💡 Using the System

### From Blender (In-Viewport Assistant)

1. **Press N** in the 3D Viewport to open sidebar
2. **Click "Learn" tab**
3. You'll see:
   - Current scene info
   - Question input field
   - Suggestions button
   - Server status

**Example Usage:**
- Type: "What is a modifier?"
- Click "Ask"
- View answer in the panel or console

### From Tauri App (Main Learning Interface)

The Tauri app provides a richer learning experience:

1. **Ask Questions**
   - Type any Blender question
   - Get context-aware answers using RAG

2. **Get Suggestions**
   - Click "Get Suggestions"
   - Receive 3-5 specific next steps based on your scene

3. **Follow Tutorials**
   - Click "Load Tutorials"
   - Choose a tutorial to start
   - Follow step-by-step instructions
   - Get validation feedback

## 📚 Tutorial System

The system includes built-in tutorials in `rag_system/tutorials.json`:

1. **Basic 3D Modeling** - Learn to create and modify shapes
2. **Introduction to Modifiers** - Understand how modifiers work
3. **Basic Materials** - Add colors and materials
4. **Navigating the 3D Viewport** - Master navigation

### Adding Your Own Tutorials

Edit `rag_system/tutorials.json`:

```json
{
  "tutorials": [
    {
      "id": "my-tutorial",
      "title": "My Custom Tutorial",
      "description": "Learn something awesome",
      "steps": [
        {
          "title": "Step 1",
          "instruction": "Do this thing...",
          "validation": {
            "check": "has_object_type",
            "params": {"type": "MESH"}
          }
        }
      ]
    }
  ]
}
```

**Validation Types:**
- `has_object_type` - Check if scene has an object of a specific type
- `has_modifier` - Check if active object has a specific modifier
- `object_count` - Check if scene has at least N objects
- `always_pass` - No validation (informational steps)

## 🔧 Configuration

### Environment Variables

```bash
# Change the model
OLLAMA_MODEL=qwen2.5:7b-instruct-q3_K_M

# Increase timeout for slower hardware (seconds)
OLLAMA_TIMEOUT_SECS=900

# RAG server URL (if running elsewhere)
RAG_SERVER_URL=http://127.0.0.1:5000
```

### Tauri Window Size

Edit `src-tauri/tauri.conf.json`:
```json
{
  "app": {
    "windows": [
      { "width": 1200, "height": 900 }
    ]
  }
}
```

## 🐛 Troubleshooting

### "Cannot connect to RAG server"

**Check if server is running:**
```bash
curl http://127.0.0.1:5000/health
```

**Expected response:**
```json
{
  "status": "ok",
  "rag_enabled": true,
  "rag_docs": 150
}
```

### "Ollama not running"

```bash
# Check Ollama status
curl http://127.0.0.1:11434/api/tags

# If not running, start it
ollama serve
```

### "RAG disabled - will use LLM knowledge only"

This means the RAG database wasn't built. While the system still works, answers won't be as accurate.

**Solution:**
```bash
cd rag_system
python build_database.py
```

This downloads Blender documentation and creates the vector database.

### Tauri App Won't Start

```bash
# Clean and rebuild
npm run tauri build --debug

# Check Rust installation
rustc --version
cargo --version
```

### Blender Addon Not Showing

1. Check Blender console for errors
2. Make sure Python `requests` library is available:
   ```python
   import requests  # Should not error
   ```
3. Verify addon is enabled in Preferences → Add-ons

### Slow Responses

**For faster responses on low-end hardware:**

1. Use smaller quantization:
   ```bash
   ollama pull qwen2.5:7b-instruct-q2_K
   export OLLAMA_MODEL=qwen2.5:7b-instruct-q2_K
   ```

2. Reduce documentation chunks in `server.py`:
   ```python
   contexts = rag.retrieve_context(question, n_results=1)  # Instead of 3
   ```

## 📖 Example Questions to Ask

**Beginner:**
- "What does the Tab key do in Blender?"
- "How do I move objects?"
- "What is Edit Mode?"

**Intermediate:**
- "How do I add a Subdivision Surface modifier?"
- "What's the difference between Edit Mode and Object Mode?"
- "How do I create materials?"

**Advanced:**
- "How do Array and Mirror modifiers work together?"
- "What's the best workflow for modeling a character?"
- "How does the shader editor work?"

## 🎓 Learning Workflow

**Recommended approach for students:**

1. **Start with a tutorial** (Tauri app → Tutorials)
2. **Follow step-by-step** and experiment in Blender
3. **Ask questions** when you don't understand something
4. **Get suggestions** for what to try next
5. **Repeat and explore!**

## 🔄 Updates and Maintenance

### Update the Model

```bash
ollama pull qwen2.5:7b-instruct-q4_K_M
```

### Update RAG Database

```bash
cd rag_system
python build_database.py
```

### Add More Tutorials

Edit `rag_system/tutorials.json` and add new tutorial objects.

## 🤝 Support

**Common Issues:**
- Check all 3 services are running (Ollama, RAG server, Tauri app)
- Verify ports 5000, 11434, and 17890 are not in use
- Check firewall isn't blocking local connections

**System Requirements:**
- **RAM:** 6-8GB minimum (for 7B quantized model)
- **Storage:** ~5GB for model + dependencies
- **CPU:** Modern multi-core processor recommended
- **GPU:** Optional (CPU inference works fine)

## 📝 Notes

- **All processing happens locally** - no internet required after setup
- **No code is executed automatically** - this is education-focused, not automation
- **Scene data stays private** - everything runs on your machine
- **Responses may vary** - AI models are probabilistic, not deterministic

## 🚀 Production Build

To create a standalone executable:

```bash
npm run tauri build
```

The installer will be in `src-tauri/target/release/bundle/`

---

**Enjoy learning Blender with AI assistance!** 🎨
