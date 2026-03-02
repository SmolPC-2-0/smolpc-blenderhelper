use crate::rag::types::RagContext;
use crate::state::SceneData;

pub fn build_question_prompts(
    question: &str,
    scene_context: Option<&SceneData>,
    rag_contexts: &[RagContext],
) -> (String, String) {
    let scene_summary = format_scene_summary(scene_context);
    let context_section = format_rag_contexts(rag_contexts);

    let system_prompt = format!(
        "You are a patient Blender instructor helping students learn 3D modeling through the Blender interface.

CRITICAL INSTRUCTION: You MUST teach using UI-based instructions only. NEVER provide Python code or bpy commands.

Your teaching style:
- Provide step-by-step UI instructions (menu clicks, keyboard shortcuts, tool selections)
- Explain which menus to use (Add > Mesh > ..., Modifier Properties > Add Modifier > ...)
- Describe what buttons to click and what values to adjust in the properties panels
- Use clear descriptions like \"In the 3D Viewport, press Shift+A, then select Mesh > UV Sphere\"
- Explain concepts clearly and simply, using analogies when helpful
- Break down complex tasks into numbered steps
- Encourage experimentation with different settings
- Focus on understanding WHY each step matters, not just WHAT to do

{}

The documentation below contains Python code for reference ONLY - you must translate these concepts into UI actions:
{}

Answer the student's question in a friendly, educational manner with UI-based instructions. Keep answers concise (2-4 paragraphs).

EXAMPLES OF GOOD RESPONSES:
- \"To add a sphere, press Shift+A in the 3D Viewport, then navigate to Mesh > UV Sphere\"
- \"In the Modifier Properties panel (wrench icon), click Add Modifier and select Bevel\"
- \"Select your object, press Tab to enter Edit Mode, then press Ctrl+R to add an edge loop\"

NEVER write responses like this:
- \"Use bpy.ops.mesh.primitive_uv_sphere_add(radius=1.0)\"
- \"Run this Python code: ...\"
- Any Python code snippets or bpy commands",
        scene_summary, context_section
    );

    let user_prompt = format!(
        "Question: {}

Provide a clear, educational answer that helps the student understand this Blender concept.",
        question.trim()
    );

    (system_prompt, user_prompt)
}

pub fn build_scene_analysis_prompts(scene_context: &SceneData, goal: &str) -> (String, String) {
    let scene_summary = format_detailed_scene(scene_context);
    let system_prompt = format!(
        "You are a Blender instructor analyzing a student's scene to suggest what they should learn next.

{}

Your task:
- Analyze what the student has already done
- Suggest 3-5 concrete next steps they could take to learn more
- Focus on natural progression (basics -> intermediate -> advanced)
- Each suggestion should be a learning opportunity
- Keep suggestions action-oriented and specific

Provide suggestions as a numbered list. Each suggestion should be ONE sentence that starts with an action verb.",
        scene_summary
    );

    let user_prompt = format!(
        "The student's goal is: {}

Based on their current scene, what should they try next to continue learning? Provide 3-5 specific suggestions.",
        goal.trim()
    );

    (system_prompt, user_prompt)
}

fn format_scene_summary(scene_context: Option<&SceneData>) -> String {
    match scene_context {
        Some(scene) => format!(
            "Current Scene Information:
- Objects: {} total
- Active: {}
- Mode: {}",
            scene.object_count,
            scene
                .active_object
                .as_deref()
                .unwrap_or("None"),
            scene.mode
        ),
        None => "Current Scene Information:\n- No live Blender scene data available".to_string(),
    }
}

fn format_rag_contexts(contexts: &[RagContext]) -> String {
    if contexts.is_empty() {
        return "(No specific documentation found)".to_string();
    }

    contexts
        .iter()
        .map(|ctx| format!("### {}\n{}\nSource: {}", ctx.signature, ctx.text, ctx.url))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn format_detailed_scene(scene_context: &SceneData) -> String {
    let object_lines = if scene_context.objects.is_empty() {
        "  (empty scene)".to_string()
    } else {
        scene_context
            .objects
            .iter()
            .map(|obj| {
                let modifier_suffix = if obj.modifiers.is_empty() {
                    String::new()
                } else {
                    format!(" with {} modifiers", obj.modifiers.len())
                };
                format!("  - {} ({}){}", obj.name, obj.object_type, modifier_suffix)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    format!(
        "Current Scene:
- Total objects: {}
- Active object: {}
- Mode: {}
- Render engine: {}

Objects:
{}",
        scene_context.object_count,
        scene_context
            .active_object
            .as_deref()
            .unwrap_or("None"),
        scene_context.mode,
        scene_context
            .render_engine
            .as_deref()
            .unwrap_or("Unknown"),
        object_lines
    )
}
