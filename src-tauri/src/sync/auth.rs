//! Supabase Auth client: email/password sign-in + sign-up, and Google via a
//! system-browser PKCE flow (no native Google SDK, no per-platform OAuth
//! client — see `wispr-fox-web/docs/SYNC_DESIGN.md`).
//!
//! Session state (access token + expiry) lives in-process only; the refresh
//! token is the durable bit and is stored keyring-FIRST, reusing the same
//! keyring service name as `secrets.rs`'s API-key storage. A plaintext file
//! fallback is used ONLY when the keyring genuinely fails to write or read —
//! per the project's hard rule for sync tokens (a stricter DPAPI-encrypted
//! fallback exists for API keys in `secrets.rs`; that machinery isn't
//! duplicated here since a plaintext fallback is explicitly permitted for
//! this narrower "keyring is broken on this machine" case).

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::AppHandle;

use super::config;

const KEYRING_SERVICE: &str = "wispr-fox";
const KEYRING_ENTRY: &str = "sync_refresh_token";
const FALLBACK_FILENAME: &str = ".sync-token.json";
const GOOGLE_CALLBACK_PORT: u16 = 43117;
const GOOGLE_SIGNIN_TIMEOUT: Duration = Duration::from_secs(180);

/// The signed-in user, as surfaced to the frontend. No tokens in here —
/// those never leave this module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUser {
    pub user_id: String,
    pub email: String,
}

#[derive(Debug, Clone)]
struct Session {
    access_token: String,
    refresh_token: String,
    expires_at: DateTime<Utc>,
    user_id: String,
    email: String,
}

static SESSION: OnceLock<Mutex<Option<Session>>> = OnceLock::new();
fn slot() -> &'static Mutex<Option<Session>> {
    SESSION.get_or_init(|| Mutex::new(None))
}

static HTTP: OnceLock<reqwest::Client> = OnceLock::new();
fn http() -> reqwest::Client {
    HTTP.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .connect_timeout(Duration::from_secs(6))
            .build()
            .expect("reqwest client construction is infallible with default config")
    })
    .clone()
}

// ── Refresh-token storage: keyring-first, plaintext-file-fallback-only-on-
// genuine-keyring-failure ────────────────────────────────────────────────

fn app_data_dir() -> PathBuf {
    let base = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.wispr-fox.app");
    let _ = std::fs::create_dir_all(&base);
    base
}

fn fallback_path() -> PathBuf {
    app_data_dir().join(FALLBACK_FILENAME)
}

fn keyring_entry() -> Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_ENTRY).context("opening sync-token keyring entry")
}

fn save_refresh_token(token: &str) {
    let keyring_ok = keyring_entry()
        .and_then(|e| e.set_password(token).map_err(|e| anyhow!(e)))
        .is_ok();
    if keyring_ok {
        let _ = std::fs::remove_file(fallback_path());
    } else {
        tracing::warn!("sync: keyring write failed, falling back to a local token file");
        let _ = std::fs::write(fallback_path(), token);
    }
}

fn load_refresh_token() -> Option<String> {
    if let Ok(entry) = keyring_entry() {
        if let Ok(pw) = entry.get_password() {
            if !pw.trim().is_empty() {
                return Some(pw);
            }
        }
    }
    std::fs::read_to_string(fallback_path())
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn clear_refresh_token() {
    if let Ok(entry) = keyring_entry() {
        let _ = entry.delete_credential();
    }
    let _ = std::fs::remove_file(fallback_path());
}

// ── Supabase GoTrue wire types ───────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
    #[serde(default)]
    expires_in: i64,
    user: TokenUser,
}

#[derive(Debug, Deserialize)]
struct TokenUser {
    id: String,
    #[serde(default)]
    email: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SupabaseError {
    #[serde(default)]
    error_description: Option<String>,
    #[serde(default)]
    msg: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

fn friendly_error(status: reqwest::StatusCode, body: &str) -> String {
    let parsed: SupabaseError = serde_json::from_str(body).unwrap_or_default();
    parsed
        .error_description
        .or(parsed.msg)
        .or(parsed.error)
        .unwrap_or_else(|| format!("HTTP {}", status.as_u16()))
}

async fn token_request(body: serde_json::Value, grant: &str) -> Result<TokenResponse> {
    if !config::is_configured() {
        return Err(anyhow!("Sync not configured in this build"));
    }
    let url = format!("{}/auth/v1/token?grant_type={grant}", config::SUPABASE_URL);
    let resp = http()
        .post(&url)
        .header("apikey", config::SUPABASE_ANON_KEY)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .context("could not reach the sync service")?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!(friendly_error(status, &text)));
    }
    serde_json::from_str(&text).context("decoding sync auth response")
}

fn store_session(tr: TokenResponse) -> AuthUser {
    let email = tr.user.email.clone().unwrap_or_default();
    let user_id = tr.user.id.clone();
    let expires_at = Utc::now() + chrono::Duration::seconds(tr.expires_in.max(60));
    save_refresh_token(&tr.refresh_token);
    *slot().lock() = Some(Session {
        access_token: tr.access_token,
        refresh_token: tr.refresh_token,
        expires_at,
        user_id: user_id.clone(),
        email: email.clone(),
    });
    AuthUser { user_id, email }
}

// ── Public API ────────────────────────────────────────────────────────────

/// Currently signed-in user, or `None` (signed out / not configured). Fast —
/// reads the in-memory session only, no network.
pub fn current_user() -> Option<AuthUser> {
    slot()
        .lock()
        .as_ref()
        .map(|s| AuthUser { user_id: s.user_id.clone(), email: s.email.clone() })
}

pub async fn sign_in_email(email: String, password: String) -> Result<AuthUser> {
    let tr = token_request(
        serde_json::json!({ "email": email, "password": password }),
        "password",
    )
    .await?;
    Ok(store_session(tr))
}

/// Sign up a new account. Supabase either returns a full session right away
/// (auto-confirm enabled on the project) or just a user record pending email
/// confirmation — in the latter case we surface a friendly message instead
/// of a confusing "signed in with no user" state.
pub async fn sign_up_email(email: String, password: String) -> Result<AuthUser> {
    if !config::is_configured() {
        return Err(anyhow!("Sync not configured in this build"));
    }
    let url = format!("{}/auth/v1/signup", config::SUPABASE_URL);
    let resp = http()
        .post(&url)
        .header("apikey", config::SUPABASE_ANON_KEY)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "email": email, "password": password }))
        .send()
        .await
        .context("could not reach the sync service")?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(anyhow!(friendly_error(status, &text)));
    }
    if let Ok(tr) = serde_json::from_str::<TokenResponse>(&text) {
        if !tr.access_token.is_empty() {
            return Ok(store_session(tr));
        }
    }
    Err(anyhow!(
        "Account created — check your email to confirm it, then sign in."
    ))
}

/// Ensure we have a live access token, refreshing (or restoring from the
/// stored refresh token) if needed. This is the one function everything else
/// in the sync engine calls before making an authenticated request.
pub async fn ensure_access_token() -> Result<String> {
    let existing = slot().lock().clone();
    if let Some(s) = &existing {
        if s.expires_at > Utc::now() + chrono::Duration::seconds(30) {
            return Ok(s.access_token.clone());
        }
    }
    let refresh_token = match &existing {
        Some(s) => s.refresh_token.clone(),
        None => load_refresh_token().ok_or_else(|| anyhow!("not signed in"))?,
    };
    let tr = token_request(
        serde_json::json!({ "refresh_token": refresh_token }),
        "refresh_token",
    )
    .await?;
    store_session(tr);
    // `store_session` unconditionally populates the slot, so this is safe.
    Ok(slot()
        .lock()
        .as_ref()
        .expect("session slot just set by store_session")
        .access_token
        .clone())
}

/// Called once at app launch. If a refresh token is on disk (from a previous
/// session) but nothing is loaded into memory yet, try to restore it. Best
/// effort — a stale/revoked token just leaves the app signed-out, exactly
/// like a fresh install.
pub async fn try_restore_session() {
    if !config::is_configured() {
        return;
    }
    if slot().lock().is_some() {
        return;
    }
    if load_refresh_token().is_none() {
        return;
    }
    if let Err(e) = ensure_access_token().await {
        tracing::info!("sync: no session to restore ({e:#})");
    }
}

/// Sign out: best-effort server-side revoke, then always clear local state
/// regardless of whether the network call succeeded — the user's intent is
/// "stop syncing on this device," and that must work offline too. Local
/// recordings are untouched.
pub async fn sign_out() {
    let token = slot().lock().as_ref().map(|s| s.access_token.clone());
    if let (Some(token), true) = (token, config::is_configured()) {
        let url = format!("{}/auth/v1/logout", config::SUPABASE_URL);
        let _ = http()
            .post(&url)
            .header("apikey", config::SUPABASE_ANON_KEY)
            .bearer_auth(token)
            .send()
            .await;
    }
    *slot().lock() = None;
    clear_refresh_token();
}

// ── Google sign-in: system-browser PKCE ─────────────────────────────────

static GOOGLE_CANCEL: AtomicBool = AtomicBool::new(false);

/// Abort an in-flight `sign_in_google` call. Best-effort — the accept loop
/// polls this roughly every 200ms, so cancellation lands quickly without
/// needing a hard socket interrupt.
pub fn cancel_google_sign_in() {
    GOOGLE_CANCEL.store(true, Ordering::SeqCst);
}

fn random_verifier() -> String {
    // High-entropy PKCE verifier from 3 UUIDv4s worth of random bytes
    // (uuid is already a dependency; no need to add `rand` just for this).
    let mut bytes = Vec::with_capacity(48);
    for _ in 0..3 {
        bytes.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    }
    URL_SAFE_NO_PAD.encode(bytes)
}

fn code_challenge_s256(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                if let Ok(byte) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
                out.push(bytes[i]);
                i += 1;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn query_param(path_and_query: &str, key: &str) -> Option<String> {
    let (_, query) = path_and_query.split_once('?')?;
    for pair in query.split('&') {
        let (k, v) = pair.split_once('=')?;
        if k == key {
            return Some(percent_decode(v));
        }
    }
    None
}

/// Blocks (on a dedicated blocking-pool thread — never the async runtime)
/// until the OAuth redirect hits our loopback listener, responds with a tiny
/// HTML page, and returns the `code` query parameter.
fn accept_callback(listener: std::net::TcpListener) -> Result<String> {
    listener
        .set_nonblocking(true)
        .context("configuring the local sign-in callback listener")?;
    let deadline = std::time::Instant::now() + GOOGLE_SIGNIN_TIMEOUT;
    GOOGLE_CANCEL.store(false, Ordering::SeqCst);

    loop {
        if GOOGLE_CANCEL.swap(false, Ordering::SeqCst) {
            return Err(anyhow!("Sign-in cancelled"));
        }
        if std::time::Instant::now() > deadline {
            return Err(anyhow!("Timed out waiting for the browser sign-in"));
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..n]).into_owned();
                let first_line = request.lines().next().unwrap_or("");
                let path = first_line.split_whitespace().nth(1).unwrap_or("");
                let code = query_param(path, "code");
                let error = query_param(path, "error");

                let ok = code.is_some();
                let (status_line, body) = if ok {
                    (
                        "200 OK",
                        "<html><body style=\"font-family:sans-serif;text-align:center;padding:60px\">\
                         <h2>You're signed in \u{2014} return to Wispr Fox.</h2>\
                         <p>You can close this tab now.</p></body></html>",
                    )
                } else {
                    (
                        "400 Bad Request",
                        "<html><body style=\"font-family:sans-serif;text-align:center;padding:60px\">\
                         <h2>Sign-in wasn't completed.</h2>\
                         <p>Close this tab and try again from Wispr Fox.</p></body></html>",
                    )
                };
                let response = format!(
                    "HTTP/1.1 {status_line}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes());
                let _ = stream.flush();

                if let Some(code) = code {
                    return Ok(code);
                }
                if let Some(err) = error {
                    return Err(anyhow!("Google sign-in was cancelled or failed: {err}"));
                }
                // Neither `code` nor `error` — e.g. a stray favicon request
                // from the callback tab. Keep waiting for the real redirect.
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => return Err(anyhow!("callback listener error: {e}")),
        }
    }
}

/// Google sign-in via the system browser + PKCE. Opens the browser through
/// the same `tauri-plugin-opener` mechanism the rest of the app uses for
/// external links (see `commands::reveal_folder`, onboarding's "Open
/// Deepgram" button). Cancellable via [`cancel_google_sign_in`].
pub async fn sign_in_google(app: AppHandle) -> Result<AuthUser> {
    if !config::is_configured() {
        return Err(anyhow!("Sync not configured in this build"));
    }

    let verifier = random_verifier();
    let challenge = code_challenge_s256(&verifier);

    let listener = std::net::TcpListener::bind(("127.0.0.1", GOOGLE_CALLBACK_PORT)).with_context(
        || {
            format!(
                "couldn't open the local sign-in port ({GOOGLE_CALLBACK_PORT}) — \
                 is another sign-in already in progress?"
            )
        },
    )?;

    let redirect = format!("http://127.0.0.1:{GOOGLE_CALLBACK_PORT}/callback");
    let auth_url = format!(
        "{}/auth/v1/authorize?provider=google&redirect_to={}&code_challenge={}&code_challenge_method=s256",
        config::SUPABASE_URL,
        percent_encode(&redirect),
        challenge,
    );

    {
        use tauri_plugin_opener::OpenerExt;
        app.opener()
            .open_url(auth_url, None::<&str>)
            .map_err(|e| anyhow!("couldn't open the system browser: {e}"))?;
    }

    let code = tauri::async_runtime::spawn_blocking(move || accept_callback(listener))
        .await
        .context("sign-in callback task panicked")??;

    let tr = token_request(
        serde_json::json!({ "auth_code": code, "code_verifier": verifier }),
        "pkce",
    )
    .await?;
    Ok(store_session(tr))
}
