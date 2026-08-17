-- Reverse of 2026-08-04-000007_observation.

DROP TABLE IF EXISTS observation;
DROP TABLE IF EXISTS observation_event;
DROP TABLE IF EXISTS metric_code;
DROP TABLE IF EXISTS metric;
DROP TABLE IF EXISTS observable;
DROP TABLE IF EXISTS item_packing_config;

DROP TYPE IF EXISTS ingestion_channel;
DROP TYPE IF EXISTS observation_method;
DROP TYPE IF EXISTS metric_result_kind;
DROP TYPE IF EXISTS packaging_level;
