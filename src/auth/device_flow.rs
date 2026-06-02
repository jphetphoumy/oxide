use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

use crate::auth::jwt::decode_payload;
use crate::auth::token_storage;
use crate::auth::workspace_selection;

pub const WORKOS_AUTHORIZE_DEVICE_URL: &str =
    "https://api.workos.com/user_management/authorize/device";
pub const WORKOS_AUTHENTICATE_URL: &str = "https://api.workos.com/user_management/authenticate";
pub const DEFAULT_WORKOS_CLIENT_ID: &str = "client_01JGCT55T7FVDG9XF74925R1KT";
pub const OAUTH_SCOPE: &str = "openid profile email";
pub const DEFAULT_REGION: &str = "us-central1";

#[derive(Debug, Deserialize)]
pub struct DeviceCodeResponse {
    pub device_code: String,
    pub user_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: u64,
    pub interval: u64,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum TokenPollResponse {
    Success {
        access_token: String,
        refresh_token: String,
    },
    Error {
        error: String,
        error_description: Option<String>,
    },
}

pub async fn request_device_code(http: &reqwest::Client) -> Result<DeviceCodeResponse> {
    let client_id = workos_client_id();
    http.post(WORKOS_AUTHORIZE_DEVICE_URL)
        .form(&[("client_id", client_id.as_str()), ("scope", OAUTH_SCOPE)])
        .send()
        .await
        .context("failed to request WorkOS device code")?
        .error_for_status()
        .context("WorkOS rejected the device code request")?
        .json()
        .await
        .context("failed to decode WorkOS device code response")
}

pub async fn poll_for_token(
    http: &reqwest::Client,
    device_code: &str,
    interval: u64,
    expires_in: u64,
) -> Result<(String, String)> {
    let client_id = workos_client_id();
    let mut poll_interval = interval;
    let deadline = Instant::now() + Duration::from_secs(expires_in);

    while Instant::now() < deadline {
        let response = http
            .post(WORKOS_AUTHENTICATE_URL)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("device_code", device_code),
                ("client_id", client_id.as_str()),
            ])
            .send()
            .await
            .context("failed while polling WorkOS for device authorization")?;

        let status = response.status();
        let body = response.text().await.with_context(|| {
            format!("failed to read WorkOS polling response body (status: {status})")
        })?;
        let poll_response: TokenPollResponse = serde_json::from_str(&body).with_context(|| {
            format!("failed to parse WorkOS polling response body (status: {status}): {body}")
        })?;
        match poll_response {
            TokenPollResponse::Success {
                access_token,
                refresh_token,
            } => return Ok((access_token, refresh_token)),
            TokenPollResponse::Error { error, .. } if error == "authorization_pending" => {}
            TokenPollResponse::Error { error, .. } if error == "slow_down" => {
                poll_interval = poll_interval.saturating_add(5);
            }
            TokenPollResponse::Error { error, .. } if error == "expired_token" => {
                bail!("Device authorization expired. Run `oxide login` again.")
            }
            TokenPollResponse::Error {
                error,
                error_description,
            } if status.is_client_error() => {
                let detail = error_description.unwrap_or_default();
                bail!(
                    "{}",
                    format!("Device authorization failed: {error} {detail}").trim()
                )
            }
            TokenPollResponse::Error {
                error,
                error_description,
            } => {
                let detail = error_description.unwrap_or_default();
                bail!(
                    "{}",
                    format!("Unexpected WorkOS response ({status}): {error} {detail}").trim()
                )
            }
        }

        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                bail!("Login cancelled.")
            }
            () = tokio::time::sleep(Duration::from_secs(poll_interval)) => {}
        }
    }

    bail!("Device authorization timed out. Run `oxide login` again.")
}

pub fn extract_region(access_token: &str) -> String {
    decode_payload::<AccessClaims>(access_token)
        .and_then(|claims| claims.region)
        .unwrap_or_else(|| DEFAULT_REGION.to_string())
}

pub async fn login() -> Result<()> {
    let http = build_http_client()?;
    let device = request_device_code(&http).await?;

    println!("Your code: {}", device.user_code);
    println!("Open this URL in your browser to continue:");
    println!("{}", device.verification_uri);
    println!("{}", device.verification_uri_complete);
    let verification_uri_complete = device.verification_uri_complete.clone();
    if let Err(error) = tokio::task::spawn_blocking(move || open::that(&verification_uri_complete))
        .await
        .context("failed to join browser launch task")?
    {
        eprintln!("Failed to open browser automatically: {error}");
        eprintln!("Open the URL above manually to continue the login flow.");
    }

    println!("Waiting for authorization...");
    let (access_token, refresh_token) = poll_for_token(
        &http,
        &device.device_code,
        device.interval,
        device.expires_in,
    )
    .await?;

    let region = extract_region(&access_token);
    token_storage::save_access_token(&access_token)?;
    token_storage::save_refresh_token(&refresh_token)?;
    token_storage::save_region(&region)?;
    let _ = workspace_selection::select_workspace_for_login(&http).await?;

    println!("Logged in successfully.");
    Ok(())
}

#[derive(Debug, Deserialize)]
struct AccessClaims {
    #[serde(rename = "https://dust.tt/region")]
    region: Option<String>,
}

pub fn workos_client_id() -> String {
    resolve_workos_client_id(std::env::var("OXIDE_WORKOS_CLIENT_ID").ok())
}

pub fn build_http_client() -> Result<reqwest::Client> {
    crate::dust::client::build_http_client()
}

fn resolve_workos_client_id(value: Option<String>) -> String {
    value.unwrap_or_else(|| DEFAULT_WORKOS_CLIENT_ID.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workos_client_id_uses_default_when_env_is_missing() {
        assert_eq!(resolve_workos_client_id(None), DEFAULT_WORKOS_CLIENT_ID);
    }

    #[test]
    fn workos_client_id_uses_env_override_when_present() {
        assert_eq!(
            resolve_workos_client_id(Some("client_override".to_string())),
            "client_override"
        );
    }

    #[test]
    fn extract_region_reads_namespaced_claim() {
        let token = format!(
            "e30.{}.",
            base64::Engine::encode(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD,
                r#"{"https://dust.tt/region":"europe-west1"}"#
            )
        );
        assert_eq!(extract_region(&token), "europe-west1");
    }

    #[test]
    fn extract_region_defaults_when_claim_is_missing() {
        let token = format!(
            "e30.{}.",
            base64::Engine::encode(
                &base64::engine::general_purpose::URL_SAFE_NO_PAD,
                r#"{"sub":"user_123"}"#
            )
        );
        assert_eq!(extract_region(&token), DEFAULT_REGION);
    }

    #[test]
    fn extract_region_defaults_for_malformed_tokens() {
        assert_eq!(extract_region("not-a-jwt"), DEFAULT_REGION);
    }

    #[test]
    fn token_poll_response_deserializes_success_variant() {
        let json = r#"{"access_token":"access","refresh_token":"refresh"}"#;
        let response = serde_json::from_str::<TokenPollResponse>(json);
        assert_eq!(
            response.ok(),
            TokenPollResponse::Success {
                access_token: "access".to_string(),
                refresh_token: "refresh".to_string(),
            }
            .into()
        );
    }

    #[test]
    fn token_poll_response_deserializes_error_variant() {
        let json = r#"{"error":"authorization_pending","error_description":"waiting"}"#;
        let response = serde_json::from_str::<TokenPollResponse>(json);
        assert_eq!(
            response.ok(),
            TokenPollResponse::Error {
                error: "authorization_pending".to_string(),
                error_description: Some("waiting".to_string()),
            }
            .into()
        );
    }
}
