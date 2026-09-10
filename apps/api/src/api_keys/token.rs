//! Generating and hashing API key tokens.

use rand::rngs::{SysError, SysRng};
use rand::TryRng;
use sha2::{Digest, Sha256};

/// Distinguishes our tokens in logs and secret scanners.
const PREFIX: &str = "iop_";
/// 32 bytes of CSPRNG output: far beyond any brute-force reach.
const TOKEN_BYTES: usize = 32;
/// How much of the token is safe to store and display.
const DISPLAY_PREFIX_LEN: usize = 12;

/// A freshly minted token and the two derived values we persist.
pub struct GeneratedToken {
    /// The plaintext. Returned to the caller once, never stored.
    pub token: String,
    /// Leading chars, safe to show in a list.
    pub prefix: String,
    /// What actually goes in the database.
    pub token_hash: String,
}

/// Mints a new token.
///
/// Fails only if the operating system's randomness source is unavailable,
/// which is why this returns a `Result` rather than panicking: a silently weak
/// credential is far worse than a failed request.
pub fn generate() -> Result<GeneratedToken, SysError> {
    // SysRng reads straight from the operating system rather than from a
    // userspace generator seeded once at startup. For a long-lived credential
    // that is the conservative choice, and it is why this call can fail.
    let mut bytes = [0u8; TOKEN_BYTES];
    SysRng.try_fill_bytes(&mut bytes)?;

    let token = format!("{PREFIX}{}", hex::encode(bytes));
    let prefix = token.chars().take(DISPLAY_PREFIX_LEN).collect();
    let token_hash = hash(&token);

    Ok(GeneratedToken { token, prefix, token_hash })
}

/// SHA-256, hex encoded. Lookups are by this value, so the raw token never
/// needs to be compared and never needs to be stored.
///
/// Deliberately NOT argon2 or bcrypt. Those are slow by design to make
/// low-entropy passwords expensive to guess. A token here is 32 random bytes,
/// so there is nothing to guess, and this runs on every ingest request where
/// an expensive hash would be a denial-of-service vector.
pub fn hash(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_prefixed_and_unique() {
        let a = generate().expect("rng available");
        let b = generate().expect("rng available");
        assert!(a.token.starts_with(PREFIX));
        assert_ne!(a.token, b.token, "two tokens must never collide");
        assert_ne!(a.token_hash, b.token_hash);
    }

    #[test]
    fn prefix_is_a_visible_slice_of_the_token() {
        let g = generate().expect("rng available");
        assert_eq!(g.prefix.len(), DISPLAY_PREFIX_LEN);
        assert!(g.token.starts_with(&g.prefix));
    }

    #[test]
    fn hash_is_stable_and_not_the_token() {
        let g = generate().expect("rng available");
        assert_eq!(hash(&g.token), g.token_hash, "hashing must be deterministic");
        assert!(!g.token_hash.contains(&g.token));
        assert_eq!(g.token_hash.len(), 64, "sha256 hex is 64 chars");
    }
}
