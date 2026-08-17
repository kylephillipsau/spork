-- Migration 84: a photograph of a thing is not a side of it.
--
-- D140 gave a record a way to point at the look that supports it, and the
-- pictures still land in `observation_image`, which types every one by *which
-- side of the object* — the seven faces D132 chose for cubing a carton.
--
-- **A photograph of a crushed corner is not a side.** There is no honest value
-- among the seven for it, and the tempting one is `front`, which is where this
-- becomes a defect rather than a wart: D141 resolves the picker's recognition
-- picture from the **front** face. Evidence filed as `front` would become what
-- the pick walk shows as *what to look for*, so the newest damage report would
-- silently replace the product photograph on the handheld of the next person
-- sent to that bay.
--
-- That is the shape this project keeps finding — a fact filed where it is
-- convenient and then indistinguishable from a fact of a different kind — and
-- it is worth a migration to avoid rather than a convention to remember.
--
-- So the vocabulary grows by one word that is not a side. `face` is a text
-- column with a CHECK rather than an enum, so unlike migration 83 this reverses
-- cleanly and runs in a transaction.

ALTER TABLE observation_image DROP CONSTRAINT observation_image_face_ck;
ALTER TABLE observation_image ADD CONSTRAINT observation_image_face_ck
    CHECK (face IN ('front', 'back', 'left', 'right', 'top', 'bottom', 'label',
                    'detail'));

COMMENT ON COLUMN observation_image.face IS
    'Which side of the object, plus `detail` for a photograph that is not a '
    'side of it — a crushed corner, a torn seal, whatever a record is being '
    'asked to believe. Kept out of the seven deliberately: D141 resolves the '
    'picker''s recognition picture from `front`, so evidence filed there would '
    'become what the next person sent to that bay is shown as what to look '
    'for. D140, migration 84.';
