use std::process::Command;
use std::path::PathBuf;

/// Checks if Python is available on the system and returns the path to the executable
/// Tries in order: py (Windows launcher), python3, python
pub fn check_python_available() -> Result<String, String> {
    // Try Python launcher on Windows first (py -3)
    #[cfg(windows)]
    {
        if let Ok(output) = Command::new("py")
            .arg("-3")
            .arg("--version")
            .output()
        {
            if output.status.success() {
                return Ok("py -3".to_string());
            }
        }
    }

    // Try python3
    if let Ok(output) = Command::new("python3")
        .arg("--version")
        .output()
    {
        if output.status.success() && check_python_version(&output.stdout) {
            return Ok("python3".to_string());
        }
    }

    // Try python
    if let Ok(output) = Command::new("python")
        .arg("--version")
        .output()
    {
        if output.status.success() && check_python_version(&output.stdout) {
            return Ok("python".to_string());
        }
    }

    Err("Python 3.10+ not found. Please install Python from https://www.python.org/downloads/".to_string())
}

/// Checks if Python version is 3.10 or higher
fn check_python_version(version_output: &[u8]) -> bool {
    let version_str = String::from_utf8_lossy(version_output);

    // Parse version from output like "Python 3.11.5"
    if let Some(version_part) = version_str.split_whitespace().nth(1) {
        if let Some((major, rest)) = version_part.split_once('.') {
            if let Ok(major_num) = major.parse::<u32>() {
                if major_num >= 3 {
                    if let Some((minor, _)) = rest.split_once('.') {
                        if let Ok(minor_num) = minor.parse::<u32>() {
                            return major_num > 3 || (major_num == 3 && minor_num >= 10);
                        }
                    }
                }
            }
        }
    }

    false
}

/// Verifies that all required Python dependencies are installed
pub fn verify_dependencies(python_path: &str) -> Result<(), Vec<String>> {
    println!("Verifying Python dependencies (this may take 10-20 seconds)...");

    // Check all packages in a single Python command for speed
    let packages_check = r#"
import sys
try:
    import flask
    import flask_cors
    import numpy
    import sentence_transformers
    import requests
    import bs4
    print("OK")
except ImportError as e:
    print(f"MISSING:{e.name}", file=sys.stderr)
    sys.exit(1)
"#;

    let result = if python_path.starts_with("py ") {
        Command::new("py")
            .arg("-3")
            .arg("-c")
            .arg(packages_check)
            .output()
    } else {
        Command::new(python_path)
            .arg("-c")
            .arg(packages_check)
            .output()
    };

    match result {
        Ok(output) => {
            if output.status.success() {
                println!("All dependencies found!");
                Ok(())
            } else {
                // Parse which package is missing from stderr
                let stderr = String::from_utf8_lossy(&output.stderr);
                if let Some(missing_line) = stderr.lines().find(|l| l.starts_with("MISSING:")) {
                    let package = missing_line.replace("MISSING:", "");
                    Err(vec![package])
                } else {
                    Err(vec!["unknown".to_string()])
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to run dependency check: {}", e);
            Err(vec!["check_failed".to_string()])
        }
    }
}

/// Installs Python dependencies from requirements file
pub fn install_dependencies(
    python_path: &str,
    requirements_path: &PathBuf,
) -> Result<String, String> {
    println!("Installing dependencies from {:?}...", requirements_path);

    let result = if python_path.starts_with("py ") {
        // Handle "py -3" command
        Command::new("py")
            .arg("-3")
            .arg("-m")
            .arg("pip")
            .arg("install")
            .arg("--user")
            .arg("-r")
            .arg(requirements_path)
            .output()
    } else {
        Command::new(python_path)
            .arg("-m")
            .arg("pip")
            .arg("install")
            .arg("--user")
            .arg("-r")
            .arg(requirements_path)
            .output()
    };

    match result {
        Ok(output) => {
            if output.status.success() {
                Ok(String::from_utf8_lossy(&output.stdout).to_string())
            } else {
                Err(format!(
                    "Failed to install dependencies: {}",
                    String::from_utf8_lossy(&output.stderr)
                ))
            }
        }
        Err(e) => Err(format!("Failed to run pip: {}", e)),
    }
}

/// Checks if dependencies are cached as installed
pub fn are_dependencies_cached(app_data_dir: &PathBuf) -> bool {
    let cache_file = app_data_dir.join("deps_installed.json");
    cache_file.exists()
}

/// Marks dependencies as installed in cache
pub fn mark_dependencies_installed(app_data_dir: &PathBuf) -> Result<(), String> {
    use std::fs;

    fs::create_dir_all(app_data_dir)
        .map_err(|e| format!("Failed to create app data directory: {}", e))?;

    let cache_file = app_data_dir.join("deps_installed.json");
    let cache_data = serde_json::json!({
        "installed": true,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    fs::write(cache_file, cache_data.to_string())
        .map_err(|e| format!("Failed to write cache file: {}", e))
}
