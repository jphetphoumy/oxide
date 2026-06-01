use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use crate::auth::device_flow::{
    DEFAULT_REGION, WORKOS_AUTHENTICATE_URL, build_http_client, workos_client_id,
};
use crate::auth::jwt::decode_payload;
use crate::auth::token_storage;

const EXPIRY_SKEW: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize)]
struct ExpClaims {
    exp: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RefreshResponse {
    Success {
        access_token: String,
        refresh_token: String,
    },
    Error {
        error: String,
        error_description: Option<String>,
    },
}

pub fn is_token_expired(access_token: &str) -> bool {
    let Some(expiry) = decode_claims(access_token).and_then(|claims| claims.exp) else {
        return true;
    };

    let now = unix_timestamp();
    let skew = EXPIRY_SKEW.as_secs();
    now >= expiry.saturating_sub(skew)
}

pub async fn get_valid_token() -> Result<String> {
    let access_token = token_storage::get_access_token()?
        .ok_or_else(|| anyhow!("Not logged in. Run `oxide login` first."))?;

    if !is_token_expired(&access_token) {
        return Ok(access_token);
    }

    refresh_tokens().await
}

pub async fn refresh_tokens() -> Result<String> {
    let refresh_token = token_storage::get_refresh_token()?
        .ok_or_else(|| anyhow!("No refresh token. Run `oxide login`."))?;

    let http = build_http_client()?;
    let client_id = workos_client_id();
    let response = http
        .post(WORKOS_AUTHENTICATE_URL)
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id.as_str()),
            ("refresh_token", refresh_token.as_str()),
        ])
        .send()
        .await
        .context("failed to refresh WorkOS access token")?;

    let status = response.status();
    if matches!(
        status,
        reqwest::StatusCode::BAD_REQUEST | reqwest::StatusCode::UNAUTHORIZED
    ) {
        token_storage::clear_all()?;
        bail!("Stored session is no longer valid. Run `oxide login` again.")
    }

    let refresh_response: RefreshResponse = response
        .error_for_status()
        .context("WorkOS rejected the refresh token request")?
        .json()
        .await
        .context("failed to decode WorkOS refresh response")?;
    match refresh_response {
        RefreshResponse::Success {
            access_token,
            refresh_token,
        } => {
            let region = crate::auth::device_flow::extract_region(&access_token);
            token_storage::save_access_token(&access_token)?;
            token_storage::save_refresh_token(&refresh_token)?;
            token_storage::save_region(if region.is_empty() {
                DEFAULT_REGION
            } else {
                &region
            })?;
            Ok(access_token)
        }
        RefreshResponse::Error {
            error,
            error_description,
        } => {
            let detail = error_description.unwrap_or_default();
            bail!(
                "{}",
                format!("Token refresh failed: {error} {detail}").trim()
            )
        }
    }
}

fn decode_claims(access_token: &str) -> Option<ExpClaims> {
    decode_payload::<ExpClaims>(access_token)
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jwt_with_exp(exp: u64) -> String {
        let payload = format!(r#"{{"exp":{exp}}}"#);
        let encoded =
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, payload);
        format!("e30.{encoded}.")
    }

    #[test]
    fn expired_tokens_are_reported_as_expired() {
        let token = jwt_with_exp(unix_timestamp().saturating_sub(1));
        assert!(is_token_expired(&token));
    }

    #[test]
    fn near_expiry_tokens_are_reported_as_expired() {
        let token = jwt_with_exp(unix_timestamp().saturating_add(10));
        assert!(is_token_expired(&token));
    }

    #[test]
    fn healthy_tokens_are_reported_as_valid() {
        let token = jwt_with_exp(unix_timestamp().saturating_add(300));
        assert!(!is_token_expired(&token));
    }

    #[test]
    fn malformed_tokens_are_treated_as_expired() {
        assert!(is_token_expired("bad-token"));
    }
}
