-- Migration 39: the winner's value, and the floors it cannot go under.
--
-- D83, finishing D22's resolver. D82 named a winning binding and stopped there,
-- so nothing could use it: `resolve` returned an id and the value was still in a
-- table nobody read.
--
-- D22 asks for two things here, and the second is the interesting one:
--
--   "takes the winner's value row, then clamps any field the value type declares
--    as clamped [...] the winner's value, clamped against every less-specific
--    match. That is what 'customer x item class, plus a site floor' actually
--    asks for, and it gives a commercial product a platform-shipped ceiling no
--    tenant can exceed."
--
-- and, on D14: **"so a site floor raises a customer rule."**

-- ---------------------------------------------------------------------------
-- 1. One function, in long format, rather than one per kind
-- ---------------------------------------------------------------------------
--
-- The obvious shape is three functions returning three different row types, and
-- it is wrong for the reason clamping is generic: **the clamp is per field, and
-- the caller needs every candidate's value for that field, not just the
-- winner's.** In row format that is three code paths that must each remember to
-- fetch the losers; in long format it is one.
--
-- Only numeric and boolean fields are returned. A clamped field is one where
-- "less specific" and "more specific" can disagree by *degree*, and that is what
-- numbers do -- `receiving_policy.default_status_id` is a choice rather than a
-- degree, so it comes from the winner whole and is not here.
--
-- Booleans are `0` and `1` so that `false < true` and a floor means "no less
-- strict than anything less specific". A platform rule requiring lot capture
-- cannot be relaxed by a tenant, and that is the same sentence as a shelf-life
-- floor rather than a special case.

CREATE FUNCTION policy_value(
        p_kind     policy_kind,
        p_bindings uuid[],
        p_at       timestamptz DEFAULT now())
    RETURNS TABLE (policy_binding_id uuid, field text, value numeric)
    LANGUAGE sql STABLE
    SET search_path = pg_catalog, public
    AS $$
    SELECT v.policy_binding_id, f.field, f.value
      FROM allocation_policy v
      CROSS JOIN LATERAL (VALUES
          ('weight_rotation',      v.weight_rotation::numeric),
          ('weight_travel',        v.weight_travel::numeric),
          ('weight_consolidation', v.weight_consolidation::numeric),
          ('allow_partial',        v.allow_partial::integer::numeric)) AS f(field, value)
     WHERE p_kind = 'allocation'
       AND v.policy_binding_id = ANY(p_bindings) AND v.effective @> p_at
     UNION ALL
    SELECT v.policy_binding_id, f.field, f.value
      FROM receiving_policy v
      CROSS JOIN LATERAL (VALUES
          ('respond_by_hours',    v.respond_by_hours::numeric),
          ('require_lot',         v.require_lot::integer::numeric),
          ('tolerance_over_pct',  v.tolerance_over_pct),
          ('tolerance_under_pct', v.tolerance_under_pct)) AS f(field, value)
     WHERE p_kind = 'receiving'
       AND v.policy_binding_id = ANY(p_bindings) AND v.effective @> p_at
     UNION ALL
    SELECT v.policy_binding_id, f.field, f.value
      FROM shelf_life_policy v
      CROSS JOIN LATERAL (VALUES
          ('min_shelf_life_days', v.min_shelf_life_days::numeric),
          ('min_shelf_life_pct',  v.min_shelf_life_pct)) AS f(field, value)
     WHERE p_kind = 'shelf_life'
       AND v.policy_binding_id = ANY(p_bindings) AND v.effective @> p_at
$$;

COMMENT ON FUNCTION policy_value(policy_kind, uuid[], timestamptz) IS
    'Every candidate''s numeric and boolean fields, in long format, so the caller '
    'can clamp a field against the matches less specific than the winner. Choices '
    'rather than degrees -- default_status_id -- are not here: they come from the '
    'winner whole. D22, D83.';

GRANT EXECUTE ON FUNCTION policy_value(policy_kind, uuid[], timestamptz)
    TO spork_app, spork_platform, spork_scheduler;

-- ---------------------------------------------------------------------------
-- 2. What decides which fields clamp, and in which direction
-- ---------------------------------------------------------------------------
--
-- Not this migration. D22 puts it *"on the Rust value type"*, and `crates/policy`
-- declares it beside the precedence orderings D81 justified, for the same reason:
-- **a clamp direction is as invisible and as consequential as a precedence
-- order.** A manager who believes a customer can shorten a shelf-life floor is
-- wrong in the same way as one who believes product outranks counterparty.
--
-- `allocation` declares no clamped fields at all, and D22 says why in its own
-- words: *"you must not take `weight_rotation` from a customer binding and
-- `weight_travel` from a site binding, because weights are only meaningful
-- relative to each other."* The whole row wins or none of it does.
