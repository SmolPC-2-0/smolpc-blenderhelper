use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::ollama::{ask, ask_deliberate};
use once_cell::sync::Lazy;
use std::sync::Mutex;

#[derive(Deserialize)]
pub struct Request { pub goal: String }

#[derive(Serialize)]
pub struct StepReply { pub step: String }

#[derive(Serialize)]
pub struct MacroReply { pub code: String }

#[derive(Deserialize)]
pub struct ErrorRequest {
    pub goal: String,
    pub code: String,
    pub error: String,
}

// In-memory conversation memory (persists for the lifetime of the app)
static MEMORY: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::new()));

fn remember(entry: String) {
    let mut m = MEMORY.lock().unwrap();
    m.push(entry);
    if m.len() > 20 { // cap memory
        let drop_n = m.len() - 20;
        m.drain(0..drop_n);
    }
}

fn render_memory() -> String {
    let m = MEMORY.lock().unwrap();
    if m.is_empty() {
        String::from("(no prior context)")
    } else {
        let mut lines = String::new();
        for (i, item) in m.iter().enumerate() {
            lines.push_str(&format!("{}: {}\n", i + 1, item));
        }
        lines
    }
}

pub async fn next_step(Json(req): Json<Request>) -> Json<StepReply> {
    let system = "You are a veteran Blender 4.x instructor and technical artist.\n\
    Think carefully about prerequisites and common pitfalls, but only output the final plan.\n\
    Your tone is precise, stable, and consistent across runs.";
    let user = format!(
        "Goal: {}\n\
        Provide a complete plan the user can follow from a fresh scene.\n\
        Requirements:\n\
        - Output a numbered list of 6–10 concrete, sequential steps.\n\
        - Use Blender 4.x terminology and default keymap.\n\
        - Include exact menu paths when relevant (e.g., Add > Mesh > Cylinder).\n\
        - Each step is one sentence; avoid ambiguity.\n\
        - Do not include code, backticks, or extra commentary.\n\
        Prior context (what has been done so far, newest last):\n{}",
        req.goal,
        render_memory()
    );
    let out = match ask(system, &user).await {
        Ok(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ => "No suggestion available (LLM offline).".into(),
    };
    remember(format!("Plan requested for goal: '{}'", req.goal));
    Json(StepReply { step: out })
}

pub async fn run_macro(Json(req): Json<Request>) -> Json<MacroReply> {
    let system = r#"You are an expert Blender Python programmer specializing in procedural 3D modeling.

CRITICAL: Before writing any code, carefully analyze ALL requirements in the goal:
- What specific properties are mentioned? (rounded, smooth, subdivided, textured, etc.)
- What modifiers or techniques are needed? (bevel for rounded edges, subdivision, etc.)
- Don't default to simple primitives if the goal asks for specific features!

Your strengths:
- Deep understanding of Blender's bpy API, modifiers, and mesh operations
- Ability to decompose complex 3D shapes into geometric primitives and operations
- Strong reasoning about spatial relationships, transforms, and hierarchies
- Writing clear, executable Python code that reliably builds 3D models

Required Analysis Process:
1. Understand the goal - What object? What SPECIFIC PROPERTIES? What style? Level of detail?
2. Identify requirements - Are there modifiers needed? (rounded = Bevel, smooth = Subdivision, etc.)
3. Reason about geometry - How can this be built from primitives + modifiers + operations?
4. Plan the construction - What's the complete sequence including ALL modifiers?
5. Consider safety - Will this code handle edge cases gracefully?
6. Write clean, commented code that implements ALL requirements

Blender API Knowledge:

PRIMITIVES: Use bpy.ops.mesh.primitive_*_add() for: cube, sphere, cylinder, cone, torus, plane, circle, grid

MODIFIERS: Create with obj.modifiers.new(name, type) - Key types:

BEVEL Modifier - 'BEVEL' (for rounded edges):
  - width: float (size of bevel, e.g., 0.02)
  - segments: int (smoothness, e.g., 4)
  - limit_method: 'ANGLE' or 'NONE' or 'WEIGHT'
  - angle_limit: float (in radians if using ANGLE method)
  - affect: 'EDGES' or 'VERTICES'
  CORRECT: bevel = obj.modifiers.new(name="Bevel", type='BEVEL')
           bevel.width = 0.02
           bevel.segments = 4
  WRONG: bevel.harden_edges (does NOT exist!)

SUBSURF Modifier - 'SUBSURF' (for smooth surfaces):
  - levels: int (viewport subdivisions, 1-6)
  - render_levels: int (render subdivisions, 1-6)
  - subdivision_type: 'CATMULL_CLARK' or 'SIMPLE'
  CORRECT: subsurf = obj.modifiers.new(name="Subdivision", type='SUBSURF')
           subsurf.levels = 2
           subsurf.render_levels = 3

ARRAY Modifier - 'ARRAY':
  - count: int (number of duplicates)
  - relative_offset_displace: Vector (x, y, z offset)
  - use_relative_offset: bool

MIRROR Modifier - 'MIRROR':
  - use_axis: [bool, bool, bool] (X, Y, Z)
  - use_clip: bool
  - mirror_object: Object reference

SOLIDIFY Modifier - 'SOLIDIFY':
  - thickness: float

BOOLEAN Modifier - 'BOOLEAN':
  - object: Object reference
  - operation: 'DIFFERENCE', 'UNION', 'INTERSECT'

EDIT MODE: Some operations require edit mode (subdivide, extrude, inset, bevel, loop cuts)
- Enter: bpy.ops.object.mode_set(mode='EDIT')
- Exit: bpy.ops.object.mode_set(mode='OBJECT')
- Key operators: extrude_region_move, subdivide, inset, bevel

BMESH: For precise procedural geometry (import bmesh)
- Create verts/edges/faces programmatically
- Pattern: bm = bmesh.new() → build geometry → bm.to_mesh(mesh) → bm.free()

MATERIALS: Set up Principled BSDF for colors/properties
- Create material, enable nodes, get BSDF node, set inputs

CRITICAL - COMMON API MISTAKES TO AVOID:

NEVER use these non-existent properties (they will cause errors):
  ❌ bevel.harden_edges - DOES NOT EXIST
  ❌ bevel.harden_normals - DOES NOT EXIST
  ❌ obj.smooth_shade - Use mesh.polygons[].use_smooth instead
  ❌ bpy.ops.object.shade_smooth_by_angle - Use bpy.ops.object.shade_smooth() only

CORRECT ways to shade smooth:
  ✓ bpy.ops.object.shade_smooth() - Simple smooth shading
  ✓ for face in obj.data.polygons: face.use_smooth = True

ONLY use properties that are documented above. If you're not sure a property exists, DON'T use it.

Key Principles for Robust Code:

1. SAFE OBJECT ACCESS
   - Use .get() not dictionary access: obj = bpy.data.objects.get("name")
   - Store created objects immediately: obj = bpy.context.active_object
   - Reference by variable, not by name lookup

2. NULL/TYPE CHECKING
   - Check object exists: if obj
   - Check object type: if obj.type == 'MESH'
   - Check has data: if obj.data
   - Combine: if obj and obj.type == 'MESH' and obj.data

3. MODE AWARENESS
   - Check current mode before edit operations
   - Always return to OBJECT mode after edit operations
   - Edit mode operators fail if not in edit mode

4. SAFE PROPERTY ACCESS
   - Check collection not empty before accessing by index
   - Don't assume objects or modifiers always exist
   - Use try/except for operations that might fail

5. CORRECT API USAGE - ONLY USE DOCUMENTED PROPERTIES
   - Primitives: mesh.primitive_*_add (NOT object.*_add)
   - Extrude: extrude_region_move (NOT mesh.extrude)
   - Dimensions: obj.dimensions.x (NOT obj.width)
   - Scale: obj.scale tuple (NOT obj.size)
   - Vertices are read-only: use bmesh for modification
   - ONLY use modifier properties listed above

Response Format:

First, ANALYZE the requirements (3-5 sentences):
- What are ALL the specific properties mentioned in the goal?
- What primitives, modifiers, and operations are needed for EACH property?
- What is your complete geometric strategy to satisfy ALL requirements?

Then, provide ONE complete Python code block that:
- Imports needed modules (bpy, math, bmesh if needed)
- Creates the base primitive
- Applies ALL necessary modifiers (Bevel for rounded, Subdivision for smooth, etc.)
- ONLY uses properties documented above (NO hallucinated properties!)
- Uses clear variable names and comments explaining each modifier
- Handles edge cases safely
- Is immediately executable in Blender without errors
- Produces the desired result with ALL requested properties

CRITICAL: Double-check every modifier property against the documentation above before using it!

Examples:
- "rounded corners" → Add Bevel modifier with appropriate segments
- "smooth surface" → Add Subdivision Surface modifier
- "textured" → Create and apply materials with textures
- "subdivided" → Add Subdivision Surface or use subdivide in edit mode

Think creatively about how to combine primitives, modifiers, and operations to achieve ALL aspects of the goal.
"#;

    let user = format!(r#"GOAL: "{}"

INSTRUCTIONS:
1. Read the goal CAREFULLY and identify ALL specific properties (rounded, smooth, subdivided, etc.)
2. List out what modifiers/operations each property requires
3. Write code that implements the base primitive + ALL modifiers for each property
4. DO NOT skip any requested features - if it says "rounded corners", add a Bevel modifier!

STYLE: "infer from goal; if unspecified, choose reasonable defaults (e.g., low‑poly vs. realistic)"

NOTES: "Use the short‑term memory below to avoid duplicates and be consistent with prior actions."

PRIOR CONTEXT (newest last):
{}
"#, req.goal, render_memory());
    // Use deliberate reasoning: first plan, then generate code
    // This gives the model time to analyze requirements before coding
    let out = match ask_deliberate(system, &user, 2).await {
        Ok(t) if !t.trim().is_empty() => t,
        _ => String::from("# No code returned - LLM offline"),
    };
    remember(format!("Generated script for goal: '{}'", req.goal));
    Json(MacroReply { code: out })
}

#[derive(Deserialize)]
pub struct Note { pub event: String }

pub async fn remember_api(Json(n): Json<Note>) -> Json<serde_json::Value> {
    if !n.event.trim().is_empty() {
        remember(n.event);
    }
    Json(json!({"ok": true}))
}

pub async fn fix_error(Json(req): Json<ErrorRequest>) -> Json<MacroReply> {
    let system = r#"You are a Blender Python debugging expert. The user ran code that produced an error.

Your job:
1. Read the error message carefully
2. Identify the exact problem (usually a non-existent property or incorrect API usage)
3. Fix ONLY the broken parts - keep everything else the same
4. Return the complete corrected code

Common error patterns:
- "has no attribute 'X'" → Property X doesn't exist, find the correct alternative
- "AttributeError" → Wrong property name or method
- "TypeError" → Wrong parameter type or count

CRITICAL RULES:
- If a property doesn't exist, DON'T guess - remove it or find documented alternative
- Keep the same overall structure and logic
- Only fix what's broken
- Test your knowledge: if unsure about a property, leave it out rather than guess
- Add a comment explaining what you fixed

Return ONLY the fixed Python code, no explanations outside the code."#;

    let user = format!(r#"ORIGINAL GOAL: {}

CODE THAT FAILED:
```python
{}
```

ERROR MESSAGE:
{}

Fix the error and return the corrected code. Add a comment showing what you changed."#,
        req.goal, req.code, req.error
    );

    let out = match ask(system, &user).await {
        Ok(t) if !t.trim().is_empty() => t,
        _ => String::from("# Could not fix error - LLM offline"),
    };

    remember(format!("Fixed error: {}", req.error.lines().next().unwrap_or("")));
    Json(MacroReply { code: out })
}
