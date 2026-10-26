-- Config folders ("config/create/") and loose files the owner chose not to share. JSON.
ALTER TABLE instance ADD COLUMN sync_exclude TEXT NOT NULL DEFAULT '[]';
