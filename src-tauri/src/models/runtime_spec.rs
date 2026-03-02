use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct ModelRuntimeSpec {
    pub id: String,
    pub display_name: String,
    pub model_path: PathBuf,
    pub tokenizer_path: PathBuf,
}

impl ModelRuntimeSpec {
    pub fn new(id: impl Into<String>, display_name: impl Into<String>, base_dir: &Path) -> Self {
        Self {
            id: id.into(),
            display_name: display_name.into(),
            model_path: base_dir.join("model.onnx"),
            tokenizer_path: base_dir.join("tokenizer.json"),
        }
    }

    pub fn is_available(&self) -> bool {
        self.model_path.exists() && self.tokenizer_path.exists()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelDescriptor {
    pub id: String,
    pub display_name: String,
    pub available: bool,
    pub loaded: bool,
    pub model_path: String,
    pub tokenizer_path: String,
}

