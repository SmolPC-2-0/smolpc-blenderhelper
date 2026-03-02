use super::runtime_spec::ModelRuntimeSpec;
use std::fs;
use std::path::Path;

fn title_case(input: &str) -> String {
    input
        .split(['-', '_', ' '])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = String::new();
                    out.extend(first.to_uppercase());
                    out.push_str(chars.as_str());
                    out
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn discover_models(models_dir: &Path) -> Vec<ModelRuntimeSpec> {
    let mut models = vec![ModelRuntimeSpec::new(
        "qwen2.5-coder-1.5b",
        "Qwen2.5 Coder 1.5B",
        &models_dir.join("qwen2.5-coder-1.5b"),
    )];

    let entries = match fs::read_dir(models_dir) {
        Ok(entries) => entries,
        Err(_) => return models,
    };

    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }

        let model_id = entry.file_name().to_string_lossy().to_string();
        if model_id.is_empty() || models.iter().any(|m| m.id == model_id) {
            continue;
        }

        models.push(ModelRuntimeSpec::new(
            model_id.clone(),
            title_case(&model_id),
            &entry.path(),
        ));
    }

    models.sort_by(|a, b| a.id.cmp(&b.id));
    models
}

