use std::fs;
use std::path::PathBuf;
use chrono::Utc;

/// Creates the logs directory and returns the path to today's log file
pub fn setup_log_file(app_data_dir: &PathBuf) -> Result<PathBuf, String> {
    let logs_dir = app_data_dir.join("logs");

    // Create logs directory if it doesn't exist
    fs::create_dir_all(&logs_dir)
        .map_err(|e| format!("Failed to create logs directory: {}", e))?;

    // Generate log filename with today's date
    let log_filename = format!("server_{}.log", Utc::now().format("%Y-%m-%d"));
    let log_file = logs_dir.join(log_filename);

    Ok(log_file)
}

/// Opens the logs directory in the system file explorer
#[cfg(windows)]
pub fn open_logs_directory(app_data_dir: &PathBuf) -> Result<(), String> {
    use std::process::Command;

    let logs_dir = app_data_dir.join("logs");

    Command::new("explorer")
        .arg(logs_dir)
        .spawn()
        .map_err(|e| format!("Failed to open logs directory: {}", e))?;

    Ok(())
}

#[cfg(target_os = "macos")]
pub fn open_logs_directory(app_data_dir: &PathBuf) -> Result<(), String> {
    use std::process::Command;

    let logs_dir = app_data_dir.join("logs");

    Command::new("open")
        .arg(logs_dir)
        .spawn()
        .map_err(|e| format!("Failed to open logs directory: {}", e))?;

    Ok(())
}

#[cfg(target_os = "linux")]
pub fn open_logs_directory(app_data_dir: &PathBuf) -> Result<(), String> {
    use std::process::Command;

    let logs_dir = app_data_dir.join("logs");

    Command::new("xdg-open")
        .arg(logs_dir)
        .spawn()
        .map_err(|e| format!("Failed to open logs directory: {}", e))?;

    Ok(())
}
