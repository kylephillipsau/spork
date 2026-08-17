-- Migration 76: a key that knows who it is.
--
-- Migration 75 authenticates against an allow-list: say who you are, and the
-- server offers the keys that person holds. That is one field to type before the
-- key is even reached, and on a shared handheld it is the field nobody wants to
-- type — an email address, on a touchscreen, with gloves on.
--
-- A discoverable credential carries the user handle inside it. The server asks
-- for *any* key, the authenticator offers the ones it holds for this site, and
-- the person taps. Nothing is typed and the server learns who it is talking to
-- from the assertion.
--
-- **`webauthn_challenge.person_id` was already nullable for this**, and said so:
-- *"absent for a discoverable-credential sign-in, where the whole point is that
-- the server does not know who is at the keyboard until they answer."* What was
-- missing is a way to tell that ceremony apart from an allow-list one, because
-- the state between the halves is a different type and deserialising the wrong
-- one is a 500 rather than a refusal.
--
-- A third kind rather than inferring it from a null person. An unknown email
-- also yields a null person — the allow-list is empty and the ceremony still
-- issues, so as not to say who exists — so a null would have meant two things.

ALTER TABLE webauthn_challenge DROP CONSTRAINT webauthn_challenge_kind_ck;
ALTER TABLE webauthn_challenge ADD CONSTRAINT webauthn_challenge_kind_ck
    CHECK (kind IN ('registration', 'authentication', 'discoverable'));

-- A discoverable ceremony must NOT name a person: if the server already knew,
-- it would not need to ask this way, and a person named here would be an
-- assumption the assertion is about to contradict.
ALTER TABLE webauthn_challenge ADD CONSTRAINT webauthn_challenge_discoverable_ck
    CHECK (kind <> 'discoverable' OR person_id IS NULL);

COMMENT ON COLUMN webauthn_challenge.kind IS
    'Which ceremony: enrolling a key, authenticating against a named person''s '
    'keys, or authenticating against whatever key the authenticator offers. The '
    'third names no person, because not knowing yet is the point. Migration 76.';
