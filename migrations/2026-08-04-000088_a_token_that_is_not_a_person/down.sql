DROP FUNCTION IF EXISTS api_tokens_for_tenant(uuid);
DROP FUNCTION IF EXISTS api_token_revoke(uuid, uuid);
DROP FUNCTION IF EXISTS api_token_resolve(bytea);
DROP FUNCTION IF EXISTS api_token_open(uuid, uuid, text, bytea, interval);
DROP TABLE IF EXISTS api_token;
