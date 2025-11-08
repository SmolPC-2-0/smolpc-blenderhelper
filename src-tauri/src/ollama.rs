use serde_json::json;
use tokio::time::Duration;

pub async fn ask(prompt: &str) -> Result<String, reqwest::Error> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(300))
        .build()?;

    // Request a non-streaming response so we can parse a single JSON
    // object with the answer instead of an NDJSON stream.
    let body = json!({
        "model": "llama3.2",
        "stream": false,
        "messages": [{ "role": "user", "content": prompt }]
    });

    let resp = client
        .post("http://127.0.0.1:11434/api/chat")
        .json(&body)
        .send()
        .await?
        .error_for_status()?;

    let j: serde_json::Value = resp.json().await?;
    Ok(j["message"]["content"].as_str().unwrap_or("").to_string())
}
