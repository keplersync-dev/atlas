-- UNIQUE (instance_id, path) already indexes instance_id first.
DROP INDEX IF EXISTS instance_content_instance_id_idx;
