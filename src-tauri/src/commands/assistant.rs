use crate::ollama;
use crate::prompts::{build_question_prompts, build_scene_analysis_prompts};
use crate::rag::types::RagContext;
use crate::state::{BackendState, GenerationBackend, SceneData};
use serde::{Deserialize, Serialize};
use tauri::State;

const MAX_QUESTION_LEN: usize = 10_000;
const MAX_GOAL_LEN: usize = 500;
const DEFAULT_N_RESULTS: usize = 3;

#[derive(Debug, Clone, Deserialize)]
pub struct AskRequest {
    pub question: String,
    pub scene_context: Option<SceneData>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AskResponse {
    pub answer: String,
    pub contexts_used: usize,
    pub rag_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SceneAnalysisRequest {
    pub goal: Option<String>,
    #[serde(default, alias = "scene_data")]
    pub scene_context: Option<SceneData>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SceneAnalysisResponse {
    pub suggestions: Vec<String>,
    pub analysis: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RagRetrieveResponse {
    pub contexts: Vec<RagContext>,
    pub rag_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AssistantStatusResponse {
    pub status: String,
    pub connected: bool,
    pub backend: String,
    pub model: String,
    pub generating: bool,
    pub rag_enabled: bool,
    pub rag_docs: usize,
    pub rag_error: Option<String>,
    pub available_models: usize,
}

pub fn resolve_scene_context(state: &BackendState, scene_context: Option<SceneData>) -> Option<SceneData> {
    if scene_context.is_some() {
        return scene_context;
    }

    match state.scene_cache.lock() {
        Ok(cache) => cache.latest_scene(),
        Err(poisoned) => poisoned.into_inner().latest_scene(),
    }
}

pub fn retrieve_contexts(
    state: &BackendState,
    query: &str,
    n_results: usize,
) -> Result<RagRetrieveResponse, String> {
    let guard = match state.rag_index.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };

    if !guard.is_loaded() {
        return Ok(RagRetrieveResponse {
            contexts: Vec::new(),
            rag_enabled: false,
        });
    }

    let contexts = guard.retrieve_context(query, n_results)?;
    Ok(RagRetrieveResponse {
        contexts,
        rag_enabled: true,
    })
}

pub async fn ask_internal(state: &BackendState, request: AskRequest) -> Result<AskResponse, String> {
    let question = request.question.trim();
    if question.is_empty() {
        return Err("No question provided".to_string());
    }
    if question.len() > MAX_QUESTION_LEN {
        return Err("Question too long (max 10,000 characters)".to_string());
    }

    let scene_context = resolve_scene_context(state, request.scene_context);
    let rag = retrieve_contexts(state, question, DEFAULT_N_RESULTS)?;
    let (system_prompt, user_prompt) =
        build_question_prompts(question, scene_context.as_ref(), &rag.contexts);

    let answer = match state.get_generation_backend() {
        GenerationBackend::Onnx => state.onnx_runtime.generate_once(&system_prompt, &user_prompt)?,
        GenerationBackend::Ollama => {
            ollama::chat_once(&system_prompt, &user_prompt, request.model, 0.7).await?
        }
    };

    Ok(AskResponse {
        answer,
        contexts_used: rag.contexts.len(),
        rag_enabled: rag.rag_enabled,
    })
}

pub async fn analyze_scene_internal(
    state: &BackendState,
    request: SceneAnalysisRequest,
) -> Result<SceneAnalysisResponse, String> {
    let goal = request
        .goal
        .unwrap_or_else(|| "learning blender".to_string())
        .trim()
        .to_string();
    if goal.len() > MAX_GOAL_LEN {
        return Err("Goal too long (max 500 characters)".to_string());
    }

    let scene_context = resolve_scene_context(state, request.scene_context)
        .ok_or_else(|| "No scene context available".to_string())?;

    let (system_prompt, user_prompt) = build_scene_analysis_prompts(&scene_context, &goal);
    let response = match state.get_generation_backend() {
        GenerationBackend::Onnx => state.onnx_runtime.generate_once(&system_prompt, &user_prompt)?,
        GenerationBackend::Ollama => {
            ollama::chat_once(&system_prompt, &user_prompt, request.model, 0.7).await?
        }
    };
    let suggestions = parse_suggestions(&response);

    Ok(SceneAnalysisResponse {
        suggestions,
        analysis: response,
    })
}

pub async fn assistant_status_internal(
    state: &BackendState,
    generating: bool,
) -> AssistantStatusResponse {
    let (rag_enabled, rag_docs, rag_error) = match state.rag_index.lock() {
        Ok(index) => (
            index.is_loaded(),
            index.document_count(),
            index.load_error().map(|s| s.to_string()),
        ),
        Err(poisoned) => {
            let index = poisoned.into_inner();
            (
                index.is_loaded(),
                index.document_count(),
                index.load_error().map(|s| s.to_string()),
            )
        }
    };

    let backend = state.get_generation_backend();
    let available_models = state.onnx_runtime.list_models().len();
    let (connected, model) = match backend {
        GenerationBackend::Onnx => (
            state.onnx_runtime.is_ready(),
            state
                .onnx_runtime
                .loaded_model_id()
                .unwrap_or_else(|| "not_loaded".to_string()),
        ),
        GenerationBackend::Ollama => (
            ollama::is_ollama_available().await,
            std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| ollama::DEFAULT_MODEL.to_string()),
        ),
    };

    AssistantStatusResponse {
        status: if connected { "ok" } else { "error" }.to_string(),
        connected,
        backend: backend.as_str().to_string(),
        model,
        generating,
        rag_enabled,
        rag_docs,
        rag_error,
        available_models,
    }
}

#[tauri::command]
pub async fn assistant_ask(
    request: AskRequest,
    state: State<'_, BackendState>,
) -> Result<AskResponse, String> {
    ask_internal(&state, request).await
}

#[tauri::command]
pub async fn assistant_analyze_scene(
    request: SceneAnalysisRequest,
    state: State<'_, BackendState>,
) -> Result<SceneAnalysisResponse, String> {
    analyze_scene_internal(&state, request).await
}

#[tauri::command]
pub async fn retrieve_rag_context(
    query: String,
    n_results: Option<usize>,
    state: State<'_, BackendState>,
) -> Result<RagRetrieveResponse, String> {
    retrieve_contexts(&state, &query, n_results.unwrap_or(DEFAULT_N_RESULTS))
}

#[tauri::command]
pub async fn assistant_status(
    state: State<'_, BackendState>,
    generation_state: State<'_, super::generation::GenerationState>,
) -> Result<AssistantStatusResponse, String> {
    Ok(assistant_status_internal(&state, generation_state.is_generating()).await)
}

fn parse_suggestions(response: &str) -> Vec<String> {
    let mut suggestions: Vec<String> = response
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(strip_leading_number)
        .collect();

    if suggestions.is_empty() && !response.trim().is_empty() {
        suggestions.push(response.trim().to_string());
    }

    suggestions.truncate(5);
    suggestions
}

fn strip_leading_number(line: &str) -> Option<String> {
    let mut chars = line.chars().peekable();
    let mut consumed_digits = false;

    while let Some(ch) = chars.peek() {
        if ch.is_ascii_digit() {
            consumed_digits = true;
            chars.next();
        } else {
            break;
        }
    }

    if consumed_digits {
        while let Some(ch) = chars.peek() {
            if *ch == '.' || *ch == ')' || *ch == ':' || *ch == '-' || ch.is_whitespace() {
                chars.next();
            } else {
                break;
            }
        }
    }

    if !consumed_digits {
        return None;
    }

    let cleaned: String = chars.collect::<String>().trim().to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}
