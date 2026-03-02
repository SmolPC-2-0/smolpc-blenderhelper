use crate::ollama::{self, OllamaMetrics};
use crate::prompts::build_question_prompts;
use crate::state::{BackendState, GenerationBackend, SceneData};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tauri::ipc::Channel;
use tauri::State;

const ERR_GENERATION_IN_PROGRESS: &str = "Generation already in progress";

#[derive(Clone)]
pub struct GenerationState {
    active_cancel: Arc<StdMutex<Option<Arc<AtomicBool>>>>,
    generating: Arc<AtomicBool>,
}

impl Default for GenerationState {
    fn default() -> Self {
        Self {
            active_cancel: Arc::new(StdMutex::new(None)),
            generating: Arc::new(AtomicBool::new(false)),
        }
    }
}

struct GenerationPermit {
    generating: Arc<AtomicBool>,
    active_cancel: Arc<StdMutex<Option<Arc<AtomicBool>>>>,
}

impl Drop for GenerationPermit {
    fn drop(&mut self) {
        self.generating.store(false, Ordering::SeqCst);
        match self.active_cancel.lock() {
            Ok(mut guard) => *guard = None,
            Err(poisoned) => *poisoned.into_inner() = None,
        }
    }
}

impl GenerationState {
    fn try_begin(&self) -> Result<(GenerationPermit, Arc<AtomicBool>), String> {
        if self
            .generating
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(ERR_GENERATION_IN_PROGRESS.to_string());
        }

        let cancel_token = Arc::new(AtomicBool::new(false));
        match self.active_cancel.lock() {
            Ok(mut guard) => *guard = Some(Arc::clone(&cancel_token)),
            Err(poisoned) => *poisoned.into_inner() = Some(Arc::clone(&cancel_token)),
        }

        Ok((
            GenerationPermit {
                generating: Arc::clone(&self.generating),
                active_cancel: Arc::clone(&self.active_cancel),
            },
            cancel_token,
        ))
    }

    pub fn is_generating(&self) -> bool {
        self.generating.load(Ordering::SeqCst)
    }
}

#[tauri::command]
pub async fn assistant_stream_ask(
    question: String,
    scene_context: Option<SceneData>,
    on_token: Channel<String>,
    backend_state: State<'_, BackendState>,
    generation_state: State<'_, GenerationState>,
) -> Result<OllamaMetrics, String> {
    let question = question.trim();
    if question.is_empty() {
        return Err("No question provided".to_string());
    }
    if question.len() > 10_000 {
        return Err("Question too long (max 10,000 characters)".to_string());
    }

    let effective_scene_context = super::assistant::resolve_scene_context(&backend_state, scene_context);
    let rag = super::assistant::retrieve_contexts(&backend_state, question, 3)?;
    let (system_prompt, user_prompt) =
        build_question_prompts(question, effective_scene_context.as_ref(), &rag.contexts);

    execute_stream_generation(
        system_prompt,
        user_prompt,
        on_token,
        &backend_state,
        &generation_state,
    )
    .await
}

pub async fn execute_stream_generation(
    system_prompt: String,
    user_prompt: String,
    on_token: Channel<String>,
    backend_state: &BackendState,
    state: &GenerationState,
) -> Result<OllamaMetrics, String> {
    let (_permit, cancelled) = state.try_begin()?;

    let result = match backend_state.get_generation_backend() {
        GenerationBackend::Onnx => {
            let token_channel = on_token.clone();
            backend_state
                .onnx_runtime
                .stream_generate(
                    &system_prompt,
                    &user_prompt,
                    Arc::clone(&cancelled),
                    move |token| {
                        if let Err(e) = token_channel.send(token) {
                            log::warn!("Failed to send token via channel: {}", e);
                        }
                    },
                )
                .await
        }
        GenerationBackend::Ollama => {
            let token_channel = on_token.clone();
            ollama::stream_chat(
                &system_prompt,
                &user_prompt,
                Arc::clone(&cancelled),
                move |token| {
                    if let Err(e) = token_channel.send(token) {
                        log::warn!("Failed to send token via channel: {}", e);
                    }
                },
            )
            .await
        }
    };

    match result {
        Ok(metrics) => {
            if cancelled.load(Ordering::SeqCst) {
                Err("GENERATION_CANCELLED: Generation cancelled by user".to_string())
            } else {
                Ok(metrics)
            }
        }
        Err(e) => Err(e),
    }
}

pub fn cancel_internal(state: &GenerationState) -> Result<(), String> {
    let active_cancel = match state.active_cancel.lock() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };

    if let Some(cancel_token) = active_cancel {
        cancel_token.store(true, Ordering::SeqCst);
    }

    Ok(())
}

#[tauri::command]
pub async fn is_generating(state: State<'_, GenerationState>) -> Result<bool, String> {
    Ok(state.is_generating())
}
