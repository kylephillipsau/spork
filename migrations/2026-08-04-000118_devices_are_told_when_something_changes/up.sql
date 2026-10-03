-- Devices are told when something changes at their site (D206).
--
-- Every act a device records inserts a `client_event`, and every order that
-- arrives or moves on writes `fulfilment`. A trigger on each sends one NOTIFY
-- per tenant and site the statement touched, on `spork_live`. The server holds
-- one connection that LISTENs and passes it on to the devices signed on there,
-- which read again what they are showing.
--
-- **Only where, never what.** The payload names the tenant and the site and
-- nothing else, so nothing is read out of a workspace by listening, and a
-- payload never nears NOTIFY's 8000 bytes. A device reads what changed through
-- the same endpoints and the same scoping as always.
--
-- **Delivered at commit.** NOTIFY waits for its transaction, so a device is
-- never told before the change can be read, and a rolled-back act tells nobody.
-- Postgres folds identical notifications within a transaction, so an import of
-- a thousand orders is one message per site.
--
-- Statement triggers with transition tables, so a bulk import is one trigger
-- call rather than a thousand. A transition table allows one event per
-- trigger, hence two on `fulfilment`.

CREATE FUNCTION live_notify() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  PERFORM pg_notify('spork_live', json_build_object('tenant', d.tenant_id, 'site', d.site_id)::text)
     FROM (SELECT DISTINCT tenant_id, site_id FROM fresh) d;
  RETURN NULL;
END
$$;

COMMENT ON FUNCTION live_notify() IS
  'D206: tells the server which tenant and site a statement changed, on spork_live, at commit.';

CREATE TRIGGER client_event_live
  AFTER INSERT ON client_event
  REFERENCING NEW TABLE AS fresh
  FOR EACH STATEMENT EXECUTE FUNCTION live_notify();

CREATE TRIGGER fulfilment_live_added
  AFTER INSERT ON fulfilment
  REFERENCING NEW TABLE AS fresh
  FOR EACH STATEMENT EXECUTE FUNCTION live_notify();

CREATE TRIGGER fulfilment_live_changed
  AFTER UPDATE ON fulfilment
  REFERENCING NEW TABLE AS fresh
  FOR EACH STATEMENT EXECUTE FUNCTION live_notify();
