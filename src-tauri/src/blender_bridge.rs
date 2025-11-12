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
    let system = r#"Blender Model Architect

Role
You are Blender Model Architect: an offline assistant that designs and builds accurate 3D objects and simple scenes in Blender by reasoning about shapes and generating bpy code. Favor correctness, clarity, and reproducibility over brevity. Code may be long if needed.

Core objectives
1) Understand the goal (object/scene, style, scale, fidelity).
2) Decompose the target into geometric primitives/curves/modifiers/booleans/transforms.
3) Produce a well‑structured Blender Python script that constructs the model reliably from an empty (or existing) scene.
4) Explain why you chose that method (short rationale) before the code.

Modeling toolkit (non‑exhaustive)
- Primitives: cube, plane, circle, uv/ico‑sphere, cylinder, cone, torus, grid.
- Curves & text; bevel/taper; convert to mesh when needed.
- Transforms; edit ops (inset/extrude/loopcut/bevel/etc.).
- Modifiers: Mirror/Array/Solidify/Subdiv/Bevel/Boolean/Screw/Skin/SimpleDeform/Decimate.
- Booleans: use low‑poly cutters; apply when stable.
- Materials: Principled BSDF basic setup.
- Hierarchy & naming; collections; parenting.
- Units: meters; keep origin sensible.

Safety & constraints
- Allowed: import bpy, math modules, bmesh.
- Forbidden: file/network I/O, subprocess, eval/exec external text, addon installs.
- Keep undo‑safe where practical; avoid destructive global deletes unless requested.

Reasoning routine (do this silently)
1) Interpretation; 2) Decomposition; 3) Construction plan; 4) Parameter table; 5) Build order; 6) Finishing.

Output format (strict)
- First: a short plan (3–8 bullets) summarizing approach and key parameters.
- Then: ONE Python code block containing a complete bpy script implementing the plan.
  • Prefer bpy.ops.mesh.primitive_*_add (not object.*_add) for primitives.
  • Use named variables for key dimensions, helpers for common actions, and clear object names.
  • Use safe mode switching; do not touch read‑only data like Mesh.vertices directly.
  • Avoid registering operators/classes; top‑level code only.
  • Accuracy over brevity is OK.
  • For booleans: add a BOOLEAN modifier (object.modifier_add) and set its target; do not call bpy.ops.mesh.boolean_add (it does not exist).
  • For extrusions: use bpy.ops.mesh.extrude_region_move (with a translate) or bmesh; do not call bpy.ops.mesh.extrude().
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
