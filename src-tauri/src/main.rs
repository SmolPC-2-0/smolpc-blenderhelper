// Disable console window on Windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod inference;
mod logger;
mod models;
mod ollama;
mod prompts;
mod rag;
mod scene_bridge;
mod state;

use std::path::PathBuf;
use std::sync::Mutex;
use state::GenerationBackend;
use tauri::Manager;

struct BridgeRuntimeState {
    bridge: Mutex<Option<scene_bridge::SceneBridgeHandle>>,
}

#[tauri::command]
fn open_logs(app_handle: tauri::AppHandle) -> Result<(), String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?;

    logger::open_logs_directory(&app_data_dir)
}

fn main() {
    let _ = env_logger::Builder::from_default_env().try_init();
    let generation_state = commands::generation::GenerationState::default();
    let setup_generation_state = generation_state.clone();

    let result = tauri::Builder::default()
        .manage(generation_state.clone())
        .setup(move |app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            println!("App data directory: {:?}", app_data_dir);

            let log_file = logger::setup_log_file(&app_data_dir).expect("Failed to setup log file");
            println!("Server logs will be written to: {:?}", log_file);
            let _ = logger::append_log_line(&log_file, "Blender Helper backend starting");

            let rag_dir = get_rag_directory(app);
            println!("RAG data directory: {:?}", rag_dir);

            let rag_index = rag::index::RagIndex::load_from_dir(&rag_dir);
            if rag_index.is_loaded() {
                println!(
                    "[RAG] OK: Loaded {} documentation chunks",
                    rag_index.document_count()
                );
            } else {
                println!(
                    "[RAG] Warning: Retrieval disabled ({})",
                    rag_index.load_error().unwrap_or("unknown error")
                );
            }

            let backend_state = state::BackendState::new(rag_index);

            let models_dir = get_models_directory(app);
            println!("ONNX models directory: {:?}", models_dir);
            backend_state.onnx_runtime.discover_from_dir(&models_dir);

            if backend_state.get_generation_backend() == GenerationBackend::Onnx {
                match backend_state.onnx_runtime.load_default_model() {
                    Ok(model) => {
                        if backend_state.onnx_runtime.is_ready() {
                            println!(
                                "[ONNX] OK: Loaded model '{}' ({})",
                                model.id, model.model_path
                            );
                        } else {
                            println!(
                                "[ONNX] Warning: Model '{}' loaded without executable ONNX session",
                                model.id
                            );
                            println!("[ONNX] Warning: Falling back to Ollama backend");
                            backend_state.set_generation_backend(GenerationBackend::Ollama);
                        }
                    }
                    Err(err) => {
                        println!("[ONNX] Warning: Failed to load default model - {}", err);
                        println!("[ONNX] Warning: Falling back to Ollama backend");
                        backend_state.set_generation_backend(GenerationBackend::Ollama);
                    }
                }
            }

            app.manage(backend_state.clone());

            let bridge = tauri::async_runtime::block_on(scene_bridge::start_scene_bridge(
                backend_state.clone(),
                setup_generation_state.clone(),
            ))
            .map_err(|e| format!("Failed to start scene bridge: {}", e))?;

            app.manage(BridgeRuntimeState {
                bridge: Mutex::new(Some(bridge)),
            });

            println!("[SceneBridge] OK: Listening on http://127.0.0.1:5179");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_logs,
            commands::inference::list_models,
            commands::inference::load_model,
            commands::inference::unload_model,
            commands::inference::set_generation_backend,
            commands::inference::get_generation_backend,
            commands::inference::inference_generate,
            commands::inference::inference_cancel,
            commands::generation::assistant_stream_ask,
            commands::generation::is_generating,
            commands::assistant::assistant_ask,
            commands::assistant::assistant_analyze_scene,
            commands::assistant::retrieve_rag_context,
            commands::assistant::assistant_status,
            commands::scene::scene_current,
            commands::scene::scene_update,
        ])
        .build(tauri::generate_context!());

    match result {
        Ok(app) => {
            app.run(|app_handle, event| {
                if let tauri::RunEvent::ExitRequested { .. } = event {
                    if let Some(state) = app_handle.try_state::<BridgeRuntimeState>() {
                        if let Ok(mut bridge_guard) = state.bridge.lock() {
                            if let Some(bridge) = bridge_guard.as_mut() {
                                bridge.stop();
                            }
                        }
                    }
                }
            });
        }
        Err(e) => {
            eprintln!("[Tauri] Error: Failed to build Tauri app - {}", e);
            std::process::exit(1);
        }
    }
}

/// Gets the RAG system directory
/// In development: uses project root rag_system/
/// In production: uses bundled resources
fn get_rag_directory(app: &tauri::App) -> PathBuf {
    // Check bundled resource paths (tauri.conf.json specifies "resources/rag_system/...")
    if let Ok(resource_dir) = app.path().resource_dir() {
        // Tauri 2 bundles "resources/rag_system/..." relative to the resource dir
        let bundled = resource_dir.join("resources").join("rag_system");
        if bundled.join("simple_db").join("metadata.json").exists() {
            return bundled;
        }
    }

    if let Ok(resource_path) = app
        .path()
        .resolve("resources/rag_system", tauri::path::BaseDirectory::Resource)
    {
        if resource_path.exists() {
            return resource_path;
        }
    }

    if let Ok(resource_path) = app
        .path()
        .resolve("rag_system", tauri::path::BaseDirectory::Resource)
    {
        if resource_path.exists() {
            return resource_path;
        }
    }

    if let Ok(exe_dir) = std::env::current_exe() {
        if let Some(parent) = exe_dir.parent() {
            // Check resources/ subfolder next to executable
            let exe_resources_rag = parent.join("resources").join("rag_system");
            if exe_resources_rag.exists() {
                return exe_resources_rag;
            }

            let exe_rag = parent.join("rag_system");
            if exe_rag.exists() {
                return exe_rag;
            }

            let up_rag = parent.join("_up_").join("rag_system");
            if up_rag.exists() {
                return up_rag;
            }
        }
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let dev_path = cwd.join("rag_system");
    if dev_path.exists() {
        return dev_path;
    }

    let parent_path = cwd.parent().map(|p| p.join("rag_system"));
    if let Some(ref parent_path) = parent_path {
        if parent_path.exists() {
            return parent_path.clone();
        }
    }

    dev_path
}

fn get_models_directory(app: &tauri::App) -> PathBuf {
    if let Ok(resource_path) = app
        .path()
        .resolve("models", tauri::path::BaseDirectory::Resource)
    {
        if resource_path.exists() {
            return resource_path;
        }
    }

    if let Ok(exe_dir) = std::env::current_exe() {
        if let Some(parent) = exe_dir.parent() {
            let exe_models = parent.join("models");
            if exe_models.exists() {
                return exe_models;
            }

            let up_models = parent.join("_up_").join("models");
            if up_models.exists() {
                return up_models;
            }
        }
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let direct = cwd.join("models");
    if direct.exists() {
        return direct;
    }

    let parent = cwd.parent().map(|p| p.join("models"));
    if let Some(parent) = parent {
        if parent.exists() {
            return parent;
        }
    }

    direct
}
