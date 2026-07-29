//! OAuth2 token exchange against Discord's HTTP API.
//!
//! Separate from [`crate::ipc`] on purpose: the authorisation code arrives over the local pipe,
//! but turning it into tokens is an ordinary HTTPS call to Discord's servers and has nothing to
//! do with the pipe's framing or lifetime.

use serde::Deserialize;
use serde_json::Value;

use crate::error::DiscordError;

const TOKEN_ENDPOINT: &str = "https://discord.com/api/v10/oauth2/token";

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: String,
}

/// Tokens issued by Discord.
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: String,
}

/// Redacts both values: these grant access to the user's Discord account, so no code path —
/// including a stray `{:?}` in a log line or a test failure message — may print them.
impl std::fmt::Debug for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tokens")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .finish()
    }
}

/// Exchanges an authorisation code for tokens.
pub async fn exchange_code(
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    code: &str,
) -> Result<Tokens, DiscordError> {
    post_token_request(&[
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri),
    ])
    .await
}

/// Renews an expired access token without user interaction.
pub async fn refresh_access_token(
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
    refresh_token: &str,
) -> Result<Tokens, DiscordError> {
    post_token_request(&[
        ("client_id", client_id),
        ("client_secret", client_secret),
        ("grant_type", "refresh_token"),
        ("refresh_token", refresh_token),
        ("redirect_uri", redirect_uri),
    ])
    .await
}

async fn post_token_request(form: &[(&str, &str)]) -> Result<Tokens, DiscordError> {
    let response = reqwest::Client::new()
        .post(TOKEN_ENDPOINT)
        .form(form)
        .send()
        .await?;

    let body = response.text().await?;
    parse_token_response(&body)
}

/// Turns Discord's reply into tokens, or into an actionable error.
///
/// The previous implementation read the fields with `trim_matches('"')`, so a failed exchange
/// silently stored the literal string `"null"` as the access token and the real reason was lost.
fn parse_token_response(body: &str) -> Result<Tokens, DiscordError> {
    let payload: Value = serde_json::from_str(body)?;

    if let Some(error) = payload.get("error").and_then(Value::as_str) {
        let description = payload
            .get("error_description")
            .and_then(Value::as_str)
            .unwrap_or("no description");
        return Err(DiscordError::OAuth {
            code: error.to_owned(),
            description: description.to_owned(),
            hint: hint_for(error),
        });
    }

    let tokens: TokenResponse = serde_json::from_value(payload)?;
    Ok(Tokens {
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
    })
}

/// Maps Discord's terse error codes to the setup step that actually went wrong.
///
/// These are the mistakes users make when registering their own application, and the raw codes
/// give them nothing to act on.
fn hint_for(error: &str) -> Option<&'static str> {
    match error {
        "invalid_redirect_uri" | "invalid_request" => Some(
            "Add http://localhost/ under OAuth2 → Redirects in your Discord application, then save",
        ),
        "invalid_client" => {
            Some("Check the Client ID and Client Secret; the secret is only shown once")
        }
        "invalid_grant" => {
            Some("The authorisation expired or was revoked. Connect again to reauthorise")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_successful_response_yields_both_tokens() {
        let tokens = parse_token_response(
            r#"{"access_token":"at","refresh_token":"rt","expires_in":604800}"#,
        )
        .expect("parses");

        assert_eq!(tokens.access_token, "at");
        assert_eq!(tokens.refresh_token, "rt");
    }

    #[test]
    fn a_failed_exchange_is_an_error_not_a_null_token() {
        // The old implementation stored the literal string "null" here and carried on as if it
        // had authenticated.
        let error = parse_token_response(
            r#"{"error":"invalid_client","error_description":"Invalid client secret"}"#,
        )
        .expect_err("must fail");

        let rendered = error.to_string();
        assert!(rendered.contains("invalid_client"));
        assert!(rendered.contains("Invalid client secret"));
    }

    #[test]
    fn the_forgotten_redirect_uri_gets_a_pointed_hint() {
        // The single most common setup mistake, and the raw code explains nothing.
        let error = parse_token_response(
            r#"{"error":"invalid_redirect_uri","error_description":"Invalid redirect uri"}"#,
        )
        .expect_err("must fail");

        assert!(error.to_string().contains("OAuth2 → Redirects"));
    }

    #[test]
    fn a_response_missing_the_tokens_fails_loudly() {
        assert!(parse_token_response(r#"{"token_type":"Bearer"}"#).is_err());
    }
}
