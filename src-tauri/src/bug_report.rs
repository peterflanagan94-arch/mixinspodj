//! "Report a problem": sends the DJ's message (and optional reply email) to
//! MixInspo's website, which emails it to the developer. Only runs when the
//! DJ presses Send; nothing about their library or plays is attached beyond
//! the app version and operating system.

use serde_json::json;

const BUG_URL: &str = "https://www.studioonscreen.com/api/mixinspo/bug";

#[tauri::command]
pub async fn send_bug_report(message: String, email: Option<String>) -> Result<(), String> {
    let response = reqwest::Client::new()
        .post(BUG_URL)
        .json(&json!({
            "message": message,
            "email": email.filter(|e| !e.trim().is_empty()),
            "version": env!("CARGO_PKG_VERSION"),
            "platform": std::env::consts::OS,
        }))
        .send()
        .await
        .map_err(|e| format!("Could not send the report: {e}"))?;

    if response.status().is_success() {
        return Ok(());
    }
    let body: serde_json::Value = response.json().await.unwrap_or_default();
    Err(body["error"]
        .as_str()
        .unwrap_or("Could not send the report. Please try again.")
        .to_string())
}
