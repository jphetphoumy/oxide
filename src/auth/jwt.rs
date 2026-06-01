use serde::de::DeserializeOwned;

pub fn decode_payload<T>(access_token: &str) -> Option<T>
where
    T: DeserializeOwned,
{
    // We intentionally decode the JWT payload without signature verification here because
    // these helpers are used only for non-security-critical metadata like region and expiry.
    // Authentication decisions must not rely on claims decoded this way.
    let payload = access_token.split('.').nth(1)?;
    let decoded =
        base64::Engine::decode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, payload).ok()?;
    serde_json::from_slice::<T>(&decoded).ok()
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize, PartialEq, Eq)]
    struct Claims {
        region: Option<String>,
    }

    fn jwt(payload: &str) -> String {
        let encoded =
            base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, payload);
        format!("e30.{encoded}.")
    }

    #[test]
    fn decode_payload_reads_claims() {
        let token = jwt(r#"{"region":"europe-west1"}"#);
        let claims = decode_payload::<Claims>(&token);
        assert_eq!(
            claims,
            Some(Claims {
                region: Some("europe-west1".to_string())
            })
        );
    }

    #[test]
    fn decode_payload_returns_none_for_malformed_jwt() {
        assert_eq!(decode_payload::<Claims>("not-a-jwt"), None);
    }

    #[test]
    fn decode_payload_returns_none_for_invalid_json() {
        let token = jwt("not-json");
        assert_eq!(decode_payload::<Claims>(&token), None);
    }
}
