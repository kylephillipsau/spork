-- Migration 76 down: signing in needs an email again.

DELETE FROM webauthn_challenge WHERE kind = 'discoverable';

ALTER TABLE webauthn_challenge DROP CONSTRAINT webauthn_challenge_discoverable_ck;
ALTER TABLE webauthn_challenge DROP CONSTRAINT webauthn_challenge_kind_ck;
ALTER TABLE webauthn_challenge ADD CONSTRAINT webauthn_challenge_kind_ck
    CHECK (kind IN ('registration', 'authentication'));
