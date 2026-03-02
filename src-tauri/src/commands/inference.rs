use crate::models::runtime_spec::ModelDescriptor;
use crate::state::{BackendState, GenerationBackend};
use tauri::State;

#[derive(Debug, Clone, serde::Serialize)]
pub struct BackendSelectionResponse {
    pub backend: String,
}

#[tauri::command]
pub async fn list_models(state: State<'_, BackendState>) -> Result<Vec<ModelDescriptor>, String> {
    Ok(state.onnx_runtime.list_models())
}

#[tauri::command]
pub async fn load_model(
    model_id: Option<String>,
    state: State<'_, BackendState>,
) -> Result<ModelDescriptor, String> {
    let descriptor = match model_id {
        Some(model_id) if !model_id.trim().is_empty() => state.onnx_runtime.load_model(model_id.trim())?,
        _ => state.onnx_runtime.load_default_model()?,
    };

    if !state.onnx_runtime.is_ready() {
        return Err(
            "Model metadata loaded, but ONNX inference is not available in this build. Use Ollama."
                .to_string(),
        );
    }

    state.set_generation_backend(GenerationBackend::Onnx);
    Ok(descriptor)
}

#[tauri::command]
pub async fn unload_model(state: State<'_, BackendState>) -> Result<(), String> {
    state.onnx_runtime.unload_model();
    Ok(())
}

#[tauri::command]
pub async fn set_generation_backend(
    backend: String,
    state: State<'_, BackendState>,
) -> Result<BackendSelectionResponse, String> {
    let parsed = GenerationBackend::from_str(&backend)
        .ok_or_else(|| "Invalid backend. Use 'onnx' or 'ollama'".to_string())?;

    if parsed == GenerationBackend::Onnx && !state.onnx_runtime.is_ready() {
        return Err("ONNX backend requested but no model is loaded".to_string());
    }

    state.set_generation_backend(parsed);
    Ok(BackendSelectionResponse {
        backend: parsed.as_str().to_string(),
    })
}

#[tauri::command]
pub async fn get_generation_backend(
    state: State<'_, BackendState>,
) -> Result<BackendSelectionResponse, String> {
    let backend = state.get_generation_backend();
    Ok(BackendSelectionResponse {
        backend: backend.as_str().to_string(),
    })
}

#[tauri::command]
pub async fn inference_generate(
    system_prompt: String,
    user_prompt: String,
    on_token: tauri::ipc::Channel<String>,
    state: State<'_, BackendState>,
    generation_state: State<'_, super::generation::GenerationState>,
) -> Result<crate::ollama::OllamaMetrics, String> {
    super::generation::execute_stream_generation(
        system_prompt,
        user_prompt,
        on_token,
        &state,
        &generation_state,
    )
    .await
}

#[tauri::command]
pub async fn inference_cancel(
    generation_state: State<'_, super::generation::GenerationState>,
) -> Result<(), String> {
    super::generation::cancel_internal(&generation_state)
}
