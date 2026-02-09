# Installation Guide - Blender Learning Assistant

## System Requirements

- **Operating System:** Windows 10+, macOS, or Linux
- **Python:** 3.10 or higher
- **RAM:** 4GB minimum
- **Disk Space:** ~500 MB (including dependencies)
- **Ollama:** Required for LLM functionality (install separately)

---

## Quick Start

### Option 1: Use Pre-built Executable (Recommended)

1. **Download** the latest release from the releases page
2. **Install Python** 3.10+ if not already installed: https://www.python.org/downloads/
3. **Run the installer** (`blender_helper_x.x.x_x64.msi` on Windows)
4. **Launch** the app from your Start Menu or Applications folder

**First Launch:**
- The app will automatically detect Python
- If Python dependencies are missing, it will install them (takes 1-2 minutes)
- After initial setup, subsequent launches are instant

---

### Option 2: Build from Source

#### Prerequisites

1. **Node.js** 18+ and npm: https://nodejs.org/
2. **Rust** toolchain: https://rustup.rs/
3. **Python** 3.10+: https://www.python.org/downloads/

#### Build Steps

```bash
# 1. Clone the repository
git clone <repository-url>
cd smolpc-blenderhelper

# 2. Install frontend dependencies
npm install

# 3. Build the application
npm run tauri build

# The executable will be in: src-tauri/target/release/
# The installer will be in: src-tauri/target/release/bundle/
```

**Alternative: Use the build script**

Windows:
```cmd
build_app.bat
```

Linux/macOS:
```bash
chmod +x build_app.sh
./build_app.sh
```

---

## How It Works

The unified launcher automates the following:

1. **Python Detection** - Checks for Python 3.10+ on your system
2. **Dependency Installation** - Auto-installs Flask, sentence-transformers, etc. (first launch only)
3. **Server Startup** - Launches the RAG server in the background (hidden console window)
4. **Health Check** - Waits for server to be ready before showing UI
5. **Clean Shutdown** - Automatically stops the server when you close the app

---

## Troubleshooting

### "Python Not Found" Error

**Solution:**
1. Install Python 3.10+ from https://www.python.org/downloads/
2. During installation, check "Add Python to PATH"
3. Restart the app

**Verify Python installation:**
```bash
python --version
# or
python3 --version
```

---

### "Failed to Install Dependencies" Error

**Possible Causes:**
- No internet connection
- Pip not installed
- Permission issues

**Solutions:**

1. **Manual Installation:**
```bash
pip install -r rag_system/requirements_server.txt
```

2. **Use --user flag (if permission denied):**
```bash
pip install --user -r rag_system/requirements_server.txt
```

3. **Clear cache and retry:**
```bash
pip cache purge
pip install -r rag_system/requirements_server.txt
```

---

### "Server Failed to Start Within 30 Seconds"

**Possible Causes:**
- Port 5000 already in use
- Python dependencies not fully installed
- Ollama not running

**Solutions:**

1. **Check if port 5000 is in use:**
```bash
# Windows
netstat -ano | findstr :5000

# Linux/macOS
lsof -i :5000
```

2. **Check server logs:**
- Windows: `%APPDATA%\blender_helper\logs\`
- Linux/macOS: `~/.local/share/blender_helper/logs/`

3. **Verify Ollama is running:**
```bash
curl http://127.0.0.1:11434
```

---

### Cargo Build Errors (Access Denied on Windows)

If you encounter "Access is denied (os error 5)" when building:

**Solutions:**

1. **Disable antivirus temporarily** (Windows Defender or third-party)
2. **Add exception** for the `src-tauri/target/` directory
3. **Run as Administrator** (right-click build script → "Run as administrator")
4. **Close other programs** that might lock files (VS Code, file explorers)

---

## Advanced Configuration

### Changing RAG Server Port

Edit `rag_system/server.py`:
```python
if __name__ == '__main__':
    app.run(port=5000)  # Change to desired port
```

Also update `src-tauri/src/server_manager.rs`:
```rust
port: 5000,  // Change to match server.py
```

---

### Using a Different Ollama Model

Edit `rag_system/server.py`:
```python
model_name = "qwen2.5:7b-instruct-q4_K_M"  # Change to your model
```

Available models: https://ollama.com/library

---

### Viewing Server Logs

**While app is running:**
- Go to Settings → View Logs

**Manual access:**
- Windows: `%APPDATA%\blender_helper\logs\server_YYYY-MM-DD.log`
- Linux: `~/.local/share/blender_helper/logs/server_YYYY-MM-DD.log`
- macOS: `~/Library/Application Support/blender_helper/logs/server_YYYY-MM-DD.log`

---

## Blender Addon Installation

The app requires the Blender addon to send scene data:

1. **Open Blender** → Edit → Preferences → Add-ons
2. **Click "Install"** and select `blender_addon/blender_helper_http.py`
3. **Enable** the addon by checking the checkbox
4. **The addon runs automatically** in the background (no UI)

**Verify it's working:**
- Open the Blender Learning Assistant app
- The "Blender" indicator should turn green when Blender is open

---

## Uninstallation

### Windows
1. Control Panel → Programs → Uninstall a program
2. Select "Blender Learning Assistant" → Uninstall
3. (Optional) Delete app data: `%APPDATA%\blender_helper\`

### macOS
1. Delete the app from Applications folder
2. (Optional) Delete app data: `~/Library/Application Support/blender_helper/`

### Linux
1. Delete the app binary
2. (Optional) Delete app data: `~/.local/share/blender_helper/`

---

## Frequently Asked Questions

### Do I need to keep Python installed?

Yes, the RAG server is written in Python and requires a Python runtime. The app uses your system's Python installation.

### Does it work offline?

Partially:
- The app itself works offline
- Ollama (LLM) works offline after models are downloaded
- The RAG system works offline (uses local embeddings)
- However, downloading Python dependencies and Ollama models requires internet

### Can I use a different LLM provider?

Yes, but it requires code changes. The RAG server currently uses Ollama's API. You could modify `rag_system/server.py` to use OpenAI, Anthropic, or other providers.

### Why is first launch slow?

The first launch installs Python dependencies (~100 MB download + processing). This only happens once. Subsequent launches are fast (2-3 seconds).

### Can I run multiple instances?

No, only one instance can run at a time because the server uses port 5000. Attempting to launch a second instance will fail with a port conflict error.

---

## Support

- **Issues:** https://github.com/your-repo/issues
- **Documentation:** See README.md and CLAUDE.md
- **Logs:** Check `%APPDATA%\blender_helper\logs\` for debugging

---

**Last Updated:** 2026-02-05
**Version:** 4.0.0
