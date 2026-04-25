use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::{OAUTH_CLIENT_ID, OAUTH_SCOPES};

const KEYRING_SERVICE: &str = "stake-dev-tool";
const KEYRING_USER: &str = "github-oauth-token";

const DEVICE_CODE_URL: &str = "https://github.com/login/device/code";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const USER_URL: &str = "https://api.github.com/user";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCode {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GithubUser {
    pub id: u64,
    pub login: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthState {
    pub user: GithubUser,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceFlowPoll {
    /// Present once the user has authorised. Otherwise `None` and the caller
    /// should wait `next_interval_secs` before polling again.
    #[serde(default)]
    pub auth: Option<AuthState>,
    /// Seconds the caller must wait before the next poll. Updated by GitHub
    /// in response to `slow_down` errors — ignoring this locks us into an
    /// ever-escalating rate-limit loop.
    pub next_interval_secs: u64,
}

#[derive(Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    expires_in: u64,
    interval: u64,
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    interval: Option<u64>,
}

fn http_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("stake-dev-tool")
        .timeout(Duration::from_secs(15))
        .build()
        .context("build reqwest client")
}

/// Step 1 of Device Flow: request a code for the user to enter at
/// github.com/login/device.
pub async fn request_device_code() -> Result<DeviceCode> {
    let client = http_client()?;
    let res = client
        .post(DEVICE_CODE_URL)
        .header("Accept", "application/json")
        .form(&[("client_id", OAUTH_CLIENT_ID), ("scope", OAUTH_SCOPES)])
        .send()
        .await
        .context("request device code")?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(anyhow!("device code request failed: {status} {body}"));
    }

    let parsed: DeviceCodeResponse = res.json().await.context("parse device code")?;
    Ok(DeviceCode {
        device_code: parsed.device_code,
        user_code: parsed.user_code,
        verification_uri: parsed.verification_uri,
        expires_in: parsed.expires_in,
        interval: parsed.interval.max(1),
    })
}

/// Step 2 of Device Flow: poll once for a token. Call every `interval` seconds
/// until the user has authorized or the code expires.
///
/// Returns:
/// - `Ok(Some(token))` — user authorized, token stored in keyring, AuthState returned
/// - `Ok(None)` — not authorized yet, keep polling
/// - `Err(..)` — fatal error (expired, denied, network, etc.)
/// Default interval to request when GitHub doesn't specify one in a
/// `slow_down` response. 5s is the spec minimum.
const DEFAULT_POLL_INTERVAL_SECS: u64 = 5;

pub async fn poll_for_token(device_code: &str, current_interval: u64) -> Result<DeviceFlowPoll> {
    let client = http_client()?;
    let res = client
        .post(TOKEN_URL)
        .header("Accept", "application/json")
        .form(&[
            ("client_id", OAUTH_CLIENT_ID),
            ("device_code", device_code),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .context("poll for token")?;

    let status = res.status();
    let body_text = res.text().await.context("read token body")?;
    tracing::debug!(status = %status, body = %body_text, "device flow poll response");

    let parsed: TokenResponse = serde_json::from_str(&body_text).with_context(|| {
        format!("parse token response (status {status}, body: {body_text})")
    })?;

    if let Some(token) = parsed.access_token {
        tracing::info!("device flow: got access token");
        let user = fetch_user_with_token(&token).await?;
        store_token(&token)?;
        tracing::info!(login = %user.login, "device flow: signed in");
        return Ok(DeviceFlowPoll {
            auth: Some(AuthState { user }),
            next_interval_secs: current_interval,
        });
    }

    match parsed.error.as_deref() {
        Some("authorization_pending") => Ok(DeviceFlowPoll {
            auth: None,
            next_interval_secs: current_interval.max(DEFAULT_POLL_INTERVAL_SECS),
        }),
        // Per the RFC, `slow_down` responses carry a `interval` hint that MUST
        // be respected. If it's missing, fall back to +5s on whatever we were
        // using.
        Some("slow_down") => {
            let hint = parsed.interval.unwrap_or(current_interval + 5);
            let next = hint.max(current_interval + 5);
            tracing::info!(next_interval = next, "device flow: slow_down, backing off");
            Ok(DeviceFlowPoll {
                auth: None,
                next_interval_secs: next,
            })
        }
        Some(err) => Err(anyhow!(
            "device flow: {err}{}",
            parsed
                .error_description
                .map(|d| format!(" — {d}"))
                .unwrap_or_default()
        )),
        None => Err(anyhow!("device flow: unknown response — {body_text}")),
    }
}

async fn fetch_user_with_token(token: &str) -> Result<GithubUser> {
    let client = http_client()?;
    let res = client
        .get(USER_URL)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .context("fetch user")?;

    if !res.status().is_success() {
        let status = res.status();
        let body = res.text().await.unwrap_or_default();
        return Err(anyhow!("fetch user failed: {status} {body}"));
    }

    let user: GithubUser = res.json().await.context("parse user")?;
    Ok(user)
}

fn keyring_entry() -> Result<keyring::Entry> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).context("open keyring entry")
}

pub fn store_token(token: &str) -> Result<()> {
    keyring_entry()?
        .set_password(token)
        .context("store token in keyring")
}

pub fn load_token() -> Result<Option<String>> {
    match keyring_entry()?.get_password() {
        Ok(s) => Ok(Some(s)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e).context("load token from keyring"),
    }
}

pub fn clear_token() -> Result<()> {
    match keyring_entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e).context("clear token from keyring"),
    }
}

pub async fn current_user() -> Result<Option<GithubUser>> {
    let Some(token) = load_token()? else {
        return Ok(None);
    };
    match fetch_user_with_token(&token).await {
        Ok(u) => Ok(Some(u)),
        Err(e) => {
            tracing::warn!(error = %e, "stored token appears invalid");
            Ok(None)
        }
    }
}
