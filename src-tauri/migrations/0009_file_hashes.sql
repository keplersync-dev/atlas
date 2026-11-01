-- Hashes of the files a share sends besides mods and packs, so Share only reads the ones
-- whose size or modified time changed. `settings`: sync/settings.rs's fingerprint, NULL
-- when the file's kind isn't compared by settings.
CREATE TABLE file_hash (
  instance_id  TEXT NOT NULL REFERENCES instance (id) ON DELETE CASCADE,
  path         TEXT NOT NULL,
  size_bytes   INTEGER NOT NULL,
  modified_ms  INTEGER NOT NULL,
  sha256       TEXT NOT NULL,
  settings     TEXT,
  PRIMARY KEY (instance_id, path)
);
