use crate::models::loader::{load_model, LoadedOnnxModel};
use crate::models::registry::discover_models;
use crate::models::runtime_spec::{ModelDescriptor, ModelRuntimeSpec};
use crate::ollama::OllamaMetrics;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Clone)]
pub struct OnnxRuntime {
    inner: Arc<Mutex<OnnxRuntimeInner>>,
}

#[derive(Default)]
struct OnnxRuntimeInner {
    models: Vec<ModelRuntimeSpec>,
    loaded: Option<Arc<LoadedOnnxModel>>,
}

impl Default for OnnxRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl OnnxRuntime {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(OnnxRuntimeInner::default())),
        }
    }

    pub fn discover_from_dir(&self, models_dir: &Path) {
        let models = discover_models(models_dir);
        match self.inner.lock() {
            Ok(mut guard) => guard.models = models,
            Err(poisoned) => poisoned.into_inner().models = models,
        }
    }

    pub fn list_models(&self) -> Vec<ModelDescriptor> {
        let guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        guard
            .models
            .iter()
            .map(|spec| ModelDescriptor {
                id: spec.id.clone(),
                display_name: spec.display_name.clone(),
                available: spec.is_available(),
                loaded: guard.loaded.as_ref().is_some_and(|m| m.spec.id == spec.id),
                model_path: spec.model_path.display().to_string(),
                tokenizer_path: spec.tokenizer_path.display().to_string(),
            })
            .collect()
    }

    pub fn load_default_model(&self) -> Result<ModelDescriptor, String> {
        let default_id = {
            let guard = match self.inner.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard
                .models
                .first()
                .map(|m| m.id.clone())
                .ok_or_else(|| "No ONNX models are registered".to_string())?
        };
        self.load_model(&default_id)
    }

    pub fn load_model(&self, model_id: &str) -> Result<ModelDescriptor, String> {
        let mut guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };

        let spec = guard
            .models
            .iter()
            .find(|m| m.id == model_id)
            .cloned()
            .ok_or_else(|| format!("Unknown model id: {}", model_id))?;

        let loaded = load_model(&spec)?;
        guard.loaded = Some(Arc::new(loaded));

        Ok(ModelDescriptor {
            id: spec.id,
            display_name: spec.display_name,
            available: true,
            loaded: true,
            model_path: spec.model_path.display().to_string(),
            tokenizer_path: spec.tokenizer_path.display().to_string(),
        })
    }

    pub fn unload_model(&self) {
        match self.inner.lock() {
            Ok(mut guard) => guard.loaded = None,
            Err(poisoned) => poisoned.into_inner().loaded = None,
        }
    }

    pub fn loaded_model_id(&self) -> Option<String> {
        let guard = match self.inner.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        guard.loaded.as_ref().map(|m| m.spec.id.clone())
    }

    pub fn is_ready(&self) -> bool {
        match self.inner.lock() {
            Ok(guard) => guard
                .loaded
                .as_ref()
                .is_some_and(|model| model.inference_ready),
            Err(poisoned) => poisoned
                .into_inner()
                .loaded
                .as_ref()
                .is_some_and(|model| model.inference_ready),
        }
    }

    pub async fn stream_generate<F>(
        &self,
        system_prompt: &str,
        user_prompt: &str,
        cancelled: Arc<AtomicBool>,
        mut on_token: F,
    ) -> Result<OllamaMetrics, String>
    where
        F: FnMut(String),
    {
        let (response, token_count) = self.generate_text(system_prompt, user_prompt)?;

        let started = Instant::now();
        for chunk in split_for_stream(&response) {
            if cancelled.load(Ordering::Relaxed) {
                return Err("GENERATION_CANCELLED: Generation cancelled by user".to_string());
            }
            on_token(chunk);
            tokio::task::yield_now().await;
        }

        let elapsed = started.elapsed();
        let elapsed_secs = elapsed.as_secs_f64();
        Ok(OllamaMetrics {
            total_tokens: token_count,
            total_time_ms: elapsed.as_millis() as u64,
            tokens_per_second: if elapsed_secs > 0.0 {
                token_count as f64 / elapsed_secs
            } else {
                0.0
            },
        })
    }

    pub fn generate_once(&self, system_prompt: &str, user_prompt: &str) -> Result<String, String> {
        let (response, _) = self.generate_text(system_prompt, user_prompt)?;
        Ok(response)
    }

    fn generate_text(&self, _system_prompt: &str, _user_prompt: &str) -> Result<(String, u64), String> {
        let loaded = {
            let guard = match self.inner.lock() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            guard
                .loaded
                .clone()
                .ok_or_else(|| "ONNX model is not loaded. Call load_model first.".to_string())?
        };

        if !loaded.inference_ready {
            return Err(
                "ONNX backend is not available in this build yet. Switch to the Ollama backend."
                    .to_string(),
            );
        }

        let response = format!(
            "Model '{}' is loaded, but no ONNX inference implementation is configured.",
            loaded.spec.id
        );
        let token_count = loaded
            .tokenizer
            .encode(response.as_str(), true)
            .map(|enc| enc.get_ids().len() as u64)
            .unwrap_or_else(|_| response.split_whitespace().count() as u64);

        Ok((response, token_count))
    }
}

fn split_for_stream(text: &str) -> Vec<String> {
    let mut chunks = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        current.push(ch);
        if ch.is_whitespace() || current.len() >= 18 {
            chunks.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}
