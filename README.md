# Blender Learning Assistant

An **offline, educational AI assistant** for Blender that helps students learn 3D modeling through context-aware Q&A, guided tutorials, and smart suggestions.

> **Version 4.0** - Educational Focus (Learning, not automation!)

## 🚀 Quick Start

**New in v4.0:** Single executable that runs everything!

1. **Download** the latest release (`.msi` installer for Windows)
2. **Install** and launch the app
3. **First launch:** Python dependencies install automatically (1-2 min)
4. **Done!** The app handles the RAG server startup automatically

No manual terminal commands needed - just click and go! See [INSTALL.md](INSTALL.md) for detailed instructions.

---

## 🎓 What This Does

**This is NOT a code generator.** Instead, it's your personal Blender tutor that:

- ✅ **Answers your questions** about Blender using RAG-enhanced AI
- ✅ **Suggests what to try next** based on your current scene
- ✅ **Guides you through tutorials** with step-by-step validation
- ✅ **Runs completely offline** after initial setup
- ✅ **Works on low-end hardware** using quantized 7B models

**Example Questions:**
- "What is a modifier?"
- "How do I add materials?"
- "What does Edit Mode do?"

---

## 🏗️ Architecture

```
Your Computer (All Offline):
┌────────────────────────────────────────────────┐
│                                                │
│  🖥️  Tauri Frontend (Desktop App)              │
│      Main learning interface                   │
│      ├── Q&A system                            │
│      ├── Tutorial viewer                       │
│      └── Suggestion engine                     │
│                ↕ HTTP                           │
│                                                │
│  🧠 RAG Server (Flask - Port 5000)             │
│      http://127.0.0.1:5000                     │
│      ├── Educational Q&A endpoint              │
│      ├── Scene analysis                        │
│      ├── Tutorial management                   │
│      └── RAG (Blender API docs)                │
│                ↕ HTTP                           │
│                                                │
│  🤖 Ollama (Local LLM - Port 11434)            │
│      Model: qwen2.5:7b-instruct-q4_K_M         │
│      Educational explanations (no code!)       │
│                                                │
│  🎨 Blender Addon                              │
│      Sidebar panel ("Learn" tab)               │
│      ├── Scene data export                     │
│      ├── Question input                        │
│      └── Suggestion display                    │
│                                                │
└────────────────────────────────────────────────┘
```

**Everything runs on localhost—completely offline!**

---

## 📋 Requirements

- **OS:** Windows, macOS, or Linux
- **RAM:** 6-8GB minimum (for quantized 7B model)
- **Storage:** ~5GB for model and dependencies
- **Python:** 3.8+ (for RAG server)
- **Node.js:** 16+ (for Tauri frontend)
- **Rust:** Latest stable (for Tauri build)
- **Blender:** 3.0+ (4.x recommended)
- **Ollama:** Local LLM server

---

## 🛠️ Setup for Development

If you want to build from source or contribute:

### 1. Install Ollama & Model (2 min)

Download from [ollama.com](https://ollama.com) and install.

Pull the educational model:
```bash
ollama pull qwen2.5:7b-instruct-q4_K_M
```

**Why this model?**
- Only ~4.7GB (quantized)
- Great for educational explanations
- Runs on 6-8GB RAM

**For even lower-end hardware:**
```bash
ollama pull qwen2.5:7b-instruct-q3_K_M  # ~3.5GB
```

### 2. Install Build Tools

- **Python** 3.10+: https://python.org/downloads/
- **Node.js** 18+: https://nodejs.org/
- **Rust**: https://rustup.rs/

### 3. Clone and Build

```bash
git clone <repository-url>
cd smolpc-blenderhelper
npm install
npm run tauri build
```

Or use the build script:
```bash
# Windows
build_app.bat

# Linux/macOS
chmod +x build_app.sh
./build_app.sh
```

### 4. Development Mode

**Note:** In development mode, you still need to manually start the RAG server:

```bash
# Terminal 1: RAG Server
cd rag_system
python server.py

# Terminal 2: Tauri App (auto-starts server in production builds)
npm run tauri dev
```

### 5. Install Blender Addon (30 sec)

**Windows:** Double-click `install_to_blender.bat`

**Manual:**
1. Open Blender
2. Edit → Preferences → Add-ons → Install
3. Select: `blender_addon/blender_helper_http.py`
4. Enable "3D View: Blender Learning Assistant"
5. Press `N` in viewport → Click "Learn" tab

---

## 💡 How to Use

### From Tauri App (Main Interface)

1. **Ask Questions**
   - Type: "What is a modifier?"
   - Click "Ask"
   - Get detailed, educational answers

2. **Get Suggestions**
   - Click "Get Suggestions"
   - See 3-5 things to try next based on your scene

3. **Follow Tutorials**
   - Click "Load Tutorials"
   - Choose a tutorial
   - Follow step-by-step with validation

### From Blender (In-Viewport)

1. Press `N` to open sidebar
2. Click "Learn" tab
3. Ask questions directly in Blender
4. See current scene info
5. Get quick suggestions

---

## 📚 Built-in Tutorials

1. **Basic 3D Modeling** (5 steps)
   - Create objects, enter Edit Mode, add modifiers

2. **Introduction to Modifiers** (4 steps)
   - Bevel, Array, Subdivision Surface

3. **Basic Materials** (4 steps)
   - Add colors and shading

4. **Navigating the 3D Viewport** (5 steps)
   - Camera controls and navigation

**Add your own tutorials** by editing `rag_system/tutorials.json`

---

## 🎯 Example Questions

**Beginner:**
- "What does the Tab key do?"
- "How do I move objects?"
- "What is Edit Mode?"

**Intermediate:**
- "How do modifiers work?"
- "What's the difference between Object and Edit Mode?"
- "How do I create materials?"

**Advanced:**
- "How do Array and Mirror modifiers work together?"
- "What's the Subdivision Surface algorithm?"
- "How does the shader editor work?"

---

## 📁 Project Structure

```
smolpc-blenderhelper/
├── src/                        # Tauri frontend (Svelte 5 + Tailwind CSS 4)
│   ├── index.html             # HTML entry point
│   ├── main.ts                # Svelte mount point
│   ├── App.svelte             # Main app component
│   ├── app.css                # Tailwind CSS 4 theme
│   └── lib/                   # Components, stores, types, utils
│
├── src-tauri/                  # Tauri desktop shell (Rust)
│   ├── src/
│   │   └── main.rs            # Tauri app entry point
│   └── tauri.conf.json        # App configuration
│
├── rag_system/                 # Educational AI backend
│   ├── server.py              # Flask server with endpoints
│   ├── tutorials.json         # Tutorial content
│   ├── build_database.py      # RAG indexer
│   └── simple_db/             # Vector database
│
├── blender_addon/
│   └── blender_helper_http.py # Scene export addon
│
├── start_server.bat            # RAG server launcher (Windows)
├── start_server.sh             # RAG server launcher (Linux/Mac)
├── GETTING_STARTED.md          # Comprehensive guide
└── README.md                   # This file
```

---

## 🔧 Configuration

### Change Model

```bash
# Windows (PowerShell)
$env:OLLAMA_MODEL="qwen2.5:7b-instruct-q3_K_M"

# Linux/Mac
export OLLAMA_MODEL="qwen2.5:7b-instruct-q3_K_M"
```

### Adjust Window Size

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

---

## 🐛 Troubleshooting

### "Cannot connect to RAG server"

**Check if server is running:**
```bash
curl http://127.0.0.1:5000/health
```

**Expected:** `{"status": "ok", "rag_enabled": true}`

### "Ollama not running"

```bash
# Check status
curl http://127.0.0.1:11434/api/tags

# Start if needed
ollama serve
```

### Slow Responses

Use smaller quantization:
```bash
ollama pull qwen2.5:7b-instruct-q2_K
export OLLAMA_MODEL="qwen2.5:7b-instruct-q2_K"
```

### Addon Not Showing

1. Check Blender console for errors
2. Verify addon is enabled in Preferences
3. Check `requests` library is available in Blender

**More solutions:** See [GETTING_STARTED.md](GETTING_STARTED.md)

---

## 🎓 Why Educational Focus?

### What Changed in v4.0

| Feature | Old (v3.0) | New (v4.0) |
|---------|-----------|-----------|
| **Purpose** | Code generation | Education |
| **Model** | qwen2.5-coder (code) | qwen2.5 (general) |
| **Output** | Python code | Explanations |
| **Execution** | Automatic | Manual learning |
| **Hardware** | 20GB+ RAM | 6-8GB RAM |
| **Focus** | Automation | Understanding |

### Why We Changed

1. **Smaller models can't reliably generate code** - Too many API hallucinations
2. **Students learn by doing, not watching** - Copy-paste doesn't teach
3. **Accessibility** - Runs on student laptops (low RAM)
4. **Safety** - No risk of executing bad code
5. **True learning** - Understanding > Automation

---

## 📊 Technical Details

### RAG System

- **Embeddings:** all-MiniLM-L6-v2 (sentence-transformers)
- **Storage:** NumPy arrays (simple, no ChromaDB needed)
- **Documents:** ~150 Blender API pages
- **Retrieval:** Cosine similarity, top-3 results

### LLM Configuration

- **Model:** qwen2.5:7b-instruct-q4_K_M
- **Quantization:** 4-bit (Q4_K_M)
- **Temperature:** 0.7 (creative but accurate)
- **Timeout:** 60s per request
- **Mode:** Non-streaming

### Validation System

Checks if students:
- Created required objects (`has_object_type`)
- Added specific modifiers (`has_modifier`)
- Reached progress milestones (`object_count`)

---

## 🚀 Build for Production

```bash
npm run tauri build
```

Installer output: `src-tauri/target/release/bundle/`

**Distribute:**
- Tauri installer (Windows .msi, Mac .dmg, Linux .deb)
- RAG server folder (`rag_system/`)
- Blender addon file
- GETTING_STARTED.md

---

## 📄 License

MIT License - See LICENSE file

---

## 🙏 Acknowledgments

- **Ollama Team** - Local LLM infrastructure
- **Alibaba Qwen Team** - Qwen2.5 models
- **Sentence Transformers** - Embedding models
- **Blender Foundation** - Incredible software and docs
- **Tauri Team** - Modern desktop framework

---

## 📞 Support

- **Full Guide:** [GETTING_STARTED.md](GETTING_STARTED.md)
- **Issues:** [GitHub Issues](https://github.com/SmolPC-2-0/smolpc-blenderhelper/issues)

---

## ⚡ Quick Reference

| Task | Command |
|------|---------|
| **Start RAG Server** | `python rag_system/server.py` |
| **Start Tauri App** | `npm run tauri dev` |
| **Start Ollama** | `ollama serve` |
| **Build Database** | `python rag_system/build_database.py` |
| **Test Server** | `curl http://127.0.0.1:5000/health` |
| **Install Addon** | `install_to_blender.bat` (Windows) |

---

**Happy Learning! 🎓🎨**

*Transform from beginner to Blender pro with AI-powered guidance.*
