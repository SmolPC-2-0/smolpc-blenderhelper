use super::runtime_spec::ModelRuntimeSpec;
use tokenizers::Tokenizer;

#[derive(Clone)]
pub struct LoadedOnnxModel {
    pub spec: ModelRuntimeSpec,
    pub tokenizer: Tokenizer,
    pub inference_ready: bool,
}

impl std::fmt::Debug for LoadedOnnxModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LoadedOnnxModel")
            .field("spec", &self.spec)
            .field("inference_ready", &self.inference_ready)
            .finish_non_exhaustive()
    }
}

pub fn load_model(spec: &ModelRuntimeSpec) -> Result<LoadedOnnxModel, String> {
    if !spec.model_path.exists() {
        return Err(format!(
            "Model file not found for {}: {}",
            spec.id,
            spec.model_path.display()
        ));
    }
    if !spec.tokenizer_path.exists() {
        return Err(format!(
            "Tokenizer file not found for {}: {}",
            spec.id,
            spec.tokenizer_path.display()
        ));
    }

    let tokenizer = Tokenizer::from_file(&spec.tokenizer_path)
        .map_err(|e| format!("Failed to load tokenizer for {}: {}", spec.id, e))?;

    Ok(LoadedOnnxModel {
        spec: spec.clone(),
        tokenizer,
        inference_ready: false,
    })
}
