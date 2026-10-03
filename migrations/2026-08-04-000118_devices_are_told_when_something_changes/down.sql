DROP TRIGGER IF EXISTS fulfilment_live_changed ON fulfilment;
DROP TRIGGER IF EXISTS fulfilment_live_added ON fulfilment;
DROP TRIGGER IF EXISTS client_event_live ON client_event;
DROP FUNCTION IF EXISTS live_notify();
