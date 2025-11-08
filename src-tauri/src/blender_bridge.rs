use axum::Json;
use serde::{Deserialize, Serialize};
use crate::ollama::ask;

#[derive(Deserialize)]
pub struct Request { pub goal: String }

#[derive(Serialize)]
pub struct StepReply { pub step: String }

#[derive(Serialize)]
pub struct MacroReply { pub code: String }

pub async fn next_step(Json(req): Json<Request>) -> Json<StepReply> {
    let prompt = format!(
        "You are a concise Blender tutor. User goal: {}\n\
        Respond with one or two short sentences describing the very next step only.\n\
        Do not include Markdown, backticks, or code.",
        req.goal
    );
    let out = match ask(&prompt).await {
        Ok(t) if !t.trim().is_empty() => t.trim().to_string(),
        _ => "No suggestion available (LLM offline).".into(),
    };
    Json(StepReply { step: out })
}

pub async fn run_macro(Json(req): Json<Request>) -> Json<MacroReply> {
    let prompt = format!(
        "Write Blender Python code (bpy) to: {}\n\
        Constraints:\n\
        - Return ONLY executable Python code, no explanations, no Markdown, no backticks.\n\
        - Do NOT define classes or register operators/panels (no bpy.types.*, no bpy.utils.register_class).\n\
        - Prefer direct bpy.ops / data API.\n\
        - Be short and undo-safe.\n\
        - Start by ensuring OBJECT mode (if needed).\n\
        - If creating a pyramid use cone with vertices=4.\n\
        - If creating a 2x2x2 cube use primitive_cube_add(size=2).\n\
        - Ensure the created/edited object becomes the active object.\n\
        - Avoid edit-mode-only operators (like mesh.select_all) unless you also switch to EDIT and back to OBJECT.",
        req.goal
    );
    let out = match ask(&prompt).await {
        Ok(t) if !t.trim().is_empty() => t,
        _ => String::from("# No code returned - LLM offline"),
    };
    Json(MacroReply { code: out })
}
