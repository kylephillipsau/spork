-- Migration 16: authority is declared, not spelled.
--
-- Question 132, raised by D48. D39 put exactly one system of record on every
-- order and said that an order whose record of authority is external is amended
-- through that system rather than here. J44 is the invariant behind that
-- sentence, and it has never been implementable, because the only thing
-- distinguishing an authoritative channel from an ordinary one was how its name
-- happened to be spelled in a free text column.
--
-- `source_channel text NOT NULL` is greppable by a human and by nothing else. A
-- check wanting to know whether an order is externally authoritative had to carry
-- a list of magic strings, and a list of magic strings in a check is the
-- convention it was meant to replace, written down twice.

-- ---------------------------------------------------------------------------
-- 1. Two values, and code branches on them
-- ---------------------------------------------------------------------------
--
-- D33's test: a fixed set that code branches on is an enum. This is fixed at two
-- because D39 fixed it there. Bidirectional merge was refused outright, so an
-- order's record of authority is ours or theirs and there is no third answer; a
-- 'shared' value would be the merge D39 rejected, wearing a different label.

CREATE TYPE channel_authority AS ENUM ('local', 'external');

COMMENT ON TYPE channel_authority IS
    'Whose system of record an order arriving on this channel belongs to. D39: '
    'one authority per row, and never both.';

-- ---------------------------------------------------------------------------
-- 2. The channel becomes a thing
-- ---------------------------------------------------------------------------
--
-- Shared-or-tenant, the same shape as carrier and package_type: a platform row
-- with a NULL tenant is visible to everyone, and a tenant may add its own. Manual
-- entry is universal and ships here; every other channel belongs to whoever
-- connected it.

CREATE TABLE source_channel (
    id        uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id uuid REFERENCES tenant(id),   -- NULL = platform-shipped
    code      text NOT NULL,
    name      text NOT NULL,

    -- The whole point of the migration. D39's "record of authority", as a value
    -- rather than as a naming convention.
    authority channel_authority NOT NULL,

    -- Deliberately absent: a counterparty_party_id naming whose system an
    -- external channel is. It is the obvious next column and it is not needed to
    -- settle 132, which asks only where authority is declared. It also cannot go
    -- on a platform-shipped row, because `party` is tenant-scoped and J14 forbids
    -- a shared row naming a tenant's, so it needs a decision about whether a
    -- channel can be shared at all once it names a counterparty. Question 128
    -- already owns what an external succession carries.

    CONSTRAINT source_channel_code_key UNIQUE NULLS NOT DISTINCT (tenant_id, code)
);

COMMENT ON TABLE source_channel IS
    'REFERENCE. Where an order came from, and whose record of authority that '
    'makes it. D39, D54.';
COMMENT ON COLUMN source_channel.authority IS
    'local: ours, amendable here. external: a counterparty''s, amended through '
    'their system. J44 reads this and nothing else.';

ALTER TABLE source_channel ENABLE ROW LEVEL SECURITY;
ALTER TABLE source_channel FORCE ROW LEVEL SECURITY;
CREATE POLICY source_channel_shared_reference ON source_channel
    USING (tenant_id IS NULL OR tenant_id = current_tenant());

GRANT SELECT, INSERT, UPDATE, DELETE ON source_channel TO spork_app;

-- Manual entry: somebody types an order in. Every deployment has it, it is
-- necessarily ours, and it is the fallback D39's "an operation running nothing
-- else works" requires.
INSERT INTO source_channel (tenant_id, code, name, authority) VALUES
    (NULL, 'manual', 'Entered by hand', 'local');

-- ---------------------------------------------------------------------------
-- 3. Backfill, and why every existing channel is local
-- ---------------------------------------------------------------------------
--
-- One row per distinct channel name already in use, per tenant, all of them
-- `local`. That is a claim about the world rather than a convenient default, and
-- it is checkable: an external channel is one where a counterparty holds the
-- record of authority, which under D21 makes their document an assertion, and
-- the assertion tables do not exist. **No external channel can have written an
-- order yet, because nothing that would make one external has been built.**
--
-- Defaulting the other way would have been worse than wrong: J44 would fire on
-- every historical order at once and the first thing anyone did would be to
-- silence it.

INSERT INTO source_channel (tenant_id, code, name, authority)
SELECT DISTINCT o.tenant_id, o.source_channel, o.source_channel, 'local'::channel_authority
  FROM "order" o
 WHERE NOT EXISTS (SELECT 1 FROM source_channel sc
                    WHERE sc.code = o.source_channel
                      AND sc.tenant_id IS NOT DISTINCT FROM o.tenant_id);

ALTER TABLE "order" ADD COLUMN source_channel_id uuid REFERENCES source_channel(id);

UPDATE "order" o
   SET source_channel_id = sc.id
  FROM source_channel sc
 WHERE sc.code = o.source_channel
   AND sc.tenant_id IS NOT DISTINCT FROM o.tenant_id;

ALTER TABLE "order" ALTER COLUMN source_channel_id SET NOT NULL;
ALTER TABLE "order" DROP COLUMN source_channel;

-- ---------------------------------------------------------------------------
-- 4. Grants: declared at creation means declared at creation
-- ---------------------------------------------------------------------------
--
-- `source_channel_id` is in the INSERT list and not the UPDATE list. D39 says the
-- system of record is declared at creation and recorded on the row; moving an
-- order to a different authority afterwards is the bidirectional merge D39
-- refused, performed one row at a time. The old text column was UPDATE-able only
-- because it arrived inside a table-wide grant nobody had revisited.

-- `currency` is granted here for the first time. D50 added the column in
-- migration 13 and granted nothing, so the application could price an order line
-- and could never record the currency that price is denominated in — which J54
-- requires before a line may be priced at all. The column has been unwritable
-- since the day it was created, and unwritable is a state nothing reported,
-- because every guard in the suite watches for too much privilege rather than
-- none. S45 is the other direction: on a table the application writes, every
-- non-projection column is in one of its grant lists, and `currency` was in
-- neither. The way that breaks is always the same — a column added to a table
-- whose grants were already enumerated, and enumerations do not update
-- themselves. It has now happened twice.
--
-- INSERT and UPDATE both, because pricing an order need not happen in the same
-- statement that creates it. D50's "one order, one currency" is a claim about the
-- row rather than about immutability.

REVOKE INSERT, UPDATE ON "order" FROM spork_app;

GRANT INSERT (id, tenant_id, site_id, customer_party_id, confirmation_number,
              source_channel_id, external_ref, supersedes_order_id, placed_at,
              promised_from, promised_to, required_by, state, currency),
      UPDATE (site_id, customer_party_id, confirmation_number, external_ref,
              supersedes_order_id, currency)
    ON "order" TO spork_app;

CREATE INDEX order_source_channel_idx ON "order" (source_channel_id);

-- ---------------------------------------------------------------------------
-- 5. What J44 turns out to be, which is not what D48 narrowed it to
-- ---------------------------------------------------------------------------
--
-- D48 narrowed J44 from "no amendment" to "no `world_event` amendment", because
-- fixing our own mis-transcription of a counterparty's document is not amending
-- their document. That is right and it is not the whole rule, and implementing it
-- is what shows why.
--
-- Consider a counterparty cancelling their own order. Their message arrives on
-- the channel, and recording it sets `order.state`, which is a covered column, so
-- it is an `intention_amendment` carrying `new_state`. The world did change, so
-- its `revision_class` is `world_event`. Under D48's statement that amendment is
-- forbidden, and there is no other way to record something that unambiguously
-- happened.
--
-- So the axis was wrong. What D39 forbids is not a class of change, it is a
-- **place**: "an order whose record of authority is external is amended through
-- that system, not here." A counterparty's event arriving through the channel is
-- their amendment reaching us. A person here typing the same thing is us editing
-- their order, which is the merge D39 refused.
--
-- `intention_amendment` already carries that distinction, in
-- `intention_amendment_actor_ck`: exactly one of `recorded_by_id` and
-- `automation_key`. An amendment with a person on it was made here.
--
-- The proxy is imperfect and the imperfection has a number. `automation_key` is
-- undefined — D27 narrowed it by contrast, "a device is *how*, a key is *who*",
-- without saying what one is — which is question 105. So a local nightly job
-- holding an automation key would pass a check that a person would fail. 105 is
-- now load-bearing rather than tidy, and J44 carries the strongest rule the
-- schema can currently express.

-- ---------------------------------------------------------------------------
-- 6. Two closure tables nothing could ever have written
-- ---------------------------------------------------------------------------
--
-- Found by a draft of S45 and kept after it was narrowed, because the finding is
-- real whichever check surfaced it. It is the third instance of one family.
-- `item_class_closure` carries a table comment saying it is "maintained by a
-- named function under D35, never written by the application role". The second
-- half is enforced — the app holds SELECT and nothing else. The first half names
-- a function that does not exist.
--
-- So in a deployment the closure tables can only be written by a superuser. The
-- fixture populates them because it runs as one. **D22's entire matching language
-- is "is this node an ancestor-or-self of that node" over these tables**, so a
-- resolver in production would read an empty closure and every scoped policy
-- would silently fail to match.
--
-- This marks the gap rather than inventing a maintainer for it, exactly as
-- migration 12 did for `consignment` and refused to do for
-- `projection_fulfilment_rebuild`. Writing the closure maintainer is D22 work
-- with a real decision in it — re-parenting is question 78, and whether the
-- rebuild is incremental or total is not obvious — and guessing at it inside a
-- migration named for something else is how the last two got their comments.
--
-- The narrowed S45 does not cover these tables at all: it inspects tables the
-- application may write, and the application holds SELECT here and nothing else.
-- So nothing in the suite watches this, which is the reason to write it down.
--
-- Question 140.

COMMENT ON COLUMN item_class_closure.tenant_id IS
    '@projection(pending) of item_class; no maintainer exists. Question 140.';
COMMENT ON COLUMN item_class_closure.ancestor_id IS
    '@projection(pending) of item_class; no maintainer exists. Question 140.';
COMMENT ON COLUMN item_class_closure.descendant_id IS
    '@projection(pending) of item_class; no maintainer exists. Question 140.';
COMMENT ON COLUMN item_class_closure.depth IS
    '@projection(pending) of item_class; no maintainer exists. Question 140.';

COMMENT ON COLUMN party_class_closure.tenant_id IS
    '@projection(pending) of party_class; no maintainer exists. Question 140.';
COMMENT ON COLUMN party_class_closure.ancestor_id IS
    '@projection(pending) of party_class; no maintainer exists. Question 140.';
COMMENT ON COLUMN party_class_closure.descendant_id IS
    '@projection(pending) of party_class; no maintainer exists. Question 140.';
COMMENT ON COLUMN party_class_closure.depth IS
    '@projection(pending) of party_class; no maintainer exists. Question 140.';

COMMENT ON TABLE item_class_closure IS
    'PROJECTION of item_class. Never written by the application role, which is '
    'enforced. The named function under D35 does not exist yet: question 140.';
