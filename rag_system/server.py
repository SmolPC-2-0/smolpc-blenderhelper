"""
RAG HTTP Server for Blender Helper AI

This server runs in system Python (outside Blender) and handles:
- RAG (vector search with ChromaDB or simple NumPy)
- Ollama LLM queries
- Returns generated code to Blender via HTTP

Runs completely offline on localhost:5000
"""

from flask import Flask, request, jsonify
from flask_cors import CORS
import os
import sys
from pathlib import Path
import traceback
import pickle
import numpy as np

# Try to import RAG dependencies
try:
    from sentence_transformers import SentenceTransformer
    HAS_TRANSFORMERS = True
except ImportError:
    print("⚠️  sentence-transformers not found. RAG will be disabled.")
    HAS_TRANSFORMERS = False

try:
    import requests as req_lib
    HAS_REQUESTS = True
except ImportError:
    print("❌ requests library required!")
    sys.exit(1)


app = Flask(__name__)
# CORS restricted to localhost origins only for security
CORS(app, origins=[
    'http://127.0.0.1:*',
    'http://localhost:*',
    'tauri://localhost'
])

# Configuration
RAG_DIR = Path(__file__).parent
DB_PATH = RAG_DIR / "simple_db"


class RAGSystem:
    """Simple RAG using NumPy arrays."""

    def __init__(self):
        self.initialized = False
        self.embeddings = None
        self.metadata = None
        self.embedding_model = None

    def initialize(self):
        """Load the RAG database."""
        if self.initialized:
            return True

        if not HAS_TRANSFORMERS:
            print("⚠️  RAG disabled - sentence-transformers not installed")
            return False

        try:
            print("Loading RAG system...")

            # Load embedding model
            self.embedding_model = SentenceTransformer('all-MiniLM-L6-v2')

            # Load database
            embeddings_file = DB_PATH / "embeddings.npy"
            metadata_file = DB_PATH / "metadata.pkl"

            if not embeddings_file.exists() or not metadata_file.exists():
                print(f"⚠️  Database not found at {DB_PATH}")
                print("   Run indexer_simple.py first to build the knowledge base")
                return False

            self.embeddings = np.load(embeddings_file)

            # SECURITY NOTE: pickle.load() is used here for bundled application resources only.
            # The metadata.pkl file is generated during build and bundled with the application.
            # This file is never loaded from user input or external sources.
            # Alternative: If security is a concern, consider switching to JSON format.
            with open(metadata_file, 'rb') as f:
                self.metadata = pickle.load(f)

            self.initialized = True
            print(f"[OK] RAG loaded: {len(self.metadata)} documents")
            return True

        except Exception as e:
            print(f"❌ RAG initialization failed: {e}")
            traceback.print_exc()
            return False

    def retrieve_context(self, query, n_results=3):
        """Retrieve relevant documentation."""
        if not self.initialize():
            return []

        try:
            # Embed query
            query_embedding = self.embedding_model.encode([query])[0]

            # Calculate cosine similarity
            similarities = np.dot(self.embeddings, query_embedding) / (
                np.linalg.norm(self.embeddings, axis=1) * np.linalg.norm(query_embedding)
            )

            # Get top N
            top_indices = np.argsort(similarities)[-n_results:][::-1]

            # Format results
            contexts = []
            for idx in top_indices:
                chunk = self.metadata[idx]
                contexts.append({
                    'text': chunk['text'],
                    'signature': chunk['signature'],
                    'url': chunk['url'],
                    'similarity': float(similarities[idx])
                })

            return contexts

        except Exception as e:
            print(f"❌ Context retrieval failed: {e}")
            traceback.print_exc()
            return []


# Global RAG instance
rag = RAGSystem()

# Global scene data cache (last received from Blender)
cached_scene_data = {
    'scene_data': None,
    'last_update': None
}


def call_ollama(system_prompt, user_prompt, model=None, temperature=0.7, timeout=120):
    """Call local Ollama API."""
    if model is None:
        model = os.getenv("OLLAMA_MODEL", "qwen2.5:7b-instruct-q4_K_M")

    payload = {
        "model": model,
        "stream": False,
        "options": {"temperature": temperature},
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_prompt}
        ]
    }

    try:
        response = req_lib.post(
            "http://127.0.0.1:11434/api/chat",
            json=payload,
            timeout=timeout
        )
        response.raise_for_status()
        data = response.json()
        return data["message"]["content"]
    except req_lib.exceptions.ConnectionError:
        raise Exception("Ollama not running. Start it with: ollama serve")
    except Exception as e:
        raise Exception(f"Ollama request failed: {e}")


def extract_code_block(text):
    """Extract Python code from markdown."""
    import re
    match = re.search(r"```(?:python)?\s*(.*?)```", text, re.DOTALL)
    if match:
        return match.group(1).strip()
    return text.strip()


@app.route('/health', methods=['GET'])
def health():
    """Health check endpoint."""
    return jsonify({
        'status': 'ok',
        'rag_enabled': rag.initialized,
        'rag_docs': len(rag.metadata) if rag.initialized else 0
    })


@app.route('/scene/update', methods=['POST'])
def update_scene():
    """Receive scene data from Blender addon and cache it."""
    try:
        data = request.json
        scene_data = data.get('scene_data', {})

        # Update cache
        import time
        cached_scene_data['scene_data'] = scene_data
        cached_scene_data['last_update'] = time.time()

        return jsonify({'status': 'ok', 'message': 'Scene data updated'})

    except Exception as e:
        print(f"❌ Error updating scene: {e}")
        return jsonify({'error': str(e)}), 500


@app.route('/scene/current', methods=['GET'])
def get_current_scene():
    """Get the cached scene data (for frontend)."""
    try:
        if cached_scene_data['scene_data'] is None:
            return jsonify({
                'connected': False,
                'message': 'No scene data available. Make sure Blender addon is installed and active.'
            })

        import time
        age = time.time() - cached_scene_data['last_update']

        # Consider stale if older than 30 seconds
        if age > 30:
            return jsonify({
                'connected': False,
                'message': 'Scene data is stale. Blender may not be connected.',
                'last_update': cached_scene_data['last_update']
            })

        return jsonify({
            'connected': True,
            'scene_data': cached_scene_data['scene_data'],
            'last_update': cached_scene_data['last_update']
        })

    except Exception as e:
        print(f"❌ Error getting scene: {e}")
        return jsonify({'error': str(e)}), 500


@app.route('/ask', methods=['POST'])
def ask_question():
    """Answer educational questions about Blender."""
    try:
        # Validate input
        data = request.json
        if data is None:
            return jsonify({'error': 'Invalid JSON or Content-Type must be application/json'}), 400

        question = data.get('question', '')
        scene_context = data.get('scene_context', {})

        # Validate question
        if not isinstance(question, str):
            return jsonify({'error': 'Question must be a string'}), 400

        question = question.strip()

        # Enforce input length limits (10,000 chars = ~2,500 words)
        if len(question) > 10000:
            return jsonify({'error': 'Question too long (max 10,000 characters)'}), 400

        # If no scene_context provided, use cached data
        if not scene_context and cached_scene_data['scene_data']:
            scene_context = cached_scene_data['scene_data']

        if not question:
            return jsonify({'error': 'No question provided'}), 400

        print(f"\n{'='*60}")
        print(f"Question: {question}")
        print(f"{'='*60}")

        # Retrieve relevant documentation
        contexts = rag.retrieve_context(question, n_results=3)

        if contexts:
            print(f"[OK] Retrieved {len(contexts)} relevant docs")
            context_section = "\n\n".join([
                f"### {ctx['signature']}\n{ctx['text']}"
                for ctx in contexts
            ])
        else:
            print("⚠️  No RAG context available")
            context_section = "(No specific documentation found)"

        # Format scene context
        scene_summary = ""
        if scene_context:
            scene_summary = f"""
Current Scene Information:
- Objects: {scene_context.get('object_count', 0)} total
- Active: {scene_context.get('active_object', 'None')}
- Mode: {scene_context.get('mode', 'OBJECT')}
"""

        # Educational prompt
        system_prompt = f"""You are a patient Blender instructor helping students learn 3D modeling through the Blender interface.

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

{scene_summary}

The documentation below contains Python code for reference ONLY - you must translate these concepts into UI actions:
{context_section}

Answer the student's question in a friendly, educational manner with UI-based instructions. Keep answers concise (2-4 paragraphs).

EXAMPLES OF GOOD RESPONSES:
- "To add a sphere, press Shift+A in the 3D Viewport, then navigate to Mesh > UV Sphere"
- "In the Modifier Properties panel (wrench icon), click Add Modifier and select Bevel"
- "Select your object, press Tab to enter Edit Mode, then press Ctrl+R to add an edge loop"

NEVER write responses like this:
- "Use bpy.ops.mesh.primitive_uv_sphere_add(radius=1.0)"
- "Run this Python code: ..."
- Any Python code snippets or bpy commands"""

        user_prompt = f"""Question: {question}

Provide a clear, educational answer that helps the student understand this Blender concept."""

        # Call Ollama
        print("🤖 Calling Ollama for educational response...")
        response = call_ollama(
            system_prompt,
            user_prompt,
            model=data.get('model'),
            temperature=0.7
        )

        print("[OK] Answer generated")
        print(f"{'='*60}\n")

        return jsonify({
            'answer': response.strip(),
            'contexts_used': len(contexts),
            'rag_enabled': rag.initialized
        })

    except Exception as e:
        error_msg = str(e)
        print(f"❌ Error: {error_msg}")
        traceback.print_exc()
        return jsonify({'error': error_msg}), 500


@app.route('/scene_analysis', methods=['POST'])
def analyze_scene():
    """Analyze scene and suggest next steps for learning."""
    try:
        # Validate input
        data = request.json
        if data is None:
            return jsonify({'error': 'Invalid JSON or Content-Type must be application/json'}), 400

        goal = data.get('goal', 'learning blender')
        scene_data = data.get('scene_data', {})

        # Validate goal
        if not isinstance(goal, str):
            return jsonify({'error': 'Goal must be a string'}), 400

        goal = goal.strip()

        # Enforce input length limits
        if len(goal) > 500:
            return jsonify({'error': 'Goal too long (max 500 characters)'}), 400

        # Validate scene_data is a dict
        if not isinstance(scene_data, dict):
            return jsonify({'error': 'Scene data must be an object'}), 400

        print(f"\n{'='*60}")
        print(f"Scene Analysis - Goal: {goal}")
        print(f"Objects in scene: {scene_data.get('object_count', 0)}")
        print(f"{'='*60}")

        # Format scene info
        objects_list = "\n".join([
            f"  - {obj['name']} ({obj['type']})" +
            (f" with {len(obj.get('modifiers', []))} modifiers" if obj.get('modifiers') else "")
            for obj in scene_data.get('objects', [])
        ])

        scene_summary = f"""Current Scene:
- Total objects: {scene_data.get('object_count', 0)}
- Active object: {scene_data.get('active_object', 'None')}
- Mode: {scene_data.get('mode', 'OBJECT')}
- Render engine: {scene_data.get('render_engine', 'Unknown')}

Objects:
{objects_list if objects_list else '  (empty scene)'}
"""

        # Educational suggestion prompt
        system_prompt = f"""You are a Blender instructor analyzing a student's scene to suggest what they should learn next.

{scene_summary}

Your task:
- Analyze what the student has already done
- Suggest 3-5 concrete next steps they could take to learn more
- Focus on natural progression (basics → intermediate → advanced)
- Each suggestion should be a learning opportunity
- Keep suggestions action-oriented and specific

Provide suggestions as a numbered list. Each suggestion should be ONE sentence that starts with an action verb."""

        user_prompt = f"""The student's goal is: {goal}

Based on their current scene, what should they try next to continue learning? Provide 3-5 specific suggestions."""

        # Call Ollama
        print("🤖 Generating suggestions...")
        response = call_ollama(
            system_prompt,
            user_prompt,
            model=data.get('model'),
            temperature=0.7
        )

        # Parse numbered list into array
        # Expected format: "1. First suggestion\n2. Second suggestion\n..."
        suggestions_list = []
        for line in response.strip().split('\n'):
            line = line.strip()
            if not line:
                continue
            # Remove leading number and punctuation (e.g., "1.", "1)", "1 -")
            import re
            cleaned = re.sub(r'^\d+[\.\)\-\:]\s*', '', line)
            if cleaned:
                suggestions_list.append(cleaned)

        print("[OK] Suggestions generated")
        print(f"{'='*60}\n")

        return jsonify({
            'suggestions': suggestions_list,
            'scene_summary': scene_summary
        })

    except Exception as e:
        error_msg = str(e)
        print(f"❌ Error: {error_msg}")
        traceback.print_exc()
        return jsonify({'error': error_msg}), 500


@app.route('/tutorial/list', methods=['GET'])
def list_tutorials():
    """List available tutorials."""
    try:
        tutorials_file = RAG_DIR / "tutorials.json"

        if not tutorials_file.exists():
            return jsonify({
                'tutorials': [],
                'message': 'No tutorials found. Create tutorials.json to add content.'
            })

        with open(tutorials_file, 'r', encoding='utf-8') as f:
            import json
            data = json.load(f)

        # Return just the tutorial metadata (not full steps)
        tutorial_list = [
            {
                'id': t['id'],
                'title': t['title'],
                'description': t['description'],
                'step_count': len(t.get('steps', []))
            }
            for t in data.get('tutorials', [])
        ]

        return jsonify({'tutorials': tutorial_list})

    except Exception as e:
        print(f"❌ Error loading tutorials: {e}")
        return jsonify({'error': str(e)}), 500


@app.route('/tutorial/step', methods=['POST'])
def get_tutorial_step():
    """Get a specific tutorial step with validation."""
    try:
        # Validate input
        data = request.json
        if data is None:
            return jsonify({'error': 'Invalid JSON or Content-Type must be application/json'}), 400

        tutorial_id = data.get('tutorial_id')
        step_number = data.get('step_number', 0)
        scene_data = data.get('scene_data', {})

        # Validate tutorial_id
        if not tutorial_id or not isinstance(tutorial_id, str):
            return jsonify({'error': 'Tutorial ID must be a non-empty string'}), 400

        # Validate step_number
        if not isinstance(step_number, int):
            return jsonify({'error': 'Step number must be an integer'}), 400

        if step_number < 0 or step_number > 1000:
            return jsonify({'error': 'Invalid step number'}), 400

        # Validate scene_data is a dict
        if not isinstance(scene_data, dict):
            return jsonify({'error': 'Scene data must be an object'}), 400

        tutorials_file = RAG_DIR / "tutorials.json"

        if not tutorials_file.exists():
            return jsonify({'error': 'Tutorials not found'}), 404

        with open(tutorials_file, 'r', encoding='utf-8') as f:
            import json
            tutorials_data = json.load(f)

        # Find tutorial
        tutorial = None
        for t in tutorials_data.get('tutorials', []):
            if t['id'] == tutorial_id:
                tutorial = t
                break

        if not tutorial:
            return jsonify({'error': f'Tutorial {tutorial_id} not found'}), 404

        steps = tutorial.get('steps', [])

        if step_number >= len(steps):
            return jsonify({
                'completed': True,
                'message': 'Tutorial complete! Great work!'
            })

        current_step = steps[step_number]

        # Validate previous step if not first
        validation_result = None
        if step_number > 0:
            prev_step = steps[step_number - 1]
            validation_result = validate_tutorial_step(prev_step, scene_data)

        return jsonify({
            'step': current_step,
            'step_number': step_number,
            'total_steps': len(steps),
            'validation': validation_result,
            'tutorial_title': tutorial['title']
        })

    except Exception as e:
        print(f"❌ Error getting tutorial step: {e}")
        traceback.print_exc()
        return jsonify({'error': str(e)}), 500


def validate_tutorial_step(step, scene_data):
    """Validate if student completed a tutorial step."""
    validation = step.get('validation', {})
    check_type = validation.get('check')
    params = validation.get('params', {})

    if not check_type:
        return {'validated': True, 'message': 'No validation for this step'}

    if check_type == 'has_object_type':
        # Check if scene has at least one object of the specified type
        target_type = params.get('type', 'MESH')
        for obj in scene_data.get('objects', []):
            if obj.get('type') == target_type:
                return {
                    'validated': True,
                    'message': f'[OK] Great! Found {obj["name"]} ({target_type})'
                }
        return {
            'validated': False,
            'message': f'⚠️  Try creating an object of type {target_type}'
        }

    elif check_type == 'has_modifier':
        # Check if active object has the specified modifier
        modifier_type = params.get('type')
        active_obj = scene_data.get('active_object')

        if not active_obj:
            return {
                'validated': False,
                'message': '⚠️  Select an object first'
            }

        # Find active object in scene data
        for obj in scene_data.get('objects', []):
            if obj.get('name') == active_obj:
                for mod in obj.get('modifiers', []):
                    if mod.get('type') == modifier_type:
                        return {
                            'validated': True,
                            'message': f'[OK] Perfect! {modifier_type} modifier added'
                        }

        return {
            'validated': False,
            'message': f'⚠️  Try adding a {modifier_type} modifier'
        }

    elif check_type == 'object_count':
        # Check if scene has at least N objects
        min_count = params.get('min', 1)
        current_count = scene_data.get('object_count', 0)

        if current_count >= min_count:
            return {
                'validated': True,
                'message': f'[OK] Great! You have {current_count} objects'
            }
        return {
            'validated': False,
            'message': f'⚠️  Create at least {min_count} objects (you have {current_count})'
        }

    return {'validated': True, 'message': 'Step validation not implemented'}


@app.route('/test', methods=['GET'])
def test():
    """Test endpoint."""
    return jsonify({
        'message': 'RAG Server is running!',
        'rag_enabled': rag.initialized,
        'endpoints': ['/health', '/ask', '/scene_analysis', '/tutorial/list', '/tutorial/step', '/test']
    })


def main():
    """Start the server."""
    print("\n" + "="*60)
    print("Blender Learning Assistant - RAG Server")
    print("="*60)
    print(f"Running at: http://127.0.0.1:5000")
    print("This server is OFFLINE - only accessible from this computer")
    print("")
    print("Educational Mode:")
    print("  - Q&A endpoint: POST /ask")
    print("  - Scene analysis: POST /scene_analysis")
    print("  - Tutorials: GET /tutorial/list, POST /tutorial/step")
    print("")
    print(f"Model: {os.getenv('OLLAMA_MODEL', 'qwen2.5:7b-instruct-q4_K_M')}")
    print("="*60 + "\n")

    # Try to initialize RAG
    if rag.initialize():
        print("[OK] RAG system ready (API documentation loaded)\n")
    else:
        print("⚠️  RAG disabled - will use LLM knowledge only\n")

    print("Press Ctrl+C to stop the server\n")

    # Run server
    app.run(host='127.0.0.1', port=5000, debug=False)


if __name__ == '__main__':
    main()
