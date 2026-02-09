use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

pub struct ServerManager {
    process: Option<Child>,
    port: u16,
    log_file: PathBuf,
}

impl ServerManager {
    pub fn new(log_file: PathBuf) -> Self {
        ServerManager {
            process: None,
            port: 5000,
            log_file,
        }
    }

    /// Starts the Python RAG server
    pub fn start(
        &mut self,
        python_path: &str,
        rag_dir: &Path,
    ) -> Result<(), String> {
        println!("Starting RAG server...");
        println!("Python: {}", python_path);
        println!("RAG directory: {:?}", rag_dir);
        println!("Log file: {:?}", self.log_file);

        // Spawn the server process
        let child = spawn_server(python_path, rag_dir, &self.log_file)?;
        self.process = Some(child);

        println!("Server process spawned, waiting for health check...");
        println!("(This may take up to 60 seconds on first run while loading AI models...)");

        // Wait for server to be ready (max 60 seconds - sentence-transformers is slow to load)
        for i in 1..=60 {
            if self.health_check() {
                println!("Server is ready!");
                return Ok(());
            }

            if i % 10 == 0 {
                println!("Still waiting for server... ({}/60 seconds)", i);
            }

            thread::sleep(Duration::from_secs(1));
        }

        // Check if process is still running
        if let Some(ref mut child) = self.process {
            if let Ok(Some(status)) = child.try_wait() {
                return Err(format!(
                    "Server process exited with status: {}. Check logs at {:?}",
                    status, self.log_file
                ));
            }
        }

        Err(format!(
            "Server failed to start within 60 seconds. Check logs at {:?}",
            self.log_file
        ))
    }

    /// Performs a health check on the server
    fn health_check(&self) -> bool {
        let url = format!("http://127.0.0.1:{}/health", self.port);

        match reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
        {
            Ok(client) => match client.get(&url).send() {
                Ok(response) => response.status().is_success(),
                Err(_) => false,
            },
            Err(_) => false,
        }
    }

    /// Gracefully stops the server
    pub fn stop(&mut self) {
        if let Some(mut child) = self.process.take() {
            println!("Stopping RAG server...");

            // Try graceful termination first
            #[cfg(windows)]
            {
                // On Windows, we can't send SIGTERM easily, so just kill it
                let _ = child.kill();
            }

            #[cfg(not(windows))]
            {
                use std::os::unix::process::CommandExt;
                // Send SIGTERM on Unix
                let _ = Command::new("kill")
                    .arg(child.id().to_string())
                    .spawn();
            }

            // Wait up to 5 seconds for graceful exit
            for _ in 0..5 {
                if let Ok(Some(_)) = child.try_wait() {
                    println!("Server stopped gracefully");
                    return;
                }
                thread::sleep(Duration::from_secs(1));
            }

            // Force kill if still running
            let _ = child.kill();
            let _ = child.wait();
            println!("Server force-stopped");
        }
    }

    /// Returns the port the server is running on
    pub fn port(&self) -> u16 {
        self.port
    }
}

impl Drop for ServerManager {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Spawns the Python server process with console window hidden on Windows
#[cfg(windows)]
fn spawn_server(python_path: &str, rag_dir: &Path, log_file: &Path) -> Result<Child, String> {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let log = File::create(log_file).map_err(|e| format!("Failed to create log file: {}", e))?;

    let mut cmd = if python_path.starts_with("py ") {
        // Handle "py -3" command
        let mut c = Command::new("py");
        c.arg("-3");
        c
    } else {
        Command::new(python_path)
    };

    cmd.arg("server.py")
        .current_dir(rag_dir)
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .creation_flags(CREATE_NO_WINDOW) // This prevents console window
        .spawn()
        .map_err(|e| format!("Failed to spawn server: {}", e))
}

/// Spawns the Python server process on non-Windows platforms
#[cfg(not(windows))]
fn spawn_server(python_path: &str, rag_dir: &Path, log_file: &Path) -> Result<Child, String> {
    let log = File::create(log_file).map_err(|e| format!("Failed to create log file: {}", e))?;

    let mut cmd = if python_path.starts_with("py ") {
        // Handle "py -3" command (unlikely on Unix, but just in case)
        let mut c = Command::new("py");
        c.arg("-3");
        c
    } else {
        Command::new(python_path)
    };

    cmd.arg("server.py")
        .current_dir(rag_dir)
        .stdout(Stdio::from(log.try_clone().unwrap()))
        .stderr(Stdio::from(log))
        .spawn()
        .map_err(|e| format!("Failed to spawn server: {}", e))
}
