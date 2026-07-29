//! Credentials for the Discord application the user registered.
//!
//! The values come from the user's own Discord application; this crate only consumes them. See
//! the `config` crate for how they are stored and where the redirect URI is defined.

/// OAuth2 scopes requested during `AUTHORIZE`.
///
/// `rpc` alone happens to be enough today, but the voice scopes are what the app actually uses;
/// requesting them explicitly avoids breaking if Discord tightens validation.
pub const DISCORD_SCOPES: &str = "rpc rpc.voice.read rpc.voice.write";

/// Everything needed to open an authenticated RPC session.
#[derive(Clone, PartialEq, Eq)]
pub struct DiscordCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_url: String,
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
}

impl DiscordCredentials {
    pub fn new(client_id: String, client_secret: String, redirect_url: String) -> Self {
        Self {
            client_id,
            client_secret,
            redirect_url,
            access_token: None,
            refresh_token: None,
        }
    }

    /// Attaches previously issued OAuth tokens, letting the app skip the authorisation modal.
    pub fn with_tokens(
        mut self,
        access_token: Option<String>,
        refresh_token: Option<String>,
    ) -> Self {
        self.access_token = access_token;
        self.refresh_token = refresh_token;
        self
    }
}

/// Deliberately opaque: credentials must never reach a log line.
impl std::fmt::Debug for DiscordCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DiscordCredentials")
            .field("client_id", &self.client_id)
            .field("client_secret", &"<redacted>")
            .field("redirect_url", &self.redirect_url)
            .field(
                "access_token",
                &self.access_token.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_never_leaks_the_secret_or_the_tokens() {
        let credentials = DiscordCredentials::new(
            "123456789".to_owned(),
            "super-secret-value".to_owned(),
            "http://localhost/".to_owned(),
        )
        .with_tokens(
            Some("access-value".to_owned()),
            Some("refresh-value".to_owned()),
        );

        let rendered = format!("{credentials:?}");

        assert!(!rendered.contains("super-secret-value"));
        assert!(!rendered.contains("access-value"));
        assert!(!rendered.contains("refresh-value"));
        // The client id is not a secret and stays visible, since it is useful when debugging.
        assert!(rendered.contains("123456789"));
    }

    #[test]
    fn requested_scopes_cover_reading_and_writing_voice_state() {
        assert!(DISCORD_SCOPES.contains("rpc.voice.read"));
        assert!(DISCORD_SCOPES.contains("rpc.voice.write"));
    }
}
