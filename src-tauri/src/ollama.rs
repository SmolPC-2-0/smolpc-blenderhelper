use serde_json::json;
use tokio::time::{Duration, sleep};

pub async fn ask(system: &str, user: &str) -> Result<String, reqwest::Error> {
    // Increase timeout to 10 minutes for large models like qwen2.5-coder:32b
    // Can be overridden via env var OLLAMA_TIMEOUT_SECS
    let timeout_secs = std::env::var("OLLAMA_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(600);

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(timeout_secs))
        .build()?;

    let model = std::env::var("OLLAMA_MODEL").unwrap_or_else(|_| "qwen2.5-coder:14b".to_string());

    // Request a non-streaming response so we can parse a single JSON
    // object with the answer instead of an NDJSON stream.
    // Temperature 0.7 balances creativity with consistency, allowing the model to reason
    // through complex 3D modeling problems while maintaining code correctness.
    let body = json!({
        "model": model,
        "stream": false,
        "options": { "temperature": 0.7 },
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user }
        ]
    });

    let resp = client
        .post("http://127.0.0.1:11434/api/chat")
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            eprintln!("Ollama request failed: {} (model: {}, timeout: {}s)", e, model, timeout_secs);
            if e.is_timeout() {
                eprintln!("TIMEOUT: Consider increasing OLLAMA_TIMEOUT_SECS env var or using a smaller model");
            } else if e.is_connect() {
                eprintln!("CONNECTION: Is Ollama running? Check: http://127.0.0.1:11434");
            }
            e
        })?
        .error_for_status()?;

    let j: serde_json::Value = resp.json().await?;
    Ok(j["message"]["content"].as_str().unwrap_or("").to_string())
}

/// Run a two‑phase request: first get hidden planning notes, wait a bit,
/// then produce the final answer using those notes. The delay defaults to
/// the provided `seconds`, but can be overridden by env `DELIBERATION_SECS`.
pub async fn ask_deliberate(system: &str, user: &str, seconds: u64) -> Result<String, reqwest::Error> {
    let planning_system = "You are an expert Blender engineer. Think privately and write short bullet notes about the best approach. Do NOT include code or backticks. Keep under 150 words.";
    let planning_user = format!("{}\n\nWrite internal notes only.", user);
    let notes = ask(planning_system, &planning_user).await.unwrap_or_default();

    let sleep_secs = std::env::var("DELIBERATION_SECS")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(seconds);
    sleep(Duration::from_secs(sleep_secs)).await;

    let final_system = format!("{}\n\nInternal planning notes (use for quality, do not echo):\n{}", system, notes);
    ask(&final_system, user).await
}
