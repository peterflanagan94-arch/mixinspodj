//! MixInspo accounts and licensing: a 14-day free trial from first sign-in
//! (no card), then a monthly subscription. The website decides who may run
//! the app (`/api/mixinspo/*`); this module signs in, keeps the session
//! fresh, and remembers the last answer so a flaky connection at a gig
//! doesn't lock the DJ out.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::AppHandle;

use crate::settings;

const BASE_URL: &str = "https://www.studioonscreen.com/api/mixinspo";
/// How long a previously-allowed licence is trusted when the server can't be
/// reached. A trial is never extended by being offline: its own end date applies.
const OFFLINE_GRACE_SECS: u64 = 7 * 24 * 60 * 60;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredAccount {
    pub email: String,
    pub access_token: String,
    pub refresh_token: String,
    pub license: License,
    pub checked_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct License {
    #[serde(default)]
    pub allowed: bool,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub trial_ends_at: Option<String>,
    #[serde(default)]
    pub trial_days_left: Option<u32>,
    #[serde(default)]
    pub must_change_password: bool,
    #[serde(default)]
    pub subscribe_url: Option<String>,
    #[serde(default)]
    pub price: String,
}

/// What the screen needs to know.
#[derive(Serialize)]
pub struct AccountState {
    signed_in: bool,
    email: Option<String>,
    allowed: bool,
    reason: String,
    trial_days_left: Option<u32>,
    must_change_password: bool,
    subscribe_url: Option<String>,
    price: String,
    offline: bool,
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn signed_out() -> AccountState {
    AccountState {
        signed_in: false,
        email: None,
        allowed: false,
        reason: "signed-out".into(),
        trial_days_left: None,
        must_change_password: false,
        subscribe_url: None,
        price: "£5.99 a month".into(),
        offline: false,
    }
}

fn state_from(account: &StoredAccount, offline: bool) -> AccountState {
    let l = &account.license;
    AccountState {
        signed_in: true,
        email: Some(account.email.clone()),
        allowed: l.allowed,
        reason: l.reason.clone(),
        trial_days_left: l.trial_days_left,
        must_change_password: l.must_change_password,
        subscribe_url: l.subscribe_url.clone(),
        price: l.price.clone(),
        offline,
    }
}

async fn post(path: &str, body: Value, token: Option<&str>) -> Result<(u16, Value), String> {
    let mut request = reqwest::Client::new().post(format!("{BASE_URL}/{path}")).json(&body);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let response = request.send().await.map_err(|_| "Can't reach MixInspo right now. Check your internet connection.".to_string())?;
    let status = response.status().as_u16();
    Ok((status, response.json().await.unwrap_or_default()))
}

fn error_message(body: &Value) -> String {
    body["error"].as_str().unwrap_or("Something went wrong. Please try again.").to_string()
}

/// Turns a successful `{ session, license }` response into a stored account.
fn account_from(email: &str, body: &Value) -> Result<StoredAccount, String> {
    let session = &body["session"];
    let license: License = serde_json::from_value(body["license"].clone()).map_err(|_| "Unexpected reply from the server.".to_string())?;
    Ok(StoredAccount {
        email: email.to_string(),
        access_token: session["access_token"].as_str().ok_or("Unexpected reply from the server.")?.to_string(),
        refresh_token: session["refresh_token"].as_str().ok_or("Unexpected reply from the server.")?.to_string(),
        license,
        checked_at: now(),
    })
}

fn store(app: &AppHandle, account: Option<StoredAccount>) -> Result<(), String> {
    let mut current = settings::load(app);
    current.account = account;
    settings::save(app, &current)
}

#[tauri::command]
pub async fn account_signup(email: String, name: Option<String>) -> Result<(), String> {
    let (status, body) = post("signup", json!({ "email": email, "name": name }), None).await?;
    if status == 200 { Ok(()) } else { Err(error_message(&body)) }
}

/// Emails a new temporary password to the address if it has an account. The
/// reply is the same either way, so it can't be used to find out who has one.
#[tauri::command]
pub async fn account_forgot_password(email: String) -> Result<(), String> {
    let response = reqwest::Client::new()
        .post("https://www.studioonscreen.com/api/forgot-password")
        .json(&json!({ "email": email }))
        .send()
        .await
        .map_err(|_| "Can't reach MixInspo right now. Check your internet connection.".to_string())?;
    if response.status().is_success() {
        return Ok(());
    }
    let body: Value = response.json().await.unwrap_or_default();
    Err(error_message(&body))
}

#[tauri::command]
pub async fn account_login(app: AppHandle, email: String, password: String) -> Result<AccountState, String> {
    let (status, body) = post("login", json!({ "email": email, "password": password }), None).await?;
    if status != 200 {
        return Err(error_message(&body));
    }
    let account = account_from(email.trim(), &body)?;
    store(&app, Some(account.clone()))?;
    Ok(state_from(&account, false))
}

#[tauri::command]
pub async fn account_change_password(app: AppHandle, password: String) -> Result<AccountState, String> {
    let Some(account) = settings::load(&app).account else { return Err("Please sign in again.".into()) };
    let (status, body) = post("password", json!({ "password": password }), Some(&account.access_token)).await?;
    if status != 200 {
        return Err(error_message(&body));
    }
    // A new session comes back with the new password; without it the old one is dead.
    let updated = if body["session"].is_object() {
        account_from(&account.email, &body)?
    } else {
        let mut a = account;
        a.license.must_change_password = false;
        a
    };
    store(&app, Some(updated.clone()))?;
    Ok(state_from(&updated, false))
}

#[tauri::command]
pub fn account_logout(app: AppHandle) -> Result<(), String> {
    store(&app, None)
}

/// Where the DJ stands right now. Refreshes the session and licence from the
/// server; if the server can't be reached, trusts the last answer for a
/// short grace period.
#[tauri::command]
pub async fn account_state(app: AppHandle) -> AccountState {
    let Some(account) = settings::load(&app).account else { return signed_out() };

    match post("refresh", json!({ "refresh_token": account.refresh_token }), None).await {
        Ok((200, body)) => match account_from(&account.email, &body) {
            Ok(fresh) => {
                let _ = store(&app, Some(fresh.clone()));
                state_from(&fresh, false)
            }
            Err(_) => offline_state(&account),
        },
        // The server said the session is no longer valid: sign out.
        Ok((401, _)) => {
            let _ = store(&app, None);
            signed_out()
        }
        // Network failure or a server error: fall back to the last answer.
        _ => offline_state(&account),
    }
}

fn offline_state(account: &StoredAccount) -> AccountState {
    let mut state = state_from(account, true);
    let within_grace = now().saturating_sub(account.checked_at) <= OFFLINE_GRACE_SECS;
    let trial_over = account.license.reason == "trial"
        && account.license.trial_ends_at.as_deref().and_then(parse_rfc3339_secs).is_some_and(|end| now() >= end);
    if !within_grace || trial_over {
        state.allowed = false;
        if state.reason != "trial-ended" {
            state.reason = "offline-expired".into();
        }
    }
    state
}

/// Seconds since the epoch for an ISO-8601 UTC timestamp like `2026-11-04T10:00:00.000Z`.
fn parse_rfc3339_secs(s: &str) -> Option<u64> {
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let mut t = time.trim_end_matches('Z').split(':');
    let (h, min) = (t.next()?.parse::<i64>().ok()?, t.next()?.parse::<i64>().ok()?);
    let sec = t.next().and_then(|x| x.split('.').next()).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
    // Days from civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    let total = days * 86400 + h * 3600 + min * 60 + sec;
    u64::try_from(total).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_server_timestamps() {
        assert_eq!(parse_rfc3339_secs("1970-01-02T00:00:00.000Z"), Some(86_400));
        assert_eq!(parse_rfc3339_secs("2026-12-01T00:00:00Z"), Some(1_796_083_200));
        assert_eq!(parse_rfc3339_secs("nonsense"), None);
    }

    #[test]
    fn an_offline_trial_cannot_outlive_its_end_date() {
        let account = StoredAccount {
            email: "a@b.c".into(),
            access_token: String::new(),
            refresh_token: String::new(),
            license: License { allowed: true, reason: "trial".into(), trial_ends_at: Some("2020-01-01T00:00:00Z".into()), ..Default::default() },
            checked_at: now(),
        };
        assert!(!offline_state(&account).allowed);
    }

    #[test]
    fn a_recent_subscriber_keeps_working_offline() {
        let account = StoredAccount {
            email: "a@b.c".into(),
            access_token: String::new(),
            refresh_token: String::new(),
            license: License { allowed: true, reason: "subscriber".into(), ..Default::default() },
            checked_at: now(),
        };
        assert!(offline_state(&account).allowed);
    }
}
