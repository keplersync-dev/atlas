-- Install new versions of a shared instance by themselves. Off for packs accepted before
-- this setting existed; accepting a pack sets it from the accept dialog.
ALTER TABLE instance ADD COLUMN auto_update INTEGER NOT NULL DEFAULT 0;
