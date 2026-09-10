//! What the API trusts a token to say.

use serde::Deserialize;

/// The claims we require and read. Anything else the auth service puts in a
/// token is ignored: this API should depend on as little of that service's
/// shape as possible.
#[derive(Debug, Clone, Deserialize)]
pub struct Claims {
    /// Stable identifier for the account. The only link between a token and a
    /// row in `users`, and the reason identity can be re-pointed at a
    /// different provider without touching the domain.
    pub sub: String,
    pub email: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub picture: Option<String>,
}

// `exp`, `aud` and `iss` are deliberately absent: jsonwebtoken validates those
// against the raw token before deserializing into this struct, so repeating
// them here would add fields nothing reads. The next phase adds `provider` and
// `amr`, which per-organization sign-in policy does read.
