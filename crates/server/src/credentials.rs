//! Changing your own password.
//!
//! D142 built the way into a deployment and named this as the thing it owed:
//! the password chosen at setup could not be changed by anybody, through
//! anything, because `spork_app` holds no grant on `person_credential` at
//! all. On a deployment reachable from the internet that is not an inconvenience
//! — this project's own ran for a day on `fixtures/seed.sql`, whose password is
//! committed in this repository.
//!
//! # The current password is required, and a session is not enough
//!
//! It looks redundant: the caller is already signed in, so what does knowing the
//! old password add? It adds the difference between a stolen session and a
//! stolen account. A session expires, can be revoked, and dies with the browser;
//! a password does not. Without this check, anyone who gets thirty seconds at an
//! unlocked terminal converts a session that would have lapsed by evening into
//! permanent access, and the real owner cannot take it back. OWASP asks for it
//! for exactly that reason, and it is the same reason [`session_revoke_others`]
//! runs afterwards.
//!
//! [`session_revoke_others`]: https://owasp.org/www-project-cheat-sheets/
//!
//! # And it is counted, because guessing it is an attack
//!
//! A wrong current password advances the same lockout counter a wrong sign-on
//! does, and a locked account is refused here before the digest is even
//! compared. The threat is precise: somebody holding a session they should not
//! have, trying to make it permanent. An endpoint that let them guess without
//! limit would be a way around the counter rather than a second door with its
//! own lock.
//!
//! Unlike sign-on, the refusal says which thing was wrong. Sign-on must answer
//! identically for a wrong password and an unknown address because the
//! difference is an oracle for whether somebody works here; here the caller has
//! already proved they are this person, so there is nothing left to leak.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;

/// The rules a password has to clear.
///
/// Deliberately short, and deliberately the **same rule at setup and at
/// change**. A policy with five clauses is a policy people write on a sticky
/// note; two different policies is one of them being the real one and the other
/// being a surprise at the worst moment.
///
/// Twelve characters and nothing else. NIST's guidance has moved this way for
/// years: length is what buys resistance, and composition rules mostly buy
/// `Password1!`.
pub fn check_password(pw: &str) -> Result<(), &'static str> {
    if pw.chars().count() < 12 {
        return Err("a password needs at least twelve characters");
    }
    Ok(())
}

#[derive(Deserialize, Debug)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Serialize, Debug)]
pub struct PasswordChanged {
    /// How many other sessions were signed out. Reported rather than assumed:
    /// somebody changing a password because they think one was stolen wants to
    /// know whether there was anything to end.
    pub other_sessions_ended: i32,
}

/// Verify the old password, store a new one, and end every other session.
///
/// The order matters and is: refuse a locked account, read the digest, verify,
/// record the attempt either way, check the new password, then compare-and-set.
/// The write is `credential_change_password`, which only writes over the digest
/// it is given — so the read and the write cannot disagree about what was there,
/// and two changes racing cannot have the loser silently win.
pub async fn change_password(
    pool: &deadpool_postgres::Pool,
    person_id: Uuid,
    keep_token: &str,
    body: ChangePasswordRequest,
) -> Result<PasswordChanged, ApiError> {
    let conn = pool.get().await?;
    crate::tenancy::ensure_app_role(&conn).await?;

    let row = conn
        .query_opt(
            "SELECT phc, locked_until FROM credential_phc($1)",
            &[&person_id],
        )
        .await?
        // A signed-in person with no credential row at all. Not reachable
        // today — setup writes one and nothing deletes it — but the honest
        // answer is that there is no password here to change.
        .ok_or_else(|| {
            ApiError::Rejected("this account has no password to change".into())
        })?;

    let phc: Option<String> = row.get(0);
    let locked_until: Option<chrono::DateTime<chrono::Utc>> = row.get(1);

    if locked_until.is_some_and(|t| t > chrono::Utc::now()) {
        return Err(ApiError::Rejected(
            "this account is locked after too many wrong passwords. Wait fifteen minutes \
             and try again."
                .into(),
        ));
    }

    // **A passkey-only person, and the refusal is specific.** Migration 75 made
    // `phc` nullable so somebody can hold a key and no password. Changing a
    // password they do not have is not a thing that can be done here, and the
    // compare-and-set below would refuse it anyway — but silently, as though the
    // current password were wrong, which would be a lie.
    let Some(phc) = phc else {
        return Err(ApiError::Rejected(
            "this account signs in with a passkey and has no password to change".into(),
        ));
    };

    let current_ok = crate::auth::verify_password(&body.current_password, &phc);

    // Counted either way, on the same counter a sign-on uses. Guessing the
    // current password from inside a stolen session is the attack this endpoint
    // creates, and it gets the same ten attempts everything else does.
    conn.execute(
        "SELECT credential_record_attempt($1, $2)",
        &[&person_id, &current_ok],
    )
    .await?;

    if !current_ok {
        return Err(ApiError::Rejected("that is not the current password".into()));
    }

    check_password(&body.new_password).map_err(|m| ApiError::Rejected(m.into()))?;

    // **Not a password policy, a check for a wasted trip.** Argon2 salts per
    // hash, so an unchanged password would store a different digest and look
    // like a change; saying so is more use than quietly doing nothing.
    if crate::auth::verify_password(&body.new_password, &phc) {
        return Err(ApiError::Rejected(
            "that is already the password on this account".into(),
        ));
    }

    let new_phc = crate::auth::hash_password(&body.new_password)
        .map_err(|_| ApiError::Rejected("that password could not be stored".into()))?;

    let changed: bool = conn
        .query_one(
            "SELECT credential_change_password($1, $2, $3)",
            &[&person_id, &phc, &new_phc],
        )
        .await?
        .get(0);

    if !changed {
        // The digest moved between the read and the write, which means another
        // change landed first. Refusing is right: the caller verified against a
        // password that is no longer current.
        return Err(ApiError::Rejected(
            "the password was changed in another session. Try again."
                .into(),
        ));
    }

    // **Every other session dies, and this one lives.** The case that makes
    // change-password worth having is somebody who thinks a session was stolen,
    // and a change that leaves the thief signed in has not helped them. Signing
    // the caller out of the screen they are looking at, on the other hand, is
    // how people learn not to use it.
    let ended: i32 = conn
        .query_one(
            "SELECT session_revoke_others($1, $2)",
            &[&person_id, &crate::auth::token_digest(keep_token)],
        )
        .await?
        .get(0);

    tracing::info!(%person_id, other_sessions_ended = ended, "password changed");

    Ok(PasswordChanged {
        other_sessions_ended: ended,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_password_has_one_rule_and_it_is_length() {
        assert!(check_password("correct horse battery").is_ok());
        assert!(check_password("exactlytwelve").is_ok());
        assert!(check_password("short").is_err());
        // Twelve is the line, so eleven is not enough and twelve is.
        assert!(check_password("elevenchars").is_err());
        assert!(check_password("twelvechars!").is_ok());
    }

    #[test]
    fn length_is_counted_in_characters_rather_than_bytes() {
        // Twelve characters that are more than twelve bytes, and twelve that are
        // fewer than twelve characters. A `.len()` here would accept a
        // four-character password made of emoji and reject a legitimate one.
        assert!(check_password("ααααααααααα").is_err(), "eleven, in two-byte letters");
        assert!(check_password("αααααααααααα").is_ok(), "twelve of them");
    }
}
