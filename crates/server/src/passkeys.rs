//! Signing in with a key the person holds.
//!
//! A password is a secret the server also knows. A passkey is a private key that
//! never leaves the authenticator: the server holds a public key, checks a
//! signature, and has nothing worth stealing. On a dock, where a handheld is
//! shared and a password is typed in front of whoever is standing there, that is
//! the difference between D11's non-repudiable floor holding and not.
//!
//! # What lives here
//!
//! The relying party configuration and the four ceremony halves. Verification
//! itself is [`webauthn_rs`], because a hand-rolled COSE parser and signature
//! check is the last thing this project should own.
//!
//! # The relying party is the domain, and that has consequences
//!
//! A credential is bound to the `rp_id` it was created against. A passkey made
//! on `nylonite.example.com` will not work on `something-else.example.com`, and
//! the browser enforces that rather than the server. So:
//!
//! - **The demo needs a stable hostname.** A quick tunnel's random name changes
//!   on every restart, and every passkey registered against the old one is dead.
//! - `localhost` is a secure context by specification, so development works with
//!   no certificate and no tunnel.
//!
//! `NYLONITE_RP_ID` and `NYLONITE_RP_ORIGIN` configure it, defaulting to
//! localhost. Getting them wrong is not a subtle failure: the browser refuses
//! before the request is sent.

use std::sync::{Arc, OnceLock};

use webauthn_rs::prelude::*;

/// How long a half-finished ceremony stays open.
///
/// Long enough to find the key in a pocket, short enough that an abandoned one
/// is not sitting there tomorrow. The browser's own timeout is 60s by default,
/// so this is deliberately the looser of the two.
pub const CEREMONY_TTL_SECONDS: i64 = 300;

/// The relying party: which origin these credentials belong to.
#[derive(Clone)]
pub struct Relying {
    pub webauthn: Arc<Webauthn>,
    pub rp_id: String,
    pub origin: String,
}

#[derive(Debug)]
pub enum Problem {
    /// The configured origin is not a URL, or does not match the id.
    Configuration(String),
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::Configuration(m) => write!(f, "webauthn configuration: {m}"),
        }
    }
}

impl Relying {
    /// **Reads the environment and fails loudly.** A relying party whose origin
    /// disagrees with its id produces credentials no browser will offer back,
    /// and the symptom is a sign-in button that silently does nothing. Better to
    /// refuse to start.
    pub fn from_env() -> Result<Self, Problem> {
        let rp_id = std::env::var("NYLONITE_RP_ID").unwrap_or_else(|_| "localhost".into());
        let origin = std::env::var("NYLONITE_RP_ORIGIN")
            .unwrap_or_else(|_| "http://localhost:18080".into());
        Self::new(&rp_id, &origin)
    }

    pub fn new(rp_id: &str, origin: &str) -> Result<Self, Problem> {
        let url = Url::parse(origin)
            .map_err(|e| Problem::Configuration(format!("{origin} is not a URL: {e}")))?;
        let webauthn = WebauthnBuilder::new(rp_id, &url)
            .map_err(|e| Problem::Configuration(e.to_string()))?
            .rp_name("Nylonite")
            .build()
            .map_err(|e| Problem::Configuration(e.to_string()))?;
        Ok(Self {
            webauthn: Arc::new(webauthn),
            rp_id: rp_id.to_string(),
            origin: origin.to_string(),
        })
    }
}

static SHARED: OnceLock<Relying> = OnceLock::new();

/// The process-wide relying party, built from the environment once.
///
/// **Not a field on `AppState`.** Every test that builds an `AppState` would
/// have to name it, and a test about packing has no opinion about WebAuthn. The
/// binary calls this at startup so a bad configuration stops the server rather
/// than surfacing as a sign-in button that does nothing; by the time a handler
/// asks, the answer is already there.
pub fn shared() -> Result<&'static Relying, Problem> {
    if let Some(r) = SHARED.get() {
        return Ok(r);
    }
    let built = Relying::from_env()?;
    Ok(SHARED.get_or_init(|| built))
}

/// What a stored passkey looks like to the rest of the server.
///
/// `verifier_state` is [`webauthn_rs`]'s own serialisation, round-tripped
/// exactly. Migration 75 says why it is text and not jsonb, and why nothing
/// queries into it.
pub struct Stored {
    pub id: Uuid,
    pub verifier_state: String,
}

impl Stored {
    pub fn passkey(&self) -> Result<Passkey, serde_json::Error> {
        serde_json::from_str(&self.verifier_state)
    }
}

/// What registration learned, for the columns migration 75 keeps beside the
/// opaque blob.
pub struct Registered {
    pub credential_id: Vec<u8>,
    pub verifier_state: String,
}

/// The columns migration 75 stores beside the opaque blob, at registration.
///
/// **Only two of them are knowable here, and the rest are honestly absent.**
/// `webauthn_rs` keeps a credential's internals sealed unless the
/// `danger-credential-internals` feature is on, which this does not enable — so
/// the make of authenticator and the backup flags are not available at
/// registration.
///
/// They are not invented. `AuthenticationResult` exposes the counter and the
/// backup state on every *use*, so `passkey_record_use` fills them in the first
/// time the key signs something. Until then they are NULL, which reads as
/// "nobody has told us" rather than as zero or false.
pub fn describe(passkey: &Passkey) -> Result<Registered, serde_json::Error> {
    Ok(Registered {
        credential_id: passkey.cred_id().as_ref().to_vec(),
        verifier_state: serde_json::to_string(passkey)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn localhost_is_a_valid_relying_party() {
        let r = Relying::new("localhost", "http://localhost:18080").expect("localhost");
        assert_eq!(r.rp_id, "localhost");
    }

    /// **The failure that would otherwise be a button that does nothing.** A
    /// credential is bound to its `rp_id`, and an origin that does not belong to
    /// that id produces credentials the browser will never offer back.
    #[test]
    fn an_origin_that_does_not_match_the_id_is_refused() {
        assert!(Relying::new("nylonite.example.com", "https://something-else.test").is_err());
        assert!(Relying::new("localhost", "not a url").is_err());
    }

    #[test]
    fn a_real_domain_configures() {
        let r = Relying::new("kyle.au", "https://nylonite.kyle.au").expect("subdomain origin");
        assert_eq!(r.rp_id, "kyle.au");
    }
}
