//! Who the caller is.
//!
//! D11 decided this on 2026-08-02 and nothing built it:
//!
//! > `recorded_by_id` comes from the authenticated session, is never
//! > client-supplied, and is never editable. That is the non-repudiable floor:
//! > whatever else is claimed, we always know which person, on which device,
//! > recorded this.
//!
//! Until this module the floor was a uuid the request asserted, so
//! architecture.md's *"every movement and every scan records an individual"* was
//! enforced by a CHECK and undone by anyone willing to type somebody else's
//! identifier. Question 171.
//!
//! # The shape, and why
//!
//! A session is a **server-side record** with an opaque token, not a signed
//! claim the client carries. That is OWASP's position and it buys the thing a
//! warehouse actually needs: revocation. A packer who leaves mid-shift is signed
//! out by one UPDATE, and a stolen token dies with it.
//!
//! The token is 32 bytes from the OS generator — well past OWASP's 64-bit floor
//! — and **only its SHA-256 is stored**. A database backup that leaks must not
//! hand over live sessions, which is the same argument as for a password one
//! layer out. It is hashed rather than encrypted because nothing ever needs to
//! read it back.
//!
//! Two transports, one record. Browsers get a `__Host-` cookie, which pins the
//! cookie to this origin, forbids a `Domain`, and requires `Secure` and `Path=/`
//! — so a subdomain cannot set or read it. Non-browser clients, which is what
//! D5's handhelds are, send the same token as a bearer. One row to revoke either
//! way.
//!
//! # What is deliberately absent
//!
//! No authorisation. `person_tenant` decides which tenants a person may act for
//! and nothing decides what they may do there; question 176 carries the role
//! vocabulary, because inventing one here would be the undesigned language D22
//! refuses everywhere else. And no `work_session`: D11 has a crew declared at
//! sign-on, which is the picking story and question 177.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// How long a session lives without being used, and at all.
///
/// OWASP's bands for a low-risk application are 15–30 minutes idle and 4–8 hours
/// absolute, the latter reasoned from a working day. A pack station sits idle
/// between trucks, so this takes the top of the idle band and the middle of the
/// absolute one: long enough not to sign somebody out while they walk to the
/// dock, short enough that an unattended browser is not a standing invitation.
///
/// **Enforced in `session_resolve`, not here.** A timeout the application
/// applies is a timeout the application can forget.
pub const IDLE_TIMEOUT_MINUTES: i64 = 30;
pub const ABSOLUTE_LIFETIME_HOURS: i64 = 8;

/// The same limits when the person ticked "keep me signed in on this device".
///
/// A week idle covers a weekend and a day off; thirty days absolute means a
/// fresh proof of identity at least monthly. The token is rotated daily
/// ([`ROTATE_AFTER_HOURS`]) so a copy of the cookie is good for a day, not a
/// month. The choice is the person's, on their own device: a shared bench
/// leaves the box clear and keeps the short limits above.
pub const REMEMBERED_IDLE_DAYS: i64 = 7;
pub const REMEMBERED_LIFETIME_DAYS: i64 = 30;

/// How old a remembered session's token may get before it is replaced.
pub const ROTATE_AFTER_HOURS: i64 = 24;

/// The two limits a session is opened with, in whole minutes because that is
/// what `make_interval` is handed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    pub remembered: bool,
    pub idle_minutes: i32,
    pub lifetime_minutes: i32,
}

impl Policy {
    pub fn new(remembered: bool) -> Self {
        let (idle, lifetime) = if remembered {
            (Duration::days(REMEMBERED_IDLE_DAYS), Duration::days(REMEMBERED_LIFETIME_DAYS))
        } else {
            (Duration::minutes(IDLE_TIMEOUT_MINUTES), Duration::hours(ABSOLUTE_LIFETIME_HOURS))
        };
        Self {
            remembered,
            idle_minutes: idle.num_minutes() as i32,
            lifetime_minutes: lifetime.num_minutes() as i32,
        }
    }

    /// When a session opened now would expire.
    pub fn expiry(&self, from: DateTime<Utc>) -> DateTime<Utc> {
        from + Duration::minutes(self.lifetime_minutes.into())
    }
}

/// The cookie name. `__Host-` is a browser-enforced prefix: the cookie is
/// refused unless it is `Secure`, has no `Domain`, and has `Path=/`, which stops
/// a sibling subdomain planting one.
pub const COOKIE_NAME: &str = "__Host-id";

/// The name used when `Secure` cannot be set, and therefore `__Host-` cannot be.
///
/// A browser refuses a `__Host-` cookie without TLS, so plain-HTTP development
/// and CI need a plainer name. **Both are accepted on the way in**, which the
/// first version did not do: it set `id` and read only `__Host-id`, so the
/// cookie transport never worked in the one mode the demo actually runs in.
/// Every test used the bearer header and none of them noticed.
pub const COOKIE_NAME_INSECURE: &str = "id";

/// Argon2id at OWASP's `m=19456 (19 MiB), t=2, p=1`.
///
/// OWASP lists five parameter sets it calls an equal level of defence, trading
/// memory against time. This one is chosen over the 46 MiB single-pass variant
/// for a reason specific to this deployment: the server shares a machine with
/// Postgres, and memory is the resource Postgres is already using. Two passes
/// over 19 MiB costs the same to an attacker and less to the database.
fn argon2() -> Argon2<'static> {
    let params = Params::new(19_456, 2, 1, None).expect("OWASP's parameters are valid");
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
}

/// Hash a password for storage, salt and parameters included.
///
/// The result is a PHC string, so the algorithm and cost travel with the digest
/// and can be raised later without a flag day: an old hash still verifies under
/// the parameters it names.
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut rand::rngs::OsRng);
    Ok(argon2().hash_password(password.as_bytes(), &salt)?.to_string())
}

/// Verify a password against a stored PHC string.
///
/// Returns `false` rather than an error for a malformed hash: a credential row
/// nobody can parse is a credential nobody can log in with, and the caller must
/// answer that identically to a wrong password anyway.
pub fn verify_password(password: &str, phc: &str) -> bool {
    match PasswordHash::new(phc) {
        Ok(parsed) => argon2()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// A session token, and the digest that is stored in its place.
pub struct MintedToken {
    /// Given to the client once and never recoverable afterwards.
    pub token: String,
    pub sha256: Vec<u8>,
}

/// 32 bytes from the OS generator, hex encoded.
///
/// OWASP's floor is 64 bits of entropy; this is 256. Hex rather than base64 so
/// the token is unambiguous in a header, a log-scrubbing rule, or a curl command
/// somebody pastes into a terminal.
pub fn mint_token() -> MintedToken {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let token = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let sha256 = token_digest(&token);
    MintedToken { token, sha256 }
}

/// The digest a token is stored and looked up by.
pub fn token_digest(token: &str) -> Vec<u8> {
    Sha256::digest(token.as_bytes()).to_vec()
}

/// What an import token looks like, and why it looks like anything at all.
///
/// **Both credential kinds arrive as `Authorization: Bearer`.** Without a mark,
/// resolving one means trying `session_resolve`, and on a miss trying
/// `api_token_resolve` — two round trips for every request made by the kind
/// that loses the coin toss, and a resolver whose behaviour depends on the
/// order somebody wrote the two branches in.
///
/// A prefix settles it in one lookup, and buys a second thing worth more: a
/// leaked token is *identifiable*. `nyl_` followed by 64 hex characters is a
/// string a secret scanner can be taught, which a bare 64-hex string is not —
/// it is indistinguishable from a digest, a commit id, or a session token.
///
/// The prefix is inside the digested string rather than stripped before
/// hashing, so nothing has to remember to put it back.
pub const API_TOKEN_PREFIX: &str = "nyl_";

/// Whether a bearer names an import token rather than a session.
pub fn is_api_token(token: &str) -> bool {
    token.starts_with(API_TOKEN_PREFIX)
}

/// The same 256 bits as [`mint_token`], wearing its kind on the front.
pub fn mint_api_token() -> MintedToken {
    let mut bytes = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut bytes);
    let token = format!(
        "{API_TOKEN_PREFIX}{}",
        bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
    );
    let sha256 = token_digest(&token);
    MintedToken { token, sha256 }
}

/// A program acting for a tenant, once its token is resolved.
///
/// **Deliberately not a [`Caller`].** A `Caller` carries a `session_id` and
/// D11's `recorded_by_id`, because it is a person who signed on; a loader is
/// neither. The import path writes `site`, `location`, `item` and
/// `reported_stock`, none of which carries `recorded_by_id`, so nothing here
/// needs the attribution floor — and giving a token one anyway would put a
/// person's name on acts they did not perform.
///
/// `created_by_id` is who is answerable for the token existing, which is a
/// different claim and the honest one.
#[derive(Clone, Debug)]
pub struct Machine {
    pub token_id: Uuid,
    pub tenant_id: Uuid,
    pub created_by_id: Uuid,
    pub label: String,
}

/// The caller, once resolved. Everything a write path needs to name an act.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caller {
    pub session_id: Uuid,
    /// D11's non-repudiable floor. This is `recorded_by_id`, and it comes from
    /// here rather than from the request.
    pub person_id: Uuid,
    pub tenant_id: Uuid,
    /// Where they signed on. `client_event.site_id` for every act they record.
    pub site_id: Option<Uuid>,
    /// Opened with "keep me signed in", so a site change keeps the long limits.
    pub remembered: bool,
    /// When the current token was issued: sign-on, or the last rotation.
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

/// Where a token may arrive, in the order it is looked for.
///
/// The cookie comes first because a browser sends it automatically and a bearer
/// header on the same request would most likely be a confused deputy. Both name
/// the same record.
pub fn token_from_headers(cookie: Option<&str>, authorization: Option<&str>) -> Option<String> {
    if let Some(raw) = cookie {
        // Split each pair on the first `=` and compare the **name**, rather than
        // testing a prefix: `not__Host-id=x` is not our cookie, and a prefix test
        // is how that sort of thing slips through.
        for part in raw.split(';') {
            let Some((name, value)) = part.trim().split_once('=') else {
                continue;
            };
            let name = name.trim();
            if (name == COOKIE_NAME || name == COOKIE_NAME_INSECURE) && !value.is_empty() {
                return Some(value.trim().to_string());
            }
        }
    }
    authorization
        .and_then(|h| h.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
}

/// The `Set-Cookie` value for a session that ends at `expires_at`.
///
/// `__Host-` requires `Secure` and `Path=/` and forbids `Domain`; `HttpOnly`
/// keeps it away from script, which is what limits an XSS to acting through the
/// page rather than walking off with the session; `SameSite=Strict` is what
/// stands in for a CSRF token here, since the cookie is simply not sent on a
/// cross-site request.
///
/// **A `Max-Age`, matching the server's absolute expiry.** This used to be a
/// browser-session cookie on OWASP's preference, and the result was a sign-in
/// page every time the browser was reopened, which is most mornings. The
/// server-side record is still what decides: the idle limit, the absolute one
/// and revocation are all enforced in `session_resolve`, so a cookie that
/// outlives its session is just a dead token. `Max-Age` only stops the browser
/// throwing away a session that is still good.
pub fn session_cookie(token: &str, secure: bool, expires_at: DateTime<Utc>) -> String {
    let max_age = (expires_at - Utc::now()).num_seconds().max(0);
    // `Secure` is mandatory for `__Host-`, so a plain-HTTP development server
    // could not set the cookie at all. Under `secure = false` the prefix is
    // dropped along with the flag, which keeps local development working without
    // pretending the weaker cookie is the same thing.
    if secure {
        format!("{COOKIE_NAME}={token}; Secure; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age}")
    } else {
        format!("{COOKIE_NAME_INSECURE}={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age={max_age}")
    }
}

/// The `Set-Cookie` that removes it.
pub fn clearing_cookie(secure: bool) -> String {
    if secure {
        format!("{COOKIE_NAME}=; Secure; HttpOnly; SameSite=Strict; Path=/; Max-Age=0")
    } else {
        format!("{COOKIE_NAME_INSECURE}=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_verifies_against_its_own_hash_and_nothing_else() {
        let phc = hash_password("correct horse battery staple").unwrap();
        assert!(verify_password("correct horse battery staple", &phc));
        assert!(!verify_password("Correct horse battery staple", &phc));
        assert!(!verify_password("", &phc));
    }

    #[test]
    fn the_same_password_hashes_differently_every_time() {
        // The salt is per-hash, so two people with one password do not share a
        // digest and a precomputed table buys nothing.
        let a = hash_password("gloves").unwrap();
        let b = hash_password("gloves").unwrap();
        assert_ne!(a, b);
        assert!(verify_password("gloves", &a) && verify_password("gloves", &b));
    }

    #[test]
    fn the_stored_hash_names_the_parameters_it_used() {
        // A PHC string, so raising the cost later does not invalidate old rows.
        let phc = hash_password("gloves").unwrap();
        assert!(phc.starts_with("$argon2id$"), "{phc}");
        assert!(phc.contains("m=19456"), "OWASP's memory cost: {phc}");
        assert!(phc.contains("t=2"), "two passes: {phc}");
        assert!(phc.contains("p=1"), "one lane: {phc}");
    }

    #[test]
    fn a_garbled_credential_is_a_refusal_rather_than_a_crash() {
        assert!(!verify_password("gloves", "not a phc string"));
        assert!(!verify_password("gloves", ""));
    }

    #[test]
    fn tokens_are_long_unique_and_stored_only_as_a_digest() {
        let a = mint_token();
        let b = mint_token();
        assert_eq!(a.token.len(), 64, "32 bytes, hex: 256 bits against OWASP's 64");
        assert_ne!(a.token, b.token);
        assert_eq!(a.sha256.len(), 32);
        assert_ne!(
            a.sha256,
            a.token.as_bytes().to_vec(),
            "what is stored is not what is presented"
        );
        assert_eq!(a.sha256, token_digest(&a.token), "and the lookup agrees");
    }

    #[test]
    fn a_token_is_found_in_either_transport() {
        let cookie = format!("{COOKIE_NAME}=abc123");
        assert_eq!(token_from_headers(Some(&cookie), None).as_deref(), Some("abc123"));
        assert_eq!(
            token_from_headers(None, Some("Bearer xyz789")).as_deref(),
            Some("xyz789")
        );
        // Alongside other cookies, which is the ordinary case in a browser.
        let mixed = format!("theme=dark; {COOKIE_NAME}=abc123; other=1");
        assert_eq!(token_from_headers(Some(&mixed), None).as_deref(), Some("abc123"));
        // The cookie wins: a browser sends it by itself, so a bearer beside it is
        // more likely a confused deputy than a second opinion.
        assert_eq!(
            token_from_headers(Some(&cookie), Some("Bearer xyz789")).as_deref(),
            Some("abc123")
        );
    }

    #[test]
    fn the_insecure_cookie_is_read_back() {
        // **The bug this test exists for.** `session_cookie(.., false)` sets a
        // cookie named `id`, and the extractor read only `__Host-id`, so the
        // cookie transport was dead in exactly the mode local development and CI
        // use. Every test reached for the bearer header instead and none of them
        // noticed until a browser did.
        let issued = session_cookie("abc123", false, Utc::now());
        let name = issued.split('=').next().unwrap();
        assert_eq!(name, COOKIE_NAME_INSECURE);
        assert_eq!(
            token_from_headers(Some(&format!("{name}=abc123")), None).as_deref(),
            Some("abc123"),
            "what is set must be what is read"
        );

        // And the same for the hardened one, which is the property that held.
        let issued = session_cookie("abc123", true, Utc::now());
        let name = issued.split('=').next().unwrap();
        assert_eq!(name, COOKIE_NAME);
        assert_eq!(
            token_from_headers(Some(&format!("{name}=abc123")), None).as_deref(),
            Some("abc123")
        );
    }

    #[test]
    fn nothing_is_a_token() {
        assert_eq!(token_from_headers(None, None), None);
        assert_eq!(token_from_headers(Some("theme=dark"), None), None);
        assert_eq!(token_from_headers(None, Some("Basic abc")), None);
        assert_eq!(token_from_headers(None, Some("Bearer ")), None);
        // A cookie whose name merely ends the same way is not ours.
        assert_eq!(token_from_headers(Some("not__Host-id=abc"), None), None);
    }

    #[test]
    fn the_secure_cookie_carries_every_flag_the_prefix_requires() {
        let c = session_cookie("abc", true, Utc::now() + Duration::hours(1));
        assert!(c.starts_with("__Host-id=abc"));
        for flag in ["Secure", "HttpOnly", "SameSite=Strict", "Path=/"] {
            assert!(c.contains(flag), "{flag} missing from {c}");
        }
        assert!(!c.contains("Domain"), "__Host- forbids a Domain: {c}");
    }

    #[test]
    fn the_cookie_lives_as_long_as_the_session() {
        // The regression: without a Max-Age the cookie died with the browser
        // and every reopen was a sign-in, whatever the server thought.
        let c = session_cookie("abc", false, Utc::now() + Duration::hours(8));
        let age: i64 = c
            .split("Max-Age=")
            .nth(1)
            .expect("a persistent cookie")
            .parse()
            .unwrap();
        assert!((8 * 3600 - 5..=8 * 3600).contains(&age), "{c}");

        // And never negative, which a browser would read as "delete".
        assert!(session_cookie("abc", false, Utc::now() - Duration::hours(1)).ends_with("Max-Age=0"));
    }

    #[test]
    fn remembering_lengthens_both_limits() {
        let short = Policy::new(false);
        assert_eq!((short.idle_minutes, short.lifetime_minutes), (30, 8 * 60));
        let long = Policy::new(true);
        assert_eq!((long.idle_minutes, long.lifetime_minutes), (7 * 24 * 60, 30 * 24 * 60));
        assert!(long.remembered && !short.remembered);
    }

    #[test]
    fn the_clearing_cookie_expires_immediately() {
        assert!(clearing_cookie(true).contains("Max-Age=0"));
    }
}
