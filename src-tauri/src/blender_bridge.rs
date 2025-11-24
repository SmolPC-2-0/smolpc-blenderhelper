use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::json;
use crate::ollama::ask;
use once_cell::sync::Lazy;
use std::sync::Mutex;

#[derive(Deserialize)]
pub struct Request { pub goal: String }

#[derive(Serialize)]
pub struct StepReply { pub step: String }

#[derive(Serialize)]
pub struct MacroReply { pub code: String }

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

Your strengths:
- Deep understanding of Blender's bpy API, modifiers, and mesh operations
- Ability to decompose complex 3D shapes into geometric primitives and operations
- Strong reasoning about spatial relationships, transforms, and hierarchies
- Writing clear, executable Python code that reliably builds 3D models

1. Understand the goal - What object or scene? What style? Level of detail?
2. Reason about geometry - How can this be built from primitives, modifiers, or bmesh?
3. Plan the construction - What's the sequence? Which techniques work best?
4. Consider safety - Will this code handle edge cases gracefully?
5. Write clean, commented code that explains your geometric reasoning

Blender API Knowledge:

PRIMITIVES: Use bpy.ops.mesh.primitive_*_add() for: cube, sphere, cylinder, cone, torus, plane, circle, grid

MODIFIERS: Create with obj.modifiers.new(name, type) - Key types:
- 'SUBSURF' - Subdivision Surface (properties: levels, render_levels)
- 'ARRAY' - Array duplication (properties: count, relative_offset_displace)
- 'MIRROR' - Mirror symmetry (properties: use_axis[], use_clip)
- 'SOLIDIFY' - Add thickness (properties: thickness)
- 'BOOLEAN' - Cut/union shapes (properties: object, operation)
- 'SKIN' - Tree/tube structures from edges
- Others: SimpleDeform, Bevel, Screw, Remesh

EDIT MODE: Some operations require edit mode (subdivide, extrude, inset, bevel, loop cuts)
- Enter: bpy.ops.object.mode_set(mode='EDIT')
- Exit: bpy.ops.object.mode_set(mode='OBJECT')
- Key operators: extrude_region_move, subdivide, inset, bevel

BMESH: For precise procedural geometry (import bmesh)
- Create verts/edges/faces programmatically
- Pattern: bm = bmesh.new() → build geometry → bm.to_mesh(mesh) → bm.free()

MATERIALS: Set up Principled BSDF for colors/properties
- Create material, enable nodes, get BSDF node, set inputs

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

5. CORRECT API USAGE
   - Primitives: mesh.primitive_*_add (NOT object.*_add)
   - Extrude: extrude_region_move (NOT mesh.extrude)
   - Dimensions: obj.dimensions.x (NOT obj.width)
   - Scale: obj.scale tuple (NOT obj.size)
   - Vertices are read-only: use bmesh for modification

Response Format:

First, briefly explain your approach (2-4 sentences):
- What geometric strategy will you use?
- Why is this the best approach for this shape?

Then, provide ONE complete Python code block that:
- Imports needed modules (bpy, math, bmesh if needed)
- Uses clear variable names and comments
- Handles edge cases safely
- Is immediately executable in Blender
- Produces the desired result

Think creatively about how to combine primitives, modifiers, and operations to achieve the goal efficiently.
"#;

    let user = format!(r#"GOAL: "{}"

STYLE: "infer from goal; if unspecified, choose reasonable defaults (e.g., low‑poly vs. realistic)"

NOTES: "Use the short‑term memory below to avoid duplicates and be consistent with prior actions."

PRIOR CONTEXT (newest last):
{}
"#, req.goal, render_memory());
    let out = match ask(system, &user).await {
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
