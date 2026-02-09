// Disable console window on Windows release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod python_checker;
mod server_manager;
mod logger;

use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

struct AppState {
    server: Mutex<server_manager::ServerManager>,
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
    // Initialize Tauri app first to get paths
    let result = tauri::Builder::default()
        .setup(|app| {
            // Get app data directory
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            println!("App data directory: {:?}", app_data_dir);

            // Setup log file
            let log_file = logger::setup_log_file(&app_data_dir)
                .expect("Failed to setup log file");

            println!("Server logs will be written to: {:?}", log_file);

            // Check if Python is available
            let python_path = match python_checker::check_python_available() {
                Ok(path) => {
                    println!("Found Python: {}", path);
                    path
                }
                Err(e) => {
                    eprintln!("{}", e);
                    show_error_dialog(
                        "Python Not Found",
                        &format!("{}\n\nWould you like to download Python?", e),
                        true,
                    );
                    std::process::exit(1);
                }
            };

            // Get the RAG system directory
            // In development, it's at the project root
            // In production, it's bundled in resources
            let rag_dir = get_rag_directory(app);

            println!("RAG system directory: {:?}", rag_dir);

            // Check if dependencies are cached
            let deps_cached = python_checker::are_dependencies_cached(&app_data_dir);

            if !deps_cached {
                println!("Dependencies not cached, checking installation...");

                // Verify dependencies
                match python_checker::verify_dependencies(&python_path) {
                    Ok(_) => {
                        println!("All dependencies found!");
                        // Mark as installed
                        python_checker::mark_dependencies_installed(&app_data_dir)
                            .expect("Failed to mark dependencies as installed");
                    }
                    Err(missing) => {
                        println!("Missing dependencies: {:?}", missing);
                        println!("Installing dependencies...");

                        let requirements_path = rag_dir.join("requirements_server.txt");

                        match python_checker::install_dependencies(&python_path, &requirements_path)
                        {
                            Ok(output) => {
                                println!("Dependencies installed successfully!");
                                println!("{}", output);

                                // Mark as installed
                                python_checker::mark_dependencies_installed(&app_data_dir)
                                    .expect("Failed to mark dependencies as installed");
                            }
                            Err(e) => {
                                eprintln!("Failed to install dependencies: {}", e);
                                show_error_dialog(
                                    "Dependency Installation Failed",
                                    &format!("Failed to install Python dependencies:\n\n{}", e),
                                    false,
                                );
                                std::process::exit(1);
                            }
                        }
                    }
                }
            } else {
                println!("Dependencies already installed (cached)");
            }

            // Start the server
            let mut server = server_manager::ServerManager::new(log_file);

            match server.start(&python_path, &rag_dir) {
                Ok(_) => {
                    println!("RAG server started successfully on port {}", server.port());
                }
                Err(e) => {
                    eprintln!("Failed to start server: {}", e);
                    show_error_dialog(
                        "Server Startup Failed",
                        &format!("Failed to start RAG server:\n\n{}", e),
                        false,
                    );
                    std::process::exit(1);
                }
            }

            // Store server in app state
            app.manage(AppState {
                server: Mutex::new(server),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![open_logs])
        .build(tauri::generate_context!());

    match result {
        Ok(app) => {
            app.run(|app_handle, event| {
                if let tauri::RunEvent::ExitRequested { .. } = event {
                    // Stop server when app exits
                    if let Some(state) = app_handle.try_state::<AppState>() {
                        if let Ok(mut server) = state.server.lock() {
                            server.stop();
                        }
                    }
                }
            });
        }
        Err(e) => {
            eprintln!("Failed to build Tauri app: {}", e);
            std::process::exit(1);
        }
    }
}

/// Gets the RAG system directory
/// In development: uses project root rag_system/
/// In production: uses bundled resources
fn get_rag_directory(app: &tauri::App) -> PathBuf {
    // Try to get bundled resources first (production)
    if let Ok(resource_path) = app
        .path()
        .resolve("rag_system", tauri::path::BaseDirectory::Resource)
    {
        println!("Checking resource path: {:?}", resource_path);
        if resource_path.exists() {
            println!("Found rag_system at resource path!");
            return resource_path;
        }
    }

    // For release builds, check next to the executable
    if let Ok(exe_dir) = std::env::current_exe() {
        if let Some(parent) = exe_dir.parent() {
            let exe_rag = parent.join("rag_system");
            println!("Checking next to exe: {:?}", exe_rag);
            if exe_rag.exists() {
                println!("Found rag_system next to executable!");
                return exe_rag;
            }

            // Also check _up_ directory (Tauri bundling quirk)
            let up_rag = parent.join("_up_").join("rag_system");
            println!("Checking _up_ directory: {:?}", up_rag);
            if up_rag.exists() {
                println!("Found rag_system in _up_ directory!");
                return up_rag;
            }
        }
    }

    // Fall back to development path
    let mut dev_path = std::env::current_dir().expect("Failed to get current directory");
    dev_path.push("rag_system");
    println!("Checking dev path: {:?}", dev_path);

    if dev_path.exists() {
        println!("Found rag_system at dev path!");
        return dev_path;
    }

    // Last resort: check parent directory (in case we're in src-tauri/)
    let mut parent_path = std::env::current_dir().expect("Failed to get current directory");
    parent_path.pop();
    parent_path.push("rag_system");
    println!("Checking parent path: {:?}", parent_path);

    if parent_path.exists() {
        println!("Found rag_system at parent path!");
        return parent_path;
    }

    eprintln!("\n=== ERROR ===");
    eprintln!("Could not find rag_system directory!");
    eprintln!("Tried the following locations:");
    eprintln!("- Tauri resource path");
    eprintln!("- Next to executable");
    eprintln!("- _up_/ directory");
    eprintln!("- Current directory: {:?}", std::env::current_dir());
    eprintln!("- Parent directory");
    eprintln!("=============\n");

    panic!("Could not find rag_system directory!");
}

/// Shows an error dialog to the user
fn show_error_dialog(title: &str, message: &str, has_download_option: bool) {
    // For now, just print to stderr. In Tauri 2, dialogs need an AppHandle
    // which we don't have before the app is built
    eprintln!("\n=== {} ===", title);
    eprintln!("{}", message);
    eprintln!("================\n");

    if has_download_option {
        eprintln!("Opening Python download page...");
        let _ = open::that("https://www.python.org/downloads/");
    }
}
