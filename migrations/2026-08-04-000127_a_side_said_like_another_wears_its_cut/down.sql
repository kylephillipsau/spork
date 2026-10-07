-- Migration 127 down: each read finds a photograph's cut for itself again.

DROP FUNCTION cut_of(uuid);
